import os
import numpy as np
from PIL import Image, ImageFilter, ImageDraw
import subprocess

def main():
    svg_content = """<?xml version="1.0" encoding="UTF-8"?>
<svg
   xmlns="http://www.w3.org/2000/svg"
   viewBox="0 0 64 64"
   width="64"
   height="64">
  <defs>
    <filter id="glow" x="-50%" y="-50%" width="200%" height="200%" color-interpolation-filters="sRGB">
      <feFlood flood-color="#0fbfbf" flood-opacity="0.45" result="flood" />
      <feGaussianBlur in="SourceGraphic" stdDeviation="2.0" result="blur" />
      <feComposite in="flood" in2="blur" operator="in" result="glow" />
      <feMerge>
        <feMergeNode in="glow" />
        <feMergeNode in="SourceGraphic" />
      </feMerge>
    </filter>
  </defs>
  <g filter="url(#glow)">
    <path
       d="M 10.73489,12.12731 v 11.06289 h 4.615739 l 3.522265,-4.46794 h 17.74052 v 5.34179 l -25.864055,16.18351 v 11.62513 h 42.511617 v -12.93668 h -4.618839 l -3.52485,4.47053 H 27.401056 v -3.15898 l 25.864054,-16.18351 v -11.93674 H 13.462372 Z"
       fill="#47d2c9"
       stroke="#47d2c9"
       stroke-width="0.5"
       stroke-linejoin="round" />
  </g>
</svg>
"""

    with open('svgs/zircon-icon.svg', 'w', encoding='utf-8') as f:
        f.write(svg_content)

    with open('crates/zircon-server/assets/web/zircon-icon.svg', 'w', encoding='utf-8') as f:
        f.write(svg_content)

    print("Updated SVG files successfully.")

    # Generate 1024x1024 master PNG
    size = 1024
    margin = 115
    glow_radius = 30
    glow_alpha = 0.55

    cx, cy = 38.666142, 146.230355
    raw_pts = [
        (17.401032, 126.35767),
        (17.401032, 126.35767 + 11.06289),
        (17.401032 + 4.466394 + 0.149345, 126.35767 + 11.06289),
        (17.401032 + 4.466394 + 0.149345 + 3.522265, 126.35767 + 11.06289 - 4.46794),
        (17.401032 + 4.466394 + 0.149345 + 3.522265 + 17.74052, 126.35767 + 11.06289 - 4.46794),
        (17.401032 + 4.466394 + 0.149345 + 3.522265 + 17.74052, 126.35767 + 11.06289 - 4.46794 + 5.34179),
        (17.401032 + 4.466394 + 0.149345 + 3.522265 + 17.74052 - 25.864055, 126.35767 + 11.06289 - 4.46794 + 5.34179 + 16.18351),
        (17.401032 + 4.466394 + 0.149345 + 3.522265 + 17.74052 - 25.864055, 126.35767 + 11.06289 - 4.46794 + 5.34179 + 16.18351 + 3.15898 + 8.46615),
        (17.401032 + 4.466394 + 0.149345 + 3.522265 + 17.74052 - 25.864055 + 42.511617, 126.35767 + 11.06289 - 4.46794 + 5.34179 + 16.18351 + 3.15898 + 8.46615),
        (17.401032 + 4.466394 + 0.149345 + 3.522265 + 17.74052 - 25.864055 + 42.511617, 126.35767 + 11.06289 - 4.46794 + 5.34179 + 16.18351 + 3.15898 + 8.46615 - 7.88428 - 0.58187 - 4.47053),
        (17.401032 + 4.466394 + 0.149345 + 3.522265 + 17.74052 - 25.864055 + 42.511617 - 4.466394 - 0.152445, 126.35767 + 11.06289 - 4.46794 + 5.34179 + 16.18351 + 3.15898 + 8.46615 - 7.88428 - 0.58187 - 4.47053),
        (17.401032 + 4.466394 + 0.149345 + 3.522265 + 17.74052 - 25.864055 + 42.511617 - 4.466394 - 0.152445 - 3.52485, 126.35767 + 11.06289 - 4.46794 + 5.34179 + 16.18351 + 3.15898 + 8.46615 - 7.88428 - 0.58187 - 4.47053 + 4.47053),
        (34.067198, 161.63252),
        (34.067198, 161.63252 - 3.15898),
        (59.931252, 138.2944),
        (59.931252, 138.2944 - 5.34179 - 6.59495),
        (20.128514, 126.35767)
    ]

    target_dim = size - 2 * margin
    scale = target_dim / 42.53022

    ss = 4
    canvas_ss = size * ss
    scaled_pts_ss = [((x - cx) * scale * ss + (canvas_ss / 2.0), (y - cy) * scale * ss + (canvas_ss / 2.0)) for x, y in raw_pts]

    solid_ss = Image.new('RGBA', (canvas_ss, canvas_ss), (0, 0, 0, 0))
    d_solid = ImageDraw.Draw(solid_ss)
    d_solid.polygon(scaled_pts_ss, fill=(71, 210, 201, 255))
    main_layer = solid_ss.resize((size, size), Image.Resampling.LANCZOS)

    glow_ss = Image.new('RGBA', (canvas_ss, canvas_ss), (0, 0, 0, 0))
    d_glow = ImageDraw.Draw(glow_ss)
    d_glow.polygon(scaled_pts_ss, fill=(15, 191, 191, int(255 * glow_alpha)))
    glow_base = glow_ss.resize((size, size), Image.Resampling.LANCZOS)
    glow_layer = glow_base.filter(ImageFilter.GaussianBlur(radius=glow_radius))

    master_icon = Image.new('RGBA', (size, size), (0, 0, 0, 0))
    master_icon.alpha_composite(glow_layer)
    master_icon.alpha_composite(main_layer)

    master_path = "crates/zircon-launcher/app-icon.png"
    master_icon.save(master_path)
    print(f"Saved master image {master_path} (size={size}x{size})")

    # Generate svgs/zircon-icon.png
    svg_png = master_icon.resize((256, 256), Image.Resampling.LANCZOS)
    svg_png.save("svgs/zircon-icon.png")

    # Generate website favicons
    fav_32 = master_icon.resize((32, 32), Image.Resampling.LANCZOS)
    if os.path.exists("website/assets/img"):
        fav_32.save("website/assets/img/favicon.png")
    if os.path.exists("cloudflare-upload/assets/img"):
        fav_32.save("cloudflare-upload/assets/img/favicon.png")

    # Run tauri icon generator
    print("Running npx @tauri-apps/cli icon...")
    cmd = ["npx", "@tauri-apps/cli", "icon", "app-icon.png"]
    subprocess.run(cmd, cwd="crates/zircon-launcher", shell=True, check=True)

    # Clean up master app-icon.png if desired
    if os.path.exists(master_path):
        os.remove(master_path)

    print("Icon generation complete.")

if __name__ == "__main__":
    main()
