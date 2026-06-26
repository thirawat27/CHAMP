# CHAMP By Thirawat27

CHAMP is a cross-platform desktop app for local web development. It provides a self-contained stack with Caddy, PHP-FPM, MySQL or PostgreSQL, phpMyAdmin or Adminer, optional language runtimes, and a public HTTPS preview tunnel for temporary testing.

CHAMP is designed to run without administrator privileges. Runtime binaries, databases, logs, generated configs, and projects live in the user's writable CHAMP data directory.

## Features

- One-click stack control for Caddy, PHP-FPM, and the selected database service.
- Database tool selection: phpMyAdmin for MySQL or Adminer for PostgreSQL.
- PHP version management with install and switch support.
- Optional runtimes for Node.js, Python, Go, and Ruby.
- Runtime catalog refresh from upstream release metadata.
- Project templates for Static, PHP, Node, Python, Go, and Ruby projects.
- HTTPS Preview using Cloudflare Quick Tunnel with a free temporary `trycloudflare.com` URL.
- Keyboard-first workflow, tray support, toast feedback, and bilingual Thai/English UI.
- System metrics and runtime/service status monitoring.

## Current Runtime Catalog

The selected defaults are defined in [src-tauri/runtime-config.json](src-tauri/runtime-config.json).

| Component   | Default          | Alternatives                                   |
| ----------- | ---------------- | ---------------------------------------------- |
| Caddy       | 2.11.4           | Catalog refresh can replace with latest stable |
| PHP         | 8.5.7            | 8.4, 8.3 LTS, 8.2, and legacy EOL versions     |
| MySQL       | 9.7.1            | MySQL 8.4 LTS                                  |
| PostgreSQL  | 18.4             | 17.10, 16.14                                   |
| Database UI | phpMyAdmin 5.2.3 | Adminer 5.4.2                                  |
| Node.js     | 24.18.0 LTS      | 26 stable, 22 LTS, 20 LTS                      |
| Python      | 3.14.6           | 3.13.14                                        |
| Go          | 1.26.4           | 1.25.11                                        |
| Ruby        | 4.0.5            | 3.4.9, 3.3.7                                   |

## Default URLs

| Service    | URL                                |
| ---------- | ---------------------------------- |
| Website    | `http://localhost:8080`            |
| phpMyAdmin | `http://localhost:8080/phpmyadmin` |
| Adminer    | `http://localhost:8080/adminer`    |
| MySQL      | `mysql://127.0.0.1:3306`           |
| PostgreSQL | `postgresql://127.0.0.1:5432`      |
| PHP-FPM    | `tcp://127.0.0.1:9000`             |

If a configured port is busy, CHAMP selects an available fallback port and shows it in the UI.

## HTTPS Preview

The Dashboard includes an HTTPS Preview panel. Press **Start HTTPS** to create a free temporary public URL.

How it works:

1. CHAMP starts the selected local stack if Caddy is not already running.
2. CHAMP installs `cloudflared` into the runtime folder if it is missing.
3. CHAMP runs `cloudflared tunnel --url http://127.0.0.1:<web-port>`.
4. CHAMP waits until the generated `trycloudflare.com` domain resolves and responds over HTTPS.
5. Open and Copy buttons are enabled only after the public URL is ready.

Cloudflare Quick Tunnels are intended for testing and development only. The generated domain is temporary and may change each time the tunnel is restarted.

## Installation

Download a release from GitHub Releases when available:

- Windows: `.msi` or `.exe`
- macOS: `.dmg`
- Linux: `.AppImage` or `.deb`

On first launch, the setup wizard checks dependencies, lets you choose packages, downloads runtimes, and creates the local CHAMP data directories.

## Usage

1. Launch CHAMP.
2. Choose the database tool in Settings if needed.
3. Press **Start** to run the selected stack.
4. Open **Website**, **phpMyAdmin/Adminer**, **Projects**, or **Runtime** from the Dashboard.
5. Use **More > Terminal** to open a terminal with installed runtimes injected into `PATH`.
6. Use **Create Project** to scaffold a starter project.
7. Use **Start HTTPS** to test or share the current local website over a temporary HTTPS URL.

## Keyboard Shortcuts

