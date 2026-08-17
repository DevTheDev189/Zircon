# Zircon Server (distribution)

A standalone copy of the Zircon **server wrapper** — the daemon that hosts
Minecraft instances, the admin web UI, the TCP multiplexer, and the idle
shutdown / auto-wake feature.

## Requirements

- **Windows** (x64)
- **Java 17 or 21** installed and on `PATH` — required to run the Minecraft
  instances themselves.

## Run

Double-click **`run-server.bat`**, or from a terminal inside this folder:

```
.\run-server.bat
```

Leave that window open — it is the server's console. When it's up, open the
admin dashboard at **http://localhost:25564** (also reachable via the public
Minecraft port, `http://localhost:25565`).

## First run

The first time you start it, the wrapper:

1. Creates `server-data/` here (with `config.json`, `jwt-secret.key`, ...).
2. Generates a **fresh admin password** and prints it to the console.
3. Registers the Minecraft world/mod folders under `server-data/instances`.

Because `server-data/` is created fresh, nothing from the machine this was
built on is carried over — this is a clean install.

## What's inside

| Path | Purpose |
|---|---|
| `zircon-server.exe` | The server wrapper daemon. |
| `run-server.bat` | Starts the wrapper and points it at the local `server-data/`. |
| `server-data/` | Created on first run — worlds, instances, config, backups. |

## Note

This is a *wrapper* host, not a full installer. There is intentionally no MSI:
the server runs from its own folder with its data beside it (the `.bat` sets
`MC_MANAGER_DATA_DIR` accordingly). To move a deployment, copy the whole
`zircon-server/` folder (or re-zip it) — do not copy a single `.exe`.
