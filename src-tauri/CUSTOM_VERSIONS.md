# Customizing Runtime Versions

CHAMP loads runtime package metadata from `runtime-config.json`. Advanced users can override bundled defaults to test a different package version or custom mirror.

## Supported Components

- Caddy
- PHP
- MySQL
- PostgreSQL
- phpMyAdmin
- Adminer
- Node.js
- Python
- Go
- Ruby

`cloudflared` for HTTPS Preview is installed separately from GitHub latest-release binaries when the feature is first used.

## Search Order

CHAMP searches for runtime config files in several locations, including:

- CHAMP config directory, for example `%LOCALAPPDATA%\CHAMP\config\runtime-config.json`
- CHAMP data directory, for example `%LOCALAPPDATA%\CHAMP\runtime-config.json`
- `CHAMP_DATA_DIR` or `CHAMP_PORTABLE_DIR`
- Tauri resource directory
- Current working directory
- `src-tauri/runtime-config.json` during development

The first readable file is used. If no file is found, CHAMP falls back to the embedded runtime config.

## Basic Workflow

1. Copy `src-tauri/runtime-config.json` or `src-tauri/runtime-config.user.json.template`.
2. Place the copy in one of the searched locations.
3. Change versions, URLs, or selected flags.
4. Restart CHAMP or run the runtime reload command through the app.
5. Reinstall affected runtimes if they were already installed.

## Version Entry Shape

Most binary packages use platform-specific URLs:

```json
{
  "id": "php-8.5",
  "version": "8.5.7",
  "selected": true,
  "display_name": "PHP 8.5",
  "eol": false,
  "lts": false,
  "checksums": {},
  "urls": {
    "windowsX64": "https://example.com/php.zip",
    "windowsArm64": null,
    "linuxX64": "https://example.com/php-linux-x64.tar.gz",
    "linuxArm64": "https://example.com/php-linux-arm64.tar.gz",
    "macOSX64": "https://example.com/php-macos-x64.tar.gz",
    "macOSArm64": "https://example.com/php-macos-arm64.tar.gz"
  }
}
```

phpMyAdmin and Adminer use a single `url` because they are PHP files or archives:

```json
{
  "id": "adminer-5.4",
  "version": "5.4.2",
  "selected": false,
  "display_name": "Adminer 5.4.2",
  "url": "https://www.adminer.org/static/download/5.4.2/adminer-5.4.2.php"
}
```

## Platform Keys

- `windowsX64`
- `windowsArm64`
- `linuxX64`
- `linuxArm64`
- `macOSX64`
- `macOSArm64`

Leave unsupported platforms empty or `null`.

## Selection Rules

- Set one version per component to `"selected": true`.
- If multiple versions are selected, CHAMP uses the first selected entry.
- If no version is selected, CHAMP falls back to the first version in that component list.
- Optional runtimes can be selected in Settings or the first-run wizard.

## Supported Archive Types

CHAMP currently supports:

- `.zip`
- `.tar.gz`
- `.tar.xz`
- `.7z`
- `.php` for Adminer-style single-file packages

## Useful Sources

- Caddy: `https://github.com/caddyserver/caddy/releases`
- PHP Windows builds: `https://windows.php.net/downloads/releases/`
- PHP Linux/macOS FPM builds: `https://dl.static-php.dev/static-php-cli/bulk/`
- MySQL: `https://cdn.mysql.com/`
- PostgreSQL: `https://get.enterprisedb.com/postgresql/`
- phpMyAdmin: `https://www.phpmyadmin.net/downloads/`
- Adminer: `https://www.adminer.org/`
- Node.js: `https://nodejs.org/dist/`
- Python standalone builds: `https://github.com/astral-sh/python-build-standalone/releases`
- Go: `https://go.dev/dl/`
- RubyInstaller: `https://github.com/oneclick/rubyinstaller2/releases`

## Troubleshooting

### Runtime config will not load

- Validate JSON syntax.
- Confirm the file is in a searched location.
- Confirm field names use the exact platform keys above.

### Download fails

- Verify the URL exists for the current OS and CPU architecture.
- Confirm the archive type is supported.
- Check whether the upstream source blocks automated downloads.

### Wrong version is active

- Ensure only one version is selected.
- Restart CHAMP or reload the runtime config.
- Reinstall affected runtime packages after changing URLs or versions.
