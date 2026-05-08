# AGENTS.md

This file provides guidance to Codex (Codex.ai/code) when working with code in this repository.

## Project Overview

CAMPP is a cross-platform desktop application (similar to XAMPP) providing a complete local web development stack. It bundles Caddy (web server), PHP-FPM 8.3 (PHP runtime), MariaDB (database), and phpMyAdmin with no external dependencies after installation.

**Key Differentiators**: No admin permissions required (uses non-default ports), self-contained with bundled binaries, cross-platform (Windows/macOS/Linux).

## Development Commands

### Frontend (React + Vite)

```bash
# Install dependencies
pnpm install

# Start dev server (runs on http://localhost:1420)
pnpm dev

# Build for production
pnpm build

# Preview production build
pnpm preview
```

### Tauri (Desktop App)

```bash
# Run Tauri dev mode (starts both Vite and Tauri)
pnpm tauri dev

# Build desktop application
pnpm tauri build

# Tauri CLI commands (via package.json script)
pnpm tauri <command>
```

### Rust (src-tauri/)

```bash
# From src-tauri directory
cargo build                    # Build Rust backend
cargo test                     # Run Rust tests
cargo clippy                   # Lint Rust code
```

## Architecture

### Frontend Structure (React + TypeScript)

- **Entry Point**: `src/main.tsx` → `src/App.tsx`
- **Components**: `src/components/` (Dashboard, ServiceCard, FirstRunWizard, SettingsPanel)
- **State Management**: React hooks (useState, useEffect, useCallback) used directly in components
- **Types**: TypeScript interfaces in `src/types/services.ts`

Frontend communicates with Rust backend via Tauri IPC using `invoke()` from `@tauri-apps/api/core`.

### Backend Structure (Rust + Tauri)

The codebase uses a modular structure under `src-tauri/src/`:

```
src-tauri/src/
├── main.rs           # Binary entry point
├── lib.rs            # Library entry, Tauri commands, invoke_handler
├── process/          # Process spawn/control (manager.rs)
├── config/           # Config generation (generator.rs, ports.rs, settings.rs)
├── database/         # MariaDB initialization (mariadb.rs)
├── runtime/          # Binary download (downloader.rs) and locator (locator.rs)
└── commands.rs       # Tauri IPC command handlers
```

### Key Design Patterns

**Tauri Commands**: Defined in `lib.rs` or `commands.rs` with `#[tauri::command]` macro, exposed to frontend via `invoke_handler()`.

**Service Management**: The `ProcessManager` in `process/manager.rs` manages three core services:

- Caddy (web server) - port 8080
- PHP-FPM - port 9000 (internal)
- MariaDB - port 3306

**Config Generation**: Mustache templates in `templates/` are rendered by `config/generator.rs` to produce service configs.

**Runtime Binaries**: Downloaded on first-run to platform-specific data directory (`~/.campp/runtime/` on Unix, `C:\Users\<user>\.campp\runtime\` on Windows).

### App Data Directory Structure

```
~/.campp/
├── config/           # Generated configs (Caddyfile, php.ini, etc.)
├── mysql/data/       # MariaDB datadir
├── logs/             # Service logs
├── projects/         # User projects
└── runtime/          # Downloaded binaries
```

## Default Configuration

Ports are chosen to avoid conflicts with system services:

- Web Server: 8080
- PHP-FPM: 9000
- MariaDB: 3306
- phpMyAdmin: 8080/phpmyadmin

## Implementation Phases (from DEVELOPMENT_PLAN.md)

The project follows an 8-phase implementation plan:

1. **Project Foundation** - Tauri + React setup (complete)
2. **Runtime Download System** - First-run binary download wizard
3. **Process Manager** - Core process spawning and control
4. **Configuration Generation** - Dynamic config file generation
5. **MariaDB Initialization** - Database setup and credential management
6. **Dashboard UI** - Service control interface
7. **Basic Settings** - Port configuration, project folder selection
8. **MVP Release** - Cross-platform installers

## Important Files

| File                        | Purpose                                           |
| --------------------------- | ------------------------------------------------- |
| `src-tauri/tauri.conf.json` | Tauri app configuration (window, build settings)  |
| `src-tauri/src/lib.rs`      | Tauri commands and invoke_handler                 |
| `src-tauri/Cargo.toml`      | Rust dependencies                                 |
| `vite.config.ts`            | Frontend dev server (port 1420)                   |
| `DEVELOPMENT_PLAN.md`       | Full implementation plan and architecture details |
| `src/App.css`               | Global styles, theme variables, scrollbar styling |
| `src/components/Dashboard.tsx`   | Main dashboard, keyboard shortcuts (Ctrl+R, Ctrl+,, Esc) |
| `src/components/SettingsPanel.tsx` | Settings UI, ESC-to-close handler              |

## UX Conventions

### Keyboard Shortcuts
All shortcuts use `e.code` (physical key position) so they work regardless of keyboard language:

| Shortcut | Action | Implemented in |
| -------- | ------ | -------------- |
| `Ctrl/Cmd + R` | Restart all services | `Dashboard.tsx` |
| `Ctrl/Cmd + ,` | Toggle Settings panel | `Dashboard.tsx` |
| `Esc` | Dismiss toast / Close Settings | `Dashboard.tsx` + `SettingsPanel.tsx` |

### Toast Notifications
- Position: `bottom-center`, `bottom: 48px` (above status bar)
- Slide-up animation on appear
- Auto-dismiss after 4.2 s for success/error; stays until dismissed for info
- Tones: `info` (blue), `success` (green), `error` (red)
- Action variants: `start`, `restart`, `stop` override background color

### Scrollbar Styling
Custom scrollbar defined globally in `App.css` using CSS variables so it adapts to both light and dark themes automatically:
- Firefox: `scrollbar-width: thin` + `scrollbar-color`
- WebKit/Blink: `::-webkit-scrollbar` (6 px, rounded thumb, primary-tinted hover)

## Platform-Specific Notes

- **Windows**: WebView2 runtime required, builds use `windows_subsystem = "windows"` to prevent extra console
- **macOS**: Code signing required for distribution (see DEVELOPMENT_PLAN.md Phase 8)
- **Linux**: AppImage target for universal distribution
