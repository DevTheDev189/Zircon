#!/usr/bin/env python3
"""
ci_deploy_cloudflare.py
Automated Cloudflare deployment for GitHub Actions:
  1. Staged multi-platform artifacts into R2 structure.
  2. Computes SHA256 hashes and generates latest.json manifests for launcher & server.
  3. Uploads binary update tree to Cloudflare R2 (downloads.zirconmc.net).
  4. Deploys website to Cloudflare Pages (zirconmc.net).
"""

import os
import sys
import re
import glob
import json
import hashlib
import datetime
import urllib.request
import subprocess
from pathlib import Path

def sha256_file(filepath: Path) -> str:
    h = hashlib.sha256()
    with open(filepath, "rb") as f:
        while chunk := f.read(65536):
            h.update(chunk)
    return h.hexdigest().upper()

def upload_to_r2(account_id: str, bucket: str, token: str, key: str, filepath: Path):
    url = f"https://api.cloudflare.com/client/v4/accounts/{account_id}/r2/buckets/{bucket}/objects/{key}"
    headers = {
        "Authorization": f"Bearer {token}",
        "Content-Type": "application/octet-stream",
    }
    file_size = filepath.stat().st_size
    print(f"  -> Uploading {key} ({file_size / (1024*1024):.2f} MB)...", flush=True)
    with open(filepath, "rb") as f:
        data = f.read()
    req = urllib.request.Request(url, data=data, headers=headers, method="PUT")
    with urllib.request.urlopen(req) as resp:
        if resp.status not in (200, 201):
            raise RuntimeError(f"R2 upload failed for {key}: HTTP {resp.status}")

def cleanup_old_r2_objects(account_id: str, bucket: str, token: str, current_version: str):
    list_url = f"https://api.cloudflare.com/client/v4/accounts/{account_id}/r2/buckets/{bucket}/objects"
    headers = {"Authorization": f"Bearer {token}"}
    try:
        req = urllib.request.Request(list_url, headers=headers, method="GET")
        with urllib.request.urlopen(req) as resp:
            data = json.loads(resp.read().decode("utf-8"))
            objects = data.get("result", [])
            for obj in objects:
                key = obj.get("key", "")
                # Purge older launcher binaries (keep latest.json and current version)
                if key.startswith("updates/launcher/") and "latest.json" not in key:
                    if any(c.isdigit() for c in key) and current_version not in key and "zircon-macos-app" not in key.lower():
                        print(f"  -> Purging obsolete launcher release from R2: {key}")
                        del_url = f"https://api.cloudflare.com/client/v4/accounts/{account_id}/r2/buckets/{bucket}/objects/{key}"
                        urllib.request.urlopen(urllib.request.Request(del_url, headers=headers, method="DELETE"))
                # Purge older server release directories (keep latest.json and current version)
                elif key.startswith("updates/server/") and not key.startswith(f"updates/server/v{current_version}/") and "latest.json" not in key:
                    print(f"  -> Purging obsolete server release from R2: {key}")
                    del_url = f"https://api.cloudflare.com/client/v4/accounts/{account_id}/r2/buckets/{bucket}/objects/{key}"
                    urllib.request.urlopen(urllib.request.Request(del_url, headers=headers, method="DELETE"))
    except Exception as e:
        print(f"  Notice: R2 cleanup skipped or completed: {e}")

def stamp_website_files(website_dir: Path, version: str):
    print(f"\n  Stamping release version v{version} into website files...")
    patterns = [
        (re.compile(r'v\d+\.\d+\.\d+'), f'v{version}'),
        (re.compile(r'Zircon_\d+\.\d+\.\d+_'), f'Zircon_{version}_'),
        (re.compile(r'zircon_\d+\.\d+\.\d+_'), f'zircon_{version}_'),
        (re.compile(r'zircon-\d+\.\d+\.\d+-'), f'zircon-{version}-'),
    ]
    for ext in ("*.html", "*.js"):
        for f in website_dir.rglob(ext):
            try:
                content = f.read_text(encoding="utf-8")
                new_content = content
                for pat, repl in patterns:
                    new_content = pat.sub(repl, new_content)
                if new_content != content:
                    f.write_text(new_content, encoding="utf-8")
                    print(f"    -> Updated version in {f.name}")
            except Exception as e:
                print(f"    Notice: could not stamp {f.name}: {e}")

