<h4 align="right">English | <strong><a href="README_cn.md">简体中文</a></strong></h4>

<p align="center">
  <a href="https://github.com/yuc1013/rsdish-cli" target="_blank"><img src="assets/parfait_gpt.png" width="120" /></a>
  <h1 align="center">rsdish</h1>
  <div align="center">
    <a href="LICENSE" target="_blank">
      <img alt="GNU General Public License v3.0" src="https://img.shields.io/badge/License-GPLv3-blue.svg">
    </a>
  </div>
  <div align="center">A multi-functional sync tool for home storage.</div>
</p>

A multi-functional sync tool for domestic storages.

## Highlights
- ✅ Backup private data once — keep it synced forever;
- 🛡️ Designed for home drives that may go offline;
- 🔗 Unify scattered data across multiple storage devices via symbolic links;
- 🖥️ Supports Linux, Windows, and macOS;
- 🌐 Optional local web UI — scan, sync and edit cabinets from a browser;

## Installation

Add `rsdish` to `PATH`;

## How it works
[<img src="assets/how_it_works.png" width="40%" alt="How_it_works">](#)

## Web UI

The same library also ships a local web UI, so scanning, group actions and
cabinet editing can be done from a browser:

```bash
rsdish web                        # http://127.0.0.1:8787
rsdish web --port 9000 --open     # pick a port and open the browser

# or use the standalone binary
rsdish-webui --host 127.0.0.1 --port 9000
```

The frontend is compiled into the binary: no node, no build step, nothing else
to install next to it.

| Tab | What it does |
| --- | --- |
| Dashboard | every scanned storage with its cabinets and memberships, plus scan issues |
| Groups | members of each group, with preview / sync / link / clean / exec |
| Cabinets | init a config, join groups, edit memberships, apply the main/mirror/gate presets |
| Settings | the config file path and the custom storages list |
| Logs | live view of the same events the CLI logs |

Destructive actions show a read-only preview first: what would be copied, linked
or removed, per member, before anything touches the disk. The preview is computed
by the very same functions the CLI uses, so both stay in sync.

The server binds to `127.0.0.1` by default and has no authentication: keep it on
loopback, or put a reverse proxy in front of it if you expose it.

## Configuration

```toml
# rsdish.config.toml

# macOS: ~/Library/Application Support/<app>/<config_name>.toml
# Linux: ~/.config/<app>/<config_name>.toml
# Windows: %APPDATA%\<app>\<config_name>.toml

# Tip: Run `rsdish config` to print current config path

custom_storages = ["<STG_ABS_PATH>(s)"]
```

```toml
# rsdish.cabinet.toml

# For example:
# Storage_SSD/
# ├── Cabinet_Book/
# │   ├── book1.epub
# │   ├── book2.pdf
# │   ├── .srcignore
# │   └── rsdish.cabinet.toml
# └── Cabinet_Movie/
#     ├── movie1.mp4
#     └── rsdish.cabinet.toml

# Tip: Run `rsdish cabinet init` to generate an empty config file;
# Run `rsdish cabinet join` to generate a random membership.

[[memberships]]
group_uuid = "0199ebad-44ad-78a2-baad-c56a052e33ac"
priority = 0   # Higher number = higher priority (higher can override lower)

[memberships.src_option]
enable = false

[memberships.dst_option]
enable = false
cover_level = 0  # Enum: 0=DontCover, 1=HigherCover
save_level  = 0  # Enum: 0=DontSave, 1=SaveHigher, 2=SaveHigherEqual, 3=SaveAll

[memberships.link_option]
enable = false
save_level = 0
```

```ignore
# .srcignore
# The syntax of .srcignore is largely the same as that of .gitignore.
```
## NOTE

⚠️ On Windows, `rsdish link` must be run with administrator privileges, or Developer Mode must be enabled on Windows 10 for proper operation.

## License

This project is licensed under the [GNU General Public License v3.0 (GPLv3)](LICENSE).
