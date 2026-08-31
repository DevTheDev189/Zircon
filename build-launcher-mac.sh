#!/usr/bin/env bash
# ==============================================================================
#  Zircon Launcher - macOS Build Script
#  Builds only the companion launcher (native .app and .dmg bundle) for macOS.
# ==============================================================================
set -euo pipefail

# Ensure ~/.cargo/bin is in PATH if installed via rustup
if [ -d "$HOME/.cargo/bin" ]; then
    export PATH="$HOME/.cargo/bin:$PATH"
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$SCRIPT_DIR"
LAUNCHER_DIR="$ROOT_DIR/crates/zircon-launcher"
UI_DIR="$LAUNCHER_DIR/ui"

echo "======================================================================"
echo "          Zircon Launcher - macOS Build Script"
echo "======================================================================"
echo ""

# ------------------------------------------------------------------------------
# 1. Environment & Dependency Checks
# ------------------------------------------------------------------------------
echo "==> [1/4] Checking build prerequisites..."

# Check Xcode / Command Line Tools
if ! xcode-select -p >/dev/null 2>&1; then
    echo "ERROR: Xcode Command Line Tools are not installed."
    echo "Run the following command in Terminal and follow the prompt:"
    echo "    xcode-select --install"
    exit 1
fi

# Check Rust & Cargo
if ! command -v cargo >/dev/null 2>&1; then
    echo "ERROR: Rust / Cargo is not found in PATH."
    echo "To install the Rust toolchain, run:"
    echo "    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    echo "Then restart your terminal or run: source \"\$HOME/.cargo/env\""
    exit 1
fi
echo "  [✓] Rust $(rustc --version | awk '{print $2}') & Cargo $(cargo --version | awk '{print $2}')"

# Check Node.js & npm
if ! command -v node >/dev/null 2>&1 || ! command -v npm >/dev/null 2>&1; then
    echo "ERROR: Node.js / npm is not installed or not in PATH."
    echo "Install Node.js (v18+ or v20+ recommended) via Homebrew:"
    echo "    brew install node"
    echo "or download the macOS installer from: https://nodejs.org/"
    exit 1
fi
echo "  [✓] Node.js $(node -v) & npm v$(npm -v)"

# ------------------------------------------------------------------------------
# 2. Keyring & Tauri Signing Key Configuration
# ------------------------------------------------------------------------------
echo ""
echo "==> [2/4] Checking Tauri signing key configuration..."

HAS_KEY=false
if [ -f "$HOME/.tauri/zircon.key" ]; then
    # minisign key file has a comment on line 1 and base64 private key on line 2
    KEY_VAL=$(tail -n 1 "$HOME/.tauri/zircon.key" | sed 's/.*untrusted comment://')
    export TAURI_SIGNING_PRIVATE_KEY="$KEY_VAL"
fi

if [ -f "$HOME/.tauri/password.txt" ]; then
    export TAURI_SIGNING_PRIVATE_KEY_PASSWORD="$(cat "$HOME/.tauri/password.txt")"
fi

if [ -n "${TAURI_SIGNING_PRIVATE_KEY:-}" ] && [ -n "${TAURI_SIGNING_PRIVATE_KEY_PASSWORD:-}" ]; then
    HAS_KEY=true
    echo "  [✓] Loaded Tauri release signing key."
else
    echo "  [i] No signing key found in ~/.tauri/ (or env vars)."
    echo "      Building standard unsigned local bundle (.app & .dmg)."
fi

# ------------------------------------------------------------------------------
# 3. Installing Frontend Dependencies
# ------------------------------------------------------------------------------
echo ""
echo "==> [3/4] Installing UI dependencies..."
cd "$UI_DIR"
if [ -f "package-lock.json" ]; then
    npm ci
else
    npm install
fi

# ------------------------------------------------------------------------------
# 4. Building Launcher Application (.app & .dmg)
# ------------------------------------------------------------------------------
echo ""
echo "==> [4/4] Building Zircon Launcher (macOS .app and .dmg)..."
cd "$LAUNCHER_DIR"

if [ "$HAS_KEY" = true ]; then
    npx --yes @tauri-apps/cli build --bundles app,dmg
else
    # Disable updater artifact generation requirement so build succeeds without signing keys
    npx --yes @tauri-apps/cli build --bundles app,dmg --config '{"bundle":{"createUpdaterArtifacts":false}}'
fi

cd "$ROOT_DIR"

# ------------------------------------------------------------------------------
# Summary & Output
# ------------------------------------------------------------------------------
echo ""
echo "======================================================================"
echo "                   Build Finished Successfully!"
echo "======================================================================"
echo ""
echo "Your macOS launcher is ready in:"
if [ -d "$ROOT_DIR/target/release/bundle/macos" ]; then
    echo "  • Application (.app):"
    echo "    $ROOT_DIR/target/release/bundle/macos/Zircon.app"
fi
if [ -d "$ROOT_DIR/target/release/bundle/dmg" ]; then
    echo "  • Disk Image (.dmg):"
    find "$ROOT_DIR/target/release/bundle/dmg" -maxdepth 1 -name "*.dmg" -exec echo "    {}" \;
fi
echo ""
echo "To run the launcher directly:"
echo "  open \"$ROOT_DIR/target/release/bundle/macos/Zircon.app\""
echo ""
echo "Or drag Zircon.app into your /Applications folder."
echo "======================================================================"
