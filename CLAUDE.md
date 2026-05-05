# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

CAMPP is a cross-platform desktop application (similar to XAMPP) providing a complete local web development stack. It bundles Caddy (web server), PHP-FPM 8.3 (PHP runtime), MariaDB (database), and phpMyAdmin with no external dependencies after installation.

**Key Differentiators**: No admin permissions required (uses non-default ports), self-contained with bundled binaries, cross-platform (Windows/macOS/Linux).

## Development Commands

### Frontend (React + Vite)
```bash
# Install dependencies
npm install

# Start dev server (runs on http://localhost:1420)
npm run dev

# Build for production
npm run build

# Preview production build
npm run preview
```

### Tauri (Desktop App)
```bash
# Run Tauri dev mode (starts both Vite and Tauri)
npm run tauri dev

# Build desktop application
npm run tauri build

# Tauri CLI commands (via package.json script)
npm run tauri <command>
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
- **State Management**: Custom hooks in `src/hooks/` (useServices.ts, useConfig.ts)
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
- MariaDB - port 3307

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
- MariaDB: 3307
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

| File | Purpose |
|------|---------|
| `src-tauri/tauri.conf.json` | Tauri app configuration (window, build settings) |
| `src-tauri/src/lib.rs` | Tauri commands and invoke_handler |
| `src-tauri/Cargo.toml` | Rust dependencies |
| `vite.config.ts` | Frontend dev server (port 1420) |
| `DEVELOPMENT_PLAN.md` | Full implementation plan and architecture details |

## Platform-Specific Notes

- **Windows**: WebView2 runtime required, builds use `windows_subsystem = "windows"` to prevent extra console
- **macOS**: Code signing required for distribution (see DEVELOPMENT_PLAN.md Phase 8)
- **Linux**: AppImage target for universal distribution
