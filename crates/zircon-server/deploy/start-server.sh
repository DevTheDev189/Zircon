#!/bin/sh
# ===========================================================================
#  Zircon Server Manager Daemon - Linux/macOS Startup Script
# ===========================================================================
set -e
DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$DIR"

echo "==========================================================================="
echo "  Zircon Server Manager Daemon"
echo "==========================================================================="
echo "  Web Dashboard:       http://localhost:25564"
echo "  Minecraft Game Port: 25565"
echo ""
echo "  * On first startup, your initial Admin password will be printed below."
echo "  * Press Ctrl+C at any time to gracefully stop the server."
echo "==========================================================================="
echo ""

if [ ! -f "$DIR/zircon-server" ]; then
    echo "[ERROR] zircon-server executable not found in $DIR"
    exit 1
fi

chmod +x "$DIR/zircon-server" 2>/dev/null || true
exec "$DIR/zircon-server" "$@"
