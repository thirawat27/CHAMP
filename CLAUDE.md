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

| File | Purpose |
|------|---------|
| `src-tauri/tauri.conf.json` | Tauri app configuration (window, build settings) |
| `src-tauri/src/lib.rs` | Tauri commands and invoke_handler |
| `src-tauri/Cargo.toml` | Rust dependencies |
| `vite.config.ts` | Frontend dev server (port 1420) |
| `src/App.css` | Global styles, theme variables, scrollbar styling |
| `src/components/Dashboard.tsx` | Main dashboard, keyboard shortcuts |
| `src/components/HelpModal.tsx` | Keyboard shortcuts help modal |
| `src/components/SettingsPanel.tsx` | Settings UI, ESC-to-close handler |
| `src/utils/clipboard.ts` | Clipboard utilities for error reporting |
| `KEYBOARD_SHORTCUTS.md` | Complete keyboard shortcuts documentation |
| `DX_FEATURES.md` | Developer experience features documentation |

## UX Conventions

### Keyboard Shortcuts
All shortcuts use `e.code` (physical key position) so they work regardless of keyboard language:

| Shortcut | Action | Implemented in |
|----------|--------|----------------|
| `Ctrl/Cmd + S` | Start all services | `Dashboard.tsx` |
| `Ctrl/Cmd + R` | Restart all services | `Dashboard.tsx` |
| `Ctrl/Cmd + X` | Stop all services | `Dashboard.tsx` |
| `Ctrl/Cmd + W` | Open website (localhost) | `Dashboard.tsx` |
| `Ctrl/Cmd + D` | Open database tool (phpMyAdmin/Adminer) | `Dashboard.tsx` |
| `Ctrl/Cmd + O` | Open projects folder | `Dashboard.tsx` |
| `Ctrl/Cmd + L` | Open logs folder | `Dashboard.tsx` |
| `Ctrl/Cmd + ,` | Toggle Settings panel | `Dashboard.tsx` |
| `?` | Show keyboard shortcuts help | `Dashboard.tsx` + `HelpModal.tsx` |
| `Esc` | Dismiss toast / Close modal | `Dashboard.tsx` + `SettingsPanel.tsx` + `HelpModal.tsx` |

See [KEYBOARD_SHORTCUTS.md](KEYBOARD_SHORTCUTS.md) for complete documentation.

### Developer Experience (DX)
CHAMP is designed with developer experience as a priority:
- **Keyboard-First**: All actions accessible via shortcuts
- **Visual Feedback**: Toast notifications, loading states, status indicators
- **Error Transparency**: Clear error messages with context
- **Help System**: In-app help modal (`?` key)
- **Accessibility**: ARIA labels, keyboard navigation, screen reader support

See [DX_FEATURES.md](DX_FEATURES.md) for complete DX documentation.

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