def main():
    token = os.environ.get("CLOUDFLARE_API_TOKEN", "").strip()
    account_id = os.environ.get("CLOUDFLARE_ACCOUNT_ID", "").strip()
    bucket = os.environ.get("CLOUDFLARE_R2_BUCKET", "").strip()
    version = os.environ.get("RELEASE_VERSION", "").strip().lstrip("v")
    assets_dir = Path(os.environ.get("RELEASE_ASSETS_DIR", "release-dist"))
    domain = "https://downloads.zirconmc.net"

    if not version:
        print("ERROR: RELEASE_VERSION environment variable is required.", file=sys.stderr)
        sys.exit(1)

    print("=" * 70)
    print(f" Zircon Cloudflare Automated Deployment (v{version})")
    print("=" * 70)

    # 1. Prepare R2 Staging Directory
    r2_stage = Path("r2-upload")
    if r2_stage.exists():
        import shutil
        shutil.rmtree(r2_stage)

    server_stage = r2_stage / "updates" / "server" / f"v{version}"
    launcher_stage = r2_stage / "updates" / "launcher"
    server_stage.mkdir(parents=True, exist_ok=True)
    launcher_stage.mkdir(parents=True, exist_ok=True)

    print("\n[1/4] Processing server artifacts & generating manifest...")
    server_platforms = {}
    
    # Locate server archives
    for f in assets_dir.glob("*server*"):
        if f.suffix in (".zip", ".gz"):
            dest = server_stage / f.name
            dest.write_bytes(f.read_bytes())
            file_hash = sha256_file(dest)
            if "windows" in f.name.lower():
                server_platforms["windows-x86_64"] = {
                    "url": f"{domain}/updates/server/v{version}/{f.name}",
                    "sha256": file_hash,
                    "binName": "zircon-server.exe"
                }
                print(f"  Windows server staged: {f.name} (SHA256: {file_hash})")
            elif "linux" in f.name.lower() and f.suffix == ".zip":
                server_platforms["linux-x86_64"] = {
                    "url": f"{domain}/updates/server/v{version}/{f.name}",
                    "sha256": file_hash,
                    "binName": "zircon-server"
                }
                print(f"  Linux server zip staged: {f.name} (SHA256: {file_hash})")

    server_manifest = {
        "version": version,
        "releaseDate": datetime.datetime.utcnow().strftime("%Y-%m-%d %H:%M:%S"),
        "notes": f"Zircon Server Release v{version}",
        "platforms": server_platforms
    }
    server_manifest_file = r2_stage / "updates" / "server" / "latest.json"
    server_manifest_file.write_text(json.dumps(server_manifest, indent=2), encoding="utf-8")
    print("  -> updates/server/latest.json generated.")

    print("\n[2/4] Processing launcher bundles & generating updater manifest...")
    launcher_platforms = {}
    pub_date = datetime.datetime.utcnow().strftime("%Y-%m-%dT%H:%M:%SZ")

    valid_launcher_extensions = (".exe", ".msi", ".dmg", ".AppImage", ".deb", ".rpm", ".tar.gz", ".sig", ".zip")
    for f in assets_dir.iterdir():
        if "server" in f.name.lower():
            continue
        if "control.tar.gz" in f.name.lower() or "data.tar.gz" in f.name.lower():
            continue
        # Skip older version artifacts
        if any(c.isdigit() for c in f.name) and version not in f.name and "zircon-macos-app" not in f.name.lower():
            print(f"  Skipping older version artifact: {f.name}")
            continue
        if any(f.name.endswith(ext) for ext in valid_launcher_extensions):
            dest = launcher_stage / f.name
            dest.write_bytes(f.read_bytes())
            print(f"  Launcher artifact staged: {f.name}")

    # Build launcher platforms
    # Windows: prefer .nsis.zip for the updater feed (which Tauri v2 updater plugin consumes)
    win_nsis_zips = list(launcher_stage.glob("*setup.nsis.zip"))
    win_setups = list(launcher_stage.glob("*setup.exe"))
    if win_nsis_zips:
        sig_file = launcher_stage / f"{win_nsis_zips[0].name}.sig"
        sig_content = sig_file.read_text(encoding="utf-8").strip() if sig_file.exists() else ""
        launcher_platforms["windows-x86_64"] = {
            "signature": sig_content,
            "url": f"{domain}/updates/launcher/{win_nsis_zips[0].name}"
        }
    elif win_setups:
        sig_file = launcher_stage / f"{win_setups[0].name}.sig"
        sig_content = sig_file.read_text(encoding="utf-8").strip() if sig_file.exists() else ""
        launcher_platforms["windows-x86_64"] = {
            "signature": sig_content,
            "url": f"{domain}/updates/launcher/{win_setups[0].name}"
        }

    linux_appimages = list(launcher_stage.glob("*.AppImage"))
    if linux_appimages:
        sig_file = launcher_stage / f"{linux_appimages[0].name}.sig"
        sig_content = sig_file.read_text(encoding="utf-8").strip() if sig_file.exists() else ""
        launcher_platforms["linux-x86_64"] = {
            "signature": sig_content,
            "url": f"{domain}/updates/launcher/{linux_appimages[0].name}"
        }

    # macOS: Tauri v2 checks for both "darwin-aarch64-app" / "darwin-aarch64" (ARM64)
    # and "darwin-x86_64-app" / "darwin-x86_64" (Intel).
    # Tauri updater on macOS strictly consumes the .app.tar.gz bundle.
    mac_tar_gzs = [f for f in launcher_stage.glob("*.tar.gz") if not f.name.lower().startswith("zircon-server")]
    # Prioritize any archive with an accompanying .sig file
    mac_updaters = sorted(mac_tar_gzs, key=lambda f: 0 if (launcher_stage / f"{f.name}.sig").exists() else 1)

    if mac_updaters:
        for mac_pkg in mac_updaters:
            sig_file = launcher_stage / f"{mac_pkg.name}.sig"
            sig_content = sig_file.read_text(encoding="utf-8").strip() if sig_file.exists() else ""
            url = f"{domain}/updates/launcher/{mac_pkg.name}"
            name_lower = mac_pkg.name.lower()

            if "x86_64" in name_lower or "x64" in name_lower:
                for key in ("darwin-x86_64", "darwin-x86_64-app"):
                    if key not in launcher_platforms:
                        launcher_platforms[key] = {"signature": sig_content, "url": url}
            elif "aarch64" in name_lower or "arm64" in name_lower:
                for key in ("darwin-aarch64", "darwin-aarch64-app"):
                    if key not in launcher_platforms:
                        launcher_platforms[key] = {"signature": sig_content, "url": url}
            else:
                # Universal or default macOS bundle (e.g. Zircon.app.tar.gz or Zircon-macOS-app.tar.gz)
                for key in ("darwin-aarch64", "darwin-aarch64-app", "darwin-x86_64", "darwin-x86_64-app"):
                    if key not in launcher_platforms:
                        launcher_platforms[key] = {"signature": sig_content, "url": url}
        print(f"  macOS updater configured with: {[f.name for f in mac_updaters]}")
    else:
        # Fallback to .dmg if no .tar.gz was found
        mac_dmgs = list(launcher_stage.glob("*.dmg"))
        if mac_dmgs:
            sig_file = launcher_stage / f"{mac_dmgs[0].name}.sig"
            sig_content = sig_file.read_text(encoding="utf-8").strip() if sig_file.exists() else ""
            url = f"{domain}/updates/launcher/{mac_dmgs[0].name}"
            for key in ("darwin-aarch64", "darwin-aarch64-app", "darwin-x86_64", "darwin-x86_64-app"):
                launcher_platforms[key] = {"signature": sig_content, "url": url}
            print(f"  macOS updater configured with fallback DMG: {mac_dmgs[0].name}")
        else:
            print("  WARNING: No macOS updater bundle found!")

    launcher_manifest = {
        "version": version,
        "notes": f"Zircon Launcher Release v{version}",
        "pub_date": pub_date,
        "platforms": launcher_platforms
    }
    launcher_manifest_file = launcher_stage / "latest.json"
    launcher_manifest_file.write_text(json.dumps(launcher_manifest, indent=2), encoding="utf-8")
    print("  -> updates/launcher/latest.json generated.")

    # 3. Upload to Cloudflare R2
    if token and account_id and bucket:
        print(f"\n[3/4] Uploading artifacts to Cloudflare R2 bucket '{bucket}'...")
        for path in r2_stage.rglob("*"):
            if path.is_file():
                key = path.relative_to(r2_stage).as_posix()
                upload_to_r2(account_id, bucket, token, key, path)
        print("  -> All binary artifacts and manifests successfully synced to R2!")
        
        # Purge older versions so only the latest release is kept
        print("\n  Cleaning up older version artifacts from R2...")
        cleanup_old_r2_objects(account_id, bucket, token, version)
    else:
        print("\n[3/4] SKIPPED R2 Upload: CLOUDFLARE_API_TOKEN, ACCOUNT_ID, or R2_BUCKET secret missing.")

    # 4. Deploy Website to Cloudflare Pages (Production branch 'main')
    if token and account_id:
        print("\n[4/4] Deploying website to Cloudflare Pages (project 'zircon', branch 'main' -> Production)...")
        website_dir = Path("website")
        if website_dir.exists():
            stamp_website_files(website_dir, version)
        env = os.environ.copy()
        env["CLOUDFLARE_API_TOKEN"] = token
        env["CLOUDFLARE_ACCOUNT_ID"] = account_id
        res = subprocess.run([
            "npx", "--yes", "wrangler", "pages", "deploy", "website",
            "--project-name", "zircon",
            "--branch", "main",
            "--commit-dirty=true"
        ], env=env)
        if res.returncode != 0:
            print("WARNING: Wrangler Pages deployment failed with non-zero exit code.", file=sys.stderr)
        else:
            print("  -> Website successfully deployed to Cloudflare Pages (Production)!")
    else:
        print("\n[4/4] SKIPPED Pages Deploy: CLOUDFLARE_API_TOKEN or ACCOUNT_ID missing.")

    print("\n" + "=" * 70)
    print(" Deployment Pipeline Complete!")
    print("=" * 70)

if __name__ == "__main__":
    main()
