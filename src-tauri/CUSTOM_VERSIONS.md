# Customizing Runtime Versions

This guide explains how to customize the versions of PHP, MariaDB, phpMyAdmin, and Caddy that CAMPP downloads and uses.

## Quick Start

1. **Copy the template**: Copy `runtime-config.user.json.template` to `runtime-config.json`
2. **Edit the file**: Open `runtime-config.json` in a text editor
3. **Select versions**: Find the versions you want to use and set `"selected": true`
4. **Restart CAMPP**: Restart the application to apply changes

## Configuration File Location

The `runtime-config.json` file should be placed in one of these locations (searched in order):
- `runtime-config.json` (current directory)
- `src-tauri/runtime-config.json` (development)
- Alongside the executable (production)

## File Structure

```json
{
  "version": "1.0",
  "binaries": {
    "php": {
      "versions": [
        {
          "id": "php-8.5",
          "version": "8.5.1",
          "selected": true,
          "display_name": "PHP 8.5.1 (Latest)",
          "urls": {
            "windowsX64": "...",
            "linuxX64": "...",
            ...
          }
        }
      ]
    },
    ...
  }
}
```

## Selecting a Different Version

To select a different version, set `"selected": true` for your desired version and `"selected": false` for all others.

### Example: Switching from PHP 8.5 to PHP 8.3

**Before:**
```json
{
  "id": "php-8.5",
  "selected": true,
  ...
},
{
  "id": "php-8.3",
  "selected": false,
  ...
}
```

**After:**
```json
{
  "id": "php-8.5",
  "selected": false,
  ...
},
{
  "id": "php-8.3",
  "selected": true,
  ...
}
```

## Adding Custom Versions

You can add custom versions by adding a new entry to the `versions` array:

### Example: Adding a Custom PHP Build

```json
{
  "id": "php-custom",
  "version": "8.4.99",
  "selected": true,
  "display_name": "PHP Custom Build",
  "urls": {
    "windowsX64": "https://your-server.com/custom-php.zip",
    "linuxX64": "https://your-server.com/custom-php.tar.gz"
  }
}
```

## Platform-Specific URLs

Each version can have different URLs for different platforms:

- `windowsX64` - Windows 64-bit (Intel/AMD)
- `windowsArm64` - Windows on ARM (Surface Pro X, etc.)
- `linuxX64` - Linux 64-bit (Intel/AMD)
- `linuxArm64` - Linux ARM64 (Raspberry Pi, etc.)
- `macOSX64` - macOS Intel
- `macOSArm64` - macOS Apple Silicon (M1/M2/M3)

For phpMyAdmin, only a single `url` field is needed since it's platform-independent.

## Finding Download URLs

### PHP
- **Windows**: https://windows.php.net/downloads/
- **Linux/macOS**: https://github.com/static-php/static-php-cli/releases

### MariaDB
- **All platforms**: https://mariadb.org/downloads/
- **Archive**: https://archive.mariadb.org/

### phpMyAdmin
- **All platforms**: https://www.phpmyadmin.net/downloads/

### Caddy
- **All platforms**: https://github.com/caddyserver/caddy/releases

## Version Selection Rules

1. **Only one version per binary should have `"selected": true`**
2. If multiple versions are selected, the first one will be used
3. If no version is selected, the default (first) version will be used

## Applying Changes

After modifying `runtime-config.json`:

1. **Restart CAMPP** - The configuration is loaded on startup
2. **Re-download runtime** - If you've already downloaded runtime binaries, you may need to:
   - Go to Settings → Reset Installation
   - Or delete the runtime folder and let CAMPP re-download

## Troubleshooting

### "Failed to parse runtime-config.json"
- Check that your JSON is valid (use a JSON validator)
- Ensure all strings are properly quoted
- Check for trailing commas (not allowed in JSON)

### "Download failed"
- Verify the URL is correct and accessible
- Some URLs may require specific user-agent headers
- Check if the version/architecture combination exists

### Wrong version installed
- Make sure only one version per binary has `"selected": true`
- Restart CAMPP after changing the configuration
- Clear existing runtime and re-download

## Example: Using PHP 8.2 for Legacy Project

```json
{
  "binaries": {
    "php": {
      "versions": [
        {
          "id": "php-8.5",
          "selected": false,
          ...
        },
        {
          "id": "php-8.2",
          "selected": true,
          "version": "8.2.30",
          "display_name": "PHP 8.2.30 (for legacy project)",
          "urls": { ... }
        }
      ]
    }
  }
}
```

## Support

For issues or questions about custom versions, please open an issue on GitHub.
