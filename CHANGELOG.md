# Changelog

All notable changes to CHAMP. Versions follow [Semantic Versioning](https://semver.org/);
the format follows [Keep a Changelog](https://keepachangelog.com/).

## [2.1.0] - 2026-11-23

Add-on services, support for any PHP framework, and an AI agent connector that can
drive the whole app.

### Added

- **Add-on services** (`Ctrl/Cmd + E`), optional and off by default. Each one is a
  pinned download, checked against its SHA-256 before install, listens on 127.0.0.1
  only, and starts and stops with the stack:
  - **Redis**: cache, queues, and sessions. Downloaded on Windows; on macOS/Linux
    CHAMP uses a system install (`brew install redis`, `apt install redis-server`).
  - **Mailpit**: catches every email your app sends, so nothing is really delivered.
    PHP's `mail()` is routed to it automatically. The web inbox shows a message
    count (`Ctrl/Cmd + M`).
  - **Meilisearch**: fast, typo-tolerant search, with a search preview in the browser.
  - **Centrifugo**: realtime messaging over WebSocket, with an admin UI.
  - **RustFS**: S3-compatible storage for file uploads, with a web console. There is
    no Intel Mac build.
- **Your own services**: register any program (for example Valkey or another Redis
  version) with a port and arguments. CHAMP starts and stops it with the stack and
  waits for its port, but never downloads or verifies it, so it is labeled
  user-managed.
- **Project background tasks**: long-running PHP commands such as
  `artisan queue:work` or `bin/console messenger:consume`. They run with CHAMP's PHP
  in the project folder, can start with the stack, and always stop with it.
- **Project domains**: every project folder is also served at
  `http://<folder>.localhost:8080/`, from `public/` when it has one. This needs no
  hosts file, no administrator rights, and no restart for new folders.
- **Support for any PHP framework**: CHAMP detects the framework and the required
  PHP version from `composer.json`, and gives ready-to-paste configuration for
  Laravel, Symfony, WordPress, CodeIgniter 4, CakePHP, a generic DSN form (Slim, Yii,
  Laminas, plain PHP), or the `CHAMP_*` variables every PHP request already receives.
- **Xdebug** on/off in Settings → PHP, in trigger mode so pages stay fast. You add
  the Xdebug file that matches your PHP. Windows only: CHAMP's static PHP on
  macOS/Linux cannot load extensions.
- **AI agent connector**: agents can now do everything the Settings window does.
  - New read tools: `get_project`, `get_env_profiles`, `list_tables`,
    `describe_table`, `get_addons`, `read_addon_log`, `list_mail`, `read_mail` (with
    the links inside the email), `get_php_info`, `list_workers`, `get_settings`,
    `check_ports`, `list_php_versions`, `get_system_info`.
  - New write tools: `start_addon`, `stop_addon`, `set_addon_enabled`,
    `set_addon_port`, `install_addon`, `switch_php_version`, `install_php_version`,
    `set_xdebug`, `switch_database_engine`, `switch_database_tool`,
    `update_settings`, `start_worker`, `stop_worker`.
  - MCP resources (`champ://status`, `champ://logs/{service}`,
    `champ://projects/{name}`, …) and prompts: connect a project, diagnose a
    problem, test an email flow.
  - Supports MCP protocol version `2025-11-25`.

### Changed

- The Add-on services window has three tabs: Services (a grid of cards),
  Project configuration, and Background tasks. It remembers the last tab and no
  longer needs long scrolling.
- Settings is wider and has a category menu: General, Ports, PHP, Database, Add-on
  services, and AI and Log Doctor. One category shows at a time, the last one is
  remembered, and **Open Settings** in Log Doctor goes straight to Ports.
- `get_environment` and `list_projects` now report project domains and frameworks.
- The in-app guide (F1), README, and AGENTS.md cover every new feature in Thai and
  English.

### Fixed

- macOS builds: a missing import in the terminal launcher stopped CHAMP 2.0.0 from
  compiling on macOS, so 2.0.0 shipped no macOS installers. 2.1.0 builds again for
  Apple Silicon and Intel.
- Release builds: the Linux release build no longer reuses a build cache made on a
  newer Ubuntu, which failed to link (`__isoc23_sscanf`).

### Security

- Agents can start and stop custom add-ons and background tasks, but only you can
  create or edit them. The ngrok authtoken and the connector's own settings also stay
  with you.
- Schema tools run fixed catalog queries with validated names. No agent tool deletes
  data or runs arbitrary SQL or shell commands.
- A background task's first argument must be a script inside the project; no shell
  is involved.

## [2.0.0] - 2026-11-07

A redesigned CHAMP with offline diagnosis, an AI agent connector, and signed
self-updates.

### Added

- **Log Doctor**: explains why a service failed, using 28 known failure patterns
  (busy ports, missing VC++ runtime, corrupted data, locks, permissions, …). It gives
  fix steps, the matching log line, and one-click actions. An offline AI model
  (potion-base-8M, about 30 MB) downloads on first start and recognises reworded
  errors; you can remove it.
- **AI agent connector (MCP)**: a local server for Claude Code, Codex, Cursor,
  VS Code, and Claude Desktop, with tools for status, environment, logs, Log Doctor,
  databases, HTTPS Preview, and service control. It accepts connections from
  127.0.0.1 only, requires a token, refuses browser requests, and has an optional
  read-only mode.
- **Signed self-updates** from GitHub Releases, with a daily check.
- **HTTPS Preview**: a public HTTPS link to your local site through ngrok.
- **Independent database choices**: MySQL or PostgreSQL, and phpMyAdmin or Adminer,
  in any combination.
- **Bilingual in-app guide** (Thai and English, F1) and keyboard shortcuts for every
  common task.
- Parallel downloads with combined progress in the setup wizard.

### Changed

- New interface: request path view, service cards, settings drawer, and icons.
- PHP always installs the newest patch from the official php.net and
  windows.php.net feeds; Windows downloads are checked against SHA-256.
- Databases shut down gracefully instead of being killed. PHP updates and database
  setup are staged, so a failure leaves the working install in place.
- PHP on Windows stays up past 500 requests.
- Package choices apply at once, and your configured ports are kept when CHAMP has
  to use a fallback port.

### Removed

- The optional Node.js, Python, Go, and Ruby runtimes and project templates.

### Security

- Caddy listens on 127.0.0.1 only. Database tools refuse non-local hosts and
  forwarded (tunnel) requests, and dotfiles are never served.
- `.htaccess` deny rules also stop PHP from running.
- Cached downloads are verified, and the download cache moved out of shared temp.
- CHAMP no longer trusts runtime config or portable markers placed beside the
  executable.
- Folder commands are scoped to CHAMP's data folder, with least-privilege
  capabilities and a stricter CSP.

### Fixed

- Language and sound settings no longer reset when you save Settings.
- Database tool installs are now verified.
- Fixed shortcuts that did nothing, and shortcuts no longer fire while you type.
- The stack stays up when an update fails.
- Log Doctor no longer raises false alarms on warnings and bare exit lines.

### Known issues

- 2.0.0 has no macOS installers (the macOS build failed). Fixed in 2.1.0.

[2.1.0]: https://github.com/thirawat27/CHAMP/compare/v2.0.0...v2.1.0
[2.0.0]: https://github.com/thirawat27/CHAMP/compare/v1.3.0...v2.0.0
