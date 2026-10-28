# AGENTS.md

Guidance for coding agents working in this repository.

## Project Overview

CHAMP is a Tauri desktop application for local web development. It manages a self-contained runtime stack without requiring administrator privileges.

Core stack:

- Caddy web server on port `8080`
- PHP-FPM on port `9000`
- MySQL on port `3306` (used with phpMyAdmin at `/phpmyadmin`)
- PostgreSQL on port `5432` (used with Adminer at `/adminer`)
- Cloudflare Quick Tunnel based HTTPS Preview using `cloudflared`

Built-in features:

- Log Doctor: offline diagnosis of service failures (knowledge base + optional Model2Vec model)
- AI agent connector: local MCP server for coding agents (off by default)
- Self-updater: signed updates from GitHub Releases via `tauri-plugin-updater`
- In-app bilingual guide (F1)

Optional Node.js / Python / Go / Ruby runtimes and project templates were removed in 1.4.0. Do not reintroduce them.

All writable data is stored under the CHAMP user data directory or a configured portable directory.

## Development Commands

### Frontend

```bash
pnpm install
pnpm dev          # in a plain browser, src/dev/mockTauri.ts fakes the backend
pnpm build        # tsc (TypeScript 7) + vite build
pnpm test:run
pnpm lint         # oxlint (config: .oxlintrc.json)
pnpm format
```

### Tauri

```bash
pnpm tauri dev
pnpm tauri build
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

- Entry point: `src/main.tsx` (font imports, dev-only IPC mock)
- Main app: `src/App.tsx`
- Dashboard: `src/components/Dashboard.tsx` (request path, service grid, side rail, update banner)
- Service cards: `src/components/ServiceCard.tsx`
- First-run setup: `src/components/FirstRunWizard.tsx`, `src/components/PackageSelector.tsx`
- Settings drawer: `src/components/SettingsPanel.tsx` + `src/components/settings/*`
- Log Doctor: `src/components/LogDoctorModal.tsx`, `src/hooks/useLogDoctorModel.ts`
- Guide: `src/components/GuideModal.tsx`, content in `src/manual/content.ts`
- Update state: `src/stores/updateStore.ts`
- Translations: `src/i18n/translations.ts`
- Shared types: `src/types/services.ts`
- Styles: `src/App.css` (plain CSS design system; tokens on `:root`, dark mode via `prefers-color-scheme`)

Frontend communicates with Rust using Tauri `invoke()` from `@tauri-apps/api/core`. Use `listenIfTauri` from `src/utils/tauri.ts` for events.

### Backend

```text
src-tauri/src/
  lib.rs              Tauri builder, plugins, tray, invoke handler
  commands.rs         IPC command handlers (process work runs via run_with_manager on the blocking pool)
  tunnel.rs           HTTPS Preview / cloudflared quick tunnel
  doctor/             Log Doctor knowledge base matching + Model2Vec runtime (doctor-knowledge.json)
  agent/              MCP server for AI agents (hyper, 127.0.0.1, bearer token)
  process/manager/    Caddy, PHP-FPM, MySQL, PostgreSQL process control
  config/             settings and port helpers
  runtime/            package catalog, PHP feeds (php_catalog.rs), downloader, binary locator
```

## Important IPC Commands

- Services: `start_service`, `stop_service`, `restart_service`
- Stack: `start_all_services`, `stop_all_services`, `restart_all_services`
- Status: `get_all_statuses`, `get_system_metrics`
- Settings: `get_settings`, `save_settings`, `validate_settings`, `check_ports`
- Runtime: `get_available_packages_cmd`, `refresh_runtime_catalog`, `refresh_php_catalog`, `download_runtime_with_packages`, `download_runtime_with_skip`
- PHP: `get_installed_php_versions`, `switch_php_version`, `download_php_version` (installs/updates to the newest patch, does not switch)
- Folders/terminal: `open_folder`, `open_project_terminal` (restricted to the CHAMP data folder)
- HTTPS Preview: `start_https_tunnel`, `stop_https_tunnel`, `get_https_tunnel_status`
- Log Doctor: `diagnose_service`, `get_log_doctor_status`, `download_log_doctor_model`, `remove_log_doctor_model`
- Agent connector: `get_agent_connector_status`, `configure_agent_connector`, `regenerate_agent_token`
- Updates: `prepare_for_update` (the update itself uses the updater plugin from the frontend)

## Runtime Catalog

Runtime metadata is defined in `src-tauri/runtime-config.json` and can be overridden by `config/runtime-config.json` in the data folder. The catalog includes Caddy, PHP, MySQL, PostgreSQL, phpMyAdmin, and Adminer.

PHP entries are rebuilt from official feeds (`php.net/releases/states.php`, `windows.php.net/.../releases.json` with SHA-256, the static-php-cli listing). Never guess PHP download URLs; Windows builds can carry rebuild suffixes such as `8.5.12-1`.

## App Data Layout

```text
CHAMP/
  config/        settings.json, agent-token, runtime-config.json override
  logs/
  models/        Log Doctor model (optional)
  mysql/data/
  postgresql/data/
  projects/
  runtime/
```

Portable mode can be controlled with `CHAMP_DATA_DIR`, `CHAMP_PORTABLE_DIR`, `CHAMP_PORTABLE`, or portable marker files.

## UX Conventions

- Keep Dashboard actions practical and compact. Do not add duplicate quick actions when an equivalent primary action already exists.
- Use existing bilingual translation keys. Add Thai and English text together.
- All common workflows must work from keyboard shortcuts or visible buttons.
- Use lucide-react icons.
- Do not expose HTTPS Preview Open/Copy until the generated URL is confirmed ready.
- When a feature changes, update `src/manual/content.ts` (the in-app guide) in both languages.
- Settings fields that have dedicated commands (language, sound, agent connector) are preserved by `save_settings`; follow that pattern for new ones.

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
- `F1`: open the guide.
- `?`: show shortcut help.
- `Esc`: dismiss a toast or close the active modal.

## Security

- Capabilities live in `src-tauri/capabilities/default.json`; keep them least-privilege.
- The CSP in `tauri.conf.json` disallows inline scripts in production.
- The agent connector must stay localhost-only, token-protected, and reject requests with an `Origin` header.
- The updater public key is in `tauri.conf.json`; the private key is a GitHub secret (`TAURI_SIGNING_PRIVATE_KEY`) and must never be committed.

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

Live/network tests are `#[ignore]`d (`cargo test -- --ignored`). On Windows, `cargo test` can fail if `src-tauri/target/debug/champ.exe` is still running. Stop only that debug process before retrying; do not reset the working tree.
