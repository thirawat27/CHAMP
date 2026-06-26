# AGENTS.md

Guidance for coding agents working in this repository.

## Project Overview

CHAMP is a Tauri desktop application for local web development. It manages a self-contained runtime stack without requiring administrator privileges.

Current core stack:

- Caddy web server on port `8080`
- PHP-FPM on port `9000`
- MySQL on port `3306`
- PostgreSQL on port `5432`
- phpMyAdmin at `/phpmyadmin`
- Adminer at `/adminer`
- Optional Node.js, Python, Go, Ruby runtimes
- Cloudflare Quick Tunnel based HTTPS Preview using `cloudflared`

All writable data is stored under the CHAMP user data directory or a configured portable directory.

## Development Commands

### Frontend

```bash
pnpm install
pnpm dev
pnpm build
pnpm preview
pnpm test:run
pnpm lint
pnpm format
```

### Tauri

```bash
pnpm tauri dev
pnpm tauri build
pnpm tauri <command>
```

### Rust Backend

```bash
cd src-tauri
cargo fmt
cargo build
cargo test
cargo clippy --all-targets -- -D warnings
```

## Architecture

### Frontend

- Entry point: `src/main.tsx`
- Main app: `src/App.tsx`
- Main dashboard: `src/components/Dashboard.tsx`
- Service cards: `src/components/ServiceCard.tsx`
- First-run setup: `src/components/FirstRunWizard.tsx`
- Settings: `src/components/SettingsPanel.tsx`
- Project scaffolding: `src/components/TemplateSelector.tsx`
- Status bar and metrics: `src/components/StatusBar.tsx`
- Translations: `src/i18n/translations.ts`
- Shared types: `src/types/services.ts`

Frontend communicates with Rust using Tauri `invoke()` from `@tauri-apps/api/core`.

### Backend

```text
src-tauri/src/
  lib.rs              Tauri builder and invoke handler
  commands.rs         IPC command handlers
  tunnel.rs           HTTPS Preview / cloudflared quick tunnel
  process/manager.rs  Caddy, PHP-FPM, MySQL, PostgreSQL process control
  config/             settings and port helpers
  runtime/            package catalog, downloader, binary locator
```

## Important IPC Commands

- Services: `start_service`, `stop_service`, `restart_service`
- Stack: `start_all_services`, `stop_all_services`, `restart_all_services`
- Status: `get_all_statuses`, `get_system_metrics`
- Settings: `get_settings`, `save_settings`, `validate_settings`, `check_ports`
- Runtime: `get_available_packages_cmd`, `refresh_runtime_catalog`, `download_runtime_with_packages`, `download_runtime_with_skip`
- PHP: `get_installed_php_versions`, `switch_php_version`, `download_php_version`
- Projects: `create_project_template`
- Folders/terminal: `open_folder`, `open_project_terminal`
- HTTPS Preview: `start_https_tunnel`, `stop_https_tunnel`, `get_https_tunnel_status`

## Runtime Catalog

Runtime metadata is defined in `src-tauri/runtime-config.json` and can be overridden with user runtime config files. The catalog currently includes:

- Caddy
- PHP stable/LTS/EOL versions
- MySQL stable/LTS
- PostgreSQL stable branches
- phpMyAdmin and Adminer
- Node.js stable/LTS
- Python
- Go
- Ruby

The app can refresh runtime metadata from upstream release sources.

## App Data Layout

```text
CHAMP/
  config/
  logs/
  mysql/data/
  postgresql/data/
  projects/
  runtime/
```

Portable mode can be controlled with `CHAMP_DATA_DIR`, `CHAMP_PORTABLE_DIR`, `CHAMP_PORTABLE`, or portable marker files.

## UX Conventions

- Keep Dashboard actions practical and compact.
- Do not add duplicate quick actions when an equivalent primary action already exists.
- Use existing bilingual translation keys. Add Thai and English text together.
- All common workflows must work from keyboard shortcuts or visible buttons.
- Use lucide-react icons where available.
- Do not expose HTTPS Preview Open/Copy until the generated URL is confirmed ready.

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

## Testing Expectations

For frontend/UI changes:

```bash
pnpm lint
pnpm build
pnpm test:run
```

For Rust/backend changes:

```bash
cd src-tauri
cargo fmt
cargo test
cargo clippy --all-targets -- -D warnings
```

On Windows, `cargo test` can fail if `src-tauri/target/debug/champ.exe` is still running. Stop only that debug process before retrying; do not reset the working tree.
