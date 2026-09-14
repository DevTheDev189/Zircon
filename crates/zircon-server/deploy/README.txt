===============================================================================
  Zircon Server Manager
  Modern, Self-Hosted Minecraft Server Management & Auto-Sync Engine
===============================================================================

Welcome to Zircon Server Manager!

Zircon is a lightweight, single-port daemon that supervises Minecraft server
instances, serves an integrated browser administration dashboard, and automatically
synchronizes exact mods, shaders, and configs to players using the Zircon Launcher.

-------------------------------------------------------------------------------
1. QUICK START
-------------------------------------------------------------------------------

Windows:
  1. Double-click `start-server.bat` (or run `zircon-server.exe` in PowerShell/CMD).
  2. On first run, a secure initial administrator password will be printed
     directly to the console window. Note this down!
  3. Open your browser and navigate to:
     http://localhost:25564
  4. Log in using username `admin` and the generated password.
  5. In the dashboard, create a server instance, select your Minecraft version
     and mod loader (Fabric, Forge, NeoForge, Quilt, or Vanilla), browse & add mods,
     and click START.

Linux:
  1. Make the script executable and launch:
     chmod +x start-server.sh zircon-server
     ./start-server.sh
  2. Note the initial admin password printed to the terminal.
  3. Open http://localhost:25564 in your web browser.
  4. (Optional) For automated background operation, install the included systemd
     unit: `zircon-server.service`.

-------------------------------------------------------------------------------
2. NETWORKING & PORTS
-------------------------------------------------------------------------------

  * Port 25565 (Default Public Port):
    Zircon's TCP multiplexer handles both Minecraft client game connections and
    HTTP manifest/sync traffic over this single port. Forward port 25565 on your router
    to allow outside players to join.

  * Port 25564 (Web Admin Dashboard):
    The web console, instance supervisor, mod browser, and file editor run here.
    (Keep this private or front it with a secure TLS reverse proxy like Caddy / Nginx).

-------------------------------------------------------------------------------
3. DATA DIRECTORY LAYOUT (Created in server-data/)
-------------------------------------------------------------------------------

  server-data/
    ├── config.json          Global wrapper configuration (ports, title, defaults)
    ├── users.json           Admin authentication accounts & hashed credentials
    ├── server_signing.key   BOM signing cryptographic key
    ├── instances/           Isolated Minecraft server directories & worlds
    ├── mods/                Central repository for server-side mods & BOM sync
    └── backups/             Automated LZ4 snapshots of instances and world saves

-------------------------------------------------------------------------------
4. OPTIONAL CLI FLAGS
-------------------------------------------------------------------------------

  --enable-startup   Install Zircon as a Windows startup background service
  --disable-startup  Remove Zircon from Windows startup
  --startup-status   Display current auto-start status

-------------------------------------------------------------------------------
5. NEED HELP?
-------------------------------------------------------------------------------

  * Documentation & Guides: https://zirconmc.net/docs.html
  * Website:               https://zirconmc.net
  * GitHub Issues:         https://github.com/DevTheDev189/Zircon/issues

Licensed under the Business Source License 1.1 (BSL 1.1).
Free for personal, homelab, and community servers.
