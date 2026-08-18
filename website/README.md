# Zircon — Static Marketing Site

Informational site for the Zircon Minecraft server manager & companion launcher.
Designed to be hosted on **Cloudflare R2** (or any static host): pure HTML/CSS/vanilla
JS, no build step, no external CDN dependencies — every asset is relative.

## Structure

```
website/
├── index.html          # Landing page (features, security, server owners)
├── downloads.html      # Downloads page (Players / Server Owners tabs)
└── assets/
    ├── css/styles.css  # Design system (mirrors the app: GitHub-dark + teal #47d2c9)
    ├── js/main.js      # Nav toggle + download tabs (progressive enhancement)
    └── img/            # Brand assets (zircon-title.svg, zircon-icon, favicon)
```

## Deploying to Cloudflare R2

1. Create an R2 bucket and enable **public access** (custom domain recommended,
   e.g. `zircon.example.com`).
2. Upload the contents of `website/` to the bucket root — keep the
   `index.html`/`downloads.html`/`assets/` layout intact (relative paths depend on it).
3. If you use R2's static website feature, make sure **index.html** is the
   default root object.
4. For friendly URLs, either serve `downloads.html` directly or add a Cloudflare
   Worker redirect for `/downloads` → `/downloads.html`.

### Serving download binaries

Point the "Download" buttons at your real artifacts once they exist. In
`downloads.html` the buttons currently link to GitHub Releases — to host on R2
instead, upload the installers to e.g. `downloads/zircon-launcher-0.1.0.exe`
and update the `href` on the buttons.

## Keeping the design consistent

All colors, radii, and fonts are tokens at the top of `assets/css/styles.css`
(`:root`). They intentionally match the launcher UI: `#0d1117` background,
`#161b22` cards, `#47d2c9` teal accent, `Segoe UI` type.
