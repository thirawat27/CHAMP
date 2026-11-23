//! Xdebug step debugging for any PHP project.
//!
//! CHAMP does not download Xdebug: builds differ per PHP minor version, thread
//! safety, and compiler, and the static PHP builds used on macOS/Linux cannot
//! load shared extensions at all. When an Xdebug extension file sits in the
//! selected PHP's `ext` folder, this toggles it in php.ini with settings that
//! only connect when asked (`start_with_request=trigger`), so pages stay fast.

use crate::config::AppSettings;
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct XdebugStatus {
    pub enabled: bool,
    /// An Xdebug extension file exists for the selected PHP.
    pub available: bool,
    pub extension_path: Option<PathBuf>,
    pub ext_dir: Option<PathBuf>,
    pub client_port: u16,
    /// How to get Xdebug when it is not available, in English.
    pub hint: Option<String>,
}

/// The Xdebug extension in `ext_dir`, if one is there.
pub fn find_extension(ext_dir: &Path) -> Option<PathBuf> {
    let exact = if cfg!(windows) {
        "php_xdebug.dll"
    } else {
        "xdebug.so"
    };
    let path = ext_dir.join(exact);
    if path.is_file() {
        return Some(path);
    }
    // Downloads from xdebug.org keep their long name on Windows
    // (php_xdebug-3.4.x-8.4-ts-vs17-x86_64.dll).
    let mut found: Vec<PathBuf> = std::fs::read_dir(ext_dir)
        .ok()?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path
                    .file_name()
                    .map(|name| name.to_string_lossy().to_ascii_lowercase())
                    .is_some_and(|name| {
                        (name.starts_with("php_xdebug") && name.ends_with(".dll"))
                            || (name.starts_with("xdebug") && name.ends_with(".so"))
                    })
        })
        .collect();
    found.sort();
    found.pop()
}

/// php.ini lines for Xdebug, or nothing when it is off or missing.
pub fn ini_lines(extension: Option<&Path>, enabled: bool, client_port: u16) -> String {
    let Some(extension) = extension.filter(|_| enabled) else {
        return String::new();
    };
    format!(
        "; Xdebug (CHAMP): connects only when a request asks (XDEBUG_TRIGGER, browser extension).\nzend_extension=\"{}\"\nxdebug.mode = debug,develop\nxdebug.start_with_request = trigger\nxdebug.client_host = 127.0.0.1\nxdebug.client_port = {}\nxdebug.log_level = 0\n",
        extension.to_string_lossy().replace('\\', "/"),
        client_port
    )
}

fn hint(ext_dir: Option<&Path>) -> String {
    if cfg!(windows) {
        let folder = ext_dir
            .map(|dir| dir.display().to_string())
            .unwrap_or_else(|| "the selected PHP's ext folder".to_string());
        format!(
            "Download the Xdebug DLL that matches this PHP (version, TS/NTS, x64) from https://xdebug.org/download and put it in {folder}, then turn Xdebug on."
        )
    } else {
        "CHAMP's PHP on macOS/Linux is a static build and cannot load Xdebug. Use the Xdebug-free profilers, or run PHP from your package manager for step debugging.".to_string()
    }
}

/// `ext` folder of the selected PHP.
pub fn ext_dir() -> Option<PathBuf> {
    let paths = crate::runtime::locator::locate_runtime_binaries().ok()?;
    paths.php_cgi.parent().map(|dir| dir.join("ext"))
}

pub fn status() -> XdebugStatus {
    let settings = AppSettings::load().xdebug;
    let ext_dir = ext_dir();
    let extension = ext_dir.as_deref().and_then(find_extension);
    XdebugStatus {
        enabled: settings.enabled,
        available: extension.is_some(),
        hint: extension.is_none().then(|| hint(ext_dir.as_deref())),
        extension_path: extension,
        ext_dir,
        client_port: settings.client_port,
    }
}

/// Save the choice and restart a running PHP so it takes effect.
pub async fn set_enabled(app: &tauri::AppHandle, enabled: bool) -> Result<XdebugStatus, String> {
    let current = tauri::async_runtime::spawn_blocking(status)
        .await
        .map_err(|e| format!("Xdebug task failed: {e}"))?;
    if enabled && !current.available {
        return Err(current
            .hint
            .unwrap_or_else(|| "Xdebug is not available for this PHP.".to_string()));
    }
    AppSettings::update(|settings| settings.xdebug.enabled = enabled)?;
    let _ = crate::process::runner::run_with_manager(app, |manager| {
        manager.update_health();
        if manager
            .status(crate::process::ServiceType::PhpFpm)
            .is_running()
        {
            let _ = manager.restart(crate::process::ServiceType::PhpFpm);
        }
    })
    .await;
    tauri::async_runtime::spawn_blocking(status)
        .await
        .map_err(|e| format!("Xdebug task failed: {e}"))
}

#[tauri::command]
pub async fn get_xdebug_status() -> Result<XdebugStatus, String> {
    tauri::async_runtime::spawn_blocking(status)
        .await
        .map_err(|e| format!("Xdebug task failed: {e}"))
}

#[tauri::command]
pub async fn set_xdebug(enabled: bool, app: tauri::AppHandle) -> Result<XdebugStatus, String> {
    set_enabled(&app, enabled).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_short_or_downloaded_name() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(find_extension(dir.path()), None);
        let name = if cfg!(windows) {
            "php_xdebug-3.4.5-8.4-ts-vs17-x86_64.dll"
        } else {
            "xdebug-3.4.so"
        };
        std::fs::write(dir.path().join(name), b"x").unwrap();
        assert_eq!(find_extension(dir.path()), Some(dir.path().join(name)));
        let exact = if cfg!(windows) {
            "php_xdebug.dll"
        } else {
            "xdebug.so"
        };
        std::fs::write(dir.path().join(exact), b"x").unwrap();
        assert_eq!(find_extension(dir.path()), Some(dir.path().join(exact)));
    }

    #[test]
    fn ini_lines_only_when_enabled_and_present() {
        let path = Path::new(r"C:\champ\php\ext\php_xdebug.dll");
        assert!(ini_lines(None, true, 9003).is_empty());
        assert!(ini_lines(Some(path), false, 9003).is_empty());
        let lines = ini_lines(Some(path), true, 9010);
        assert!(lines.contains("zend_extension=\"C:/champ/php/ext/php_xdebug.dll\""));
        assert!(lines.contains("xdebug.start_with_request = trigger"));
        assert!(lines.contains("xdebug.client_port = 9010"));
    }
}