- `Ctrl/Cmd + S`: start the selected stack.
- `Ctrl/Cmd + R`: restart the selected stack.
- `Ctrl/Cmd + X`: stop the selected stack.
- `Ctrl/Cmd + W`: open the local website.
- `Ctrl/Cmd + D`: open phpMyAdmin or Adminer.
- `Ctrl/Cmd + O`: open the projects folder.
- `Ctrl/Cmd + L`: open the logs folder.
- `Ctrl/Cmd + T`: open a terminal with CHAMP runtimes on `PATH`.
- `Ctrl/Cmd + ,`: toggle Settings.
- `?`: show shortcut help.
- `Esc`: dismiss a toast or close the active modal.

## Developer Experience

- Dashboard status and HTTPS tunnel status refresh together to reduce duplicate IPC work.
- System metrics polling is throttled and pauses while the app is hidden.
- HTTPS Preview waits for DNS and HTTPS readiness before showing Open or Copy.
- Project terminals include installed runtime paths automatically.
- Toasts and service cards expose actionable pending, success, and error states.

## Development

### Prerequisites

- Node.js 18+ and pnpm 10+
- Rust stable
- Tauri platform dependencies
- Windows: WebView2 Runtime
- macOS: Xcode Command Line Tools
- Linux: WebKitGTK dependencies for Tauri

### Commands

```bash
pnpm install
pnpm dev
pnpm build
pnpm test:run
pnpm lint
pnpm tauri dev
pnpm tauri build
```

Rust backend commands:

```bash
cd src-tauri
cargo fmt
cargo test
cargo clippy --all-targets -- -D warnings
```

## Architecture

```text
src/
  App.tsx
  App.css
  components/
    Dashboard.tsx
    ServiceCard.tsx
    FirstRunWizard.tsx
    SettingsPanel.tsx
    StatusBar.tsx
    TemplateSelector.tsx
  i18n/translations.ts
  stores/languageStore.ts
  types/services.ts

src-tauri/src/
  lib.rs
  commands.rs
  tunnel.rs
  process/manager.rs
  config/
  runtime/
```

Key backend responsibilities:

- `process/manager.rs`: starts, stops, and monitors Caddy, PHP-FPM, MySQL, and PostgreSQL.
- `runtime/downloader.rs`: downloads and installs runtime archives.
- `runtime/packages.rs`: loads and refreshes the runtime catalog.
- `runtime/locator.rs`: resolves installed runtime binaries and CHAMP data paths.
- `commands.rs`: exposes Tauri IPC commands to the React frontend.
- `tunnel.rs`: installs and manages Cloudflare Quick Tunnel for HTTPS Preview.

## Data Directories

Default user data directory:

```text
%LOCALAPPDATA%\CHAMP\       Windows
~/Library/Application Support/CHAMP/  macOS
~/.local/share/CHAMP/       Linux, depending on XDG settings
```

Directory layout:

```text
CHAMP/
  config/
  logs/
  mysql/data/
  postgresql/data/
  projects/
  runtime/
```

Portable mode is supported through `CHAMP_DATA_DIR`, `CHAMP_PORTABLE_DIR`, `CHAMP_PORTABLE`, or portable marker files.

## Runtime Customization

See [src-tauri/CUSTOM_VERSIONS.md](src-tauri/CUSTOM_VERSIONS.md).

## Testing

See [tests/README.md](tests/README.md).

Current verification set:

```bash
pnpm lint
pnpm build
pnpm test:run
cd src-tauri
cargo test
cargo clippy --all-targets -- -D warnings
```

## Troubleshooting

### HTTPS Preview shows DNS_PROBE_FINISHED_NXDOMAIN

This means the generated Cloudflare domain was opened before DNS was ready, or the quick tunnel expired. Current CHAMP builds keep Open/Copy disabled until the URL responds over HTTPS. If it still happens, stop HTTPS and start it again to generate a fresh tunnel.

### Services do not start

Check service logs under `logs/`, verify runtime installation, and review fallback port messages in the Dashboard.

### Runtime download fails

Use Settings or the setup wizard to refresh the runtime catalog. Advanced users can override runtime URLs with `runtime-config.json`.

## License

MIT. See [LICENSE](LICENSE).

## Acknowledgments

- Original project: [CAMPP](https://github.com/KarnYong/campp)
- [Tauri](https://tauri.app/)
- [React](https://react.dev/)
- [Caddy](https://caddyserver.com/)
- [PHP](https://www.php.net/)
- [MySQL](https://www.mysql.com/)
- [PostgreSQL](https://www.postgresql.org/)
- [phpMyAdmin](https://www.phpmyadmin.net/)
- [Adminer](https://www.adminer.org/)
- [Cloudflare Tunnel](https://developers.cloudflare.com/cloudflare-one/networks/connectors/cloudflare-tunnel/)
