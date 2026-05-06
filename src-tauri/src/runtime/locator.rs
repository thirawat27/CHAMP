use std::fs;
use std::path::{Path, PathBuf};

const APP_DIR_NAME: &str = "CHAMP";
const LEGACY_APP_DIR_NAME: &str = "campp";

/// Runtime binary paths
#[derive(Debug, Clone)]
pub struct RuntimePaths {
    pub caddy: PathBuf,
    pub php_cgi: PathBuf,
    pub php_ini: PathBuf,
    pub mysql: PathBuf,
    pub adminer: PathBuf,
    /// Directory where PHP extensions are located (same as php_cgi)
    #[allow(dead_code)]
    pub php_ext_dir: PathBuf,
    /// Data directory for MySQL
    pub mysql_data_dir: PathBuf,
    /// Logs directory
    pub logs_dir: PathBuf,
    /// Config directory
    pub config_dir: PathBuf,
    /// Projects directory
    pub projects_dir: PathBuf,
}

/// Application data directory structure
#[derive(Debug, Clone)]
pub struct AppDataPaths {
    /// Base data directory (e.g., %LOCALAPPDATA%/CHAMP)
    pub base_dir: PathBuf,
    /// Runtime binaries directory
    pub runtime_dir: PathBuf,
    /// Configuration files directory
    pub config_dir: PathBuf,
    /// MySQL data directory
    pub mysql_data_dir: PathBuf,
    /// Logs directory
    pub logs_dir: PathBuf,
    /// Projects directory
    pub projects_dir: PathBuf,
}

impl AppDataPaths {
    /// Create all necessary directories
    pub fn ensure_directories(&self) -> Result<(), String> {
        for dir in [
            &self.config_dir,
            &self.mysql_data_dir,
            &self.logs_dir,
            &self.projects_dir,
        ] {
            if !dir.exists() {
                fs::create_dir_all(dir)
                    .map_err(|e| format!("Failed to create directory {}: {}", dir.display(), e))?;
            }
        }
        Ok(())
    }
}

/// Get the application data directory paths
pub fn get_app_data_paths() -> Result<AppDataPaths, String> {
    let data_dir = dirs::data_local_dir()
        .or_else(dirs::home_dir)
        .ok_or_else(|| "Cannot find a writable user data directory".to_string())?
        .join(APP_DIR_NAME);

    Ok(AppDataPaths {
        base_dir: data_dir.clone(),
        runtime_dir: data_dir.join("runtime"),
        config_dir: data_dir.join("config"),
        mysql_data_dir: data_dir.join("mysql").join("data"),
        logs_dir: data_dir.join("logs"),
        projects_dir: data_dir.join("projects"),
    })
}

/// Locate runtime binaries after download
pub fn locate_runtime_binaries() -> Result<RuntimePaths, String> {
    let app_paths = get_app_data_paths()?;
    let runtime_dir = resolve_runtime_dir(&app_paths)?;

    // Ensure runtime directory exists
    if !runtime_dir.exists() {
        return Err(format!(
            "Runtime directory not found. Please download runtime binaries first. Expected: {}",
            runtime_dir.display()
        ));
    }

    let adminer_path = app_paths.config_dir.join("adminer");

    Ok(RuntimePaths {
        caddy: detect_caddy_binary(&runtime_dir)?,
        php_cgi: detect_php_binary(&runtime_dir)?,
        php_ini: detect_php_ini(&runtime_dir)?,
        php_ext_dir: detect_php_ext_dir(&runtime_dir)?,
        mysql: detect_mysql_binary(&runtime_dir)?,
        adminer: adminer_path,
        mysql_data_dir: app_paths.mysql_data_dir.clone(),
        logs_dir: app_paths.logs_dir.clone(),
        config_dir: app_paths.config_dir.clone(),
        projects_dir: app_paths.projects_dir.clone(),
    })
}

fn resolve_runtime_dir(app_paths: &AppDataPaths) -> Result<PathBuf, String> {
    if app_paths.runtime_dir.exists() {
        return Ok(app_paths.runtime_dir.clone());
    }

    #[cfg(target_os = "windows")]
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(install_dir) = exe_path.parent() {
            let legacy_runtime = install_dir.join("runtime");
            if legacy_runtime.exists() {
                return Ok(legacy_runtime);
            }
        }
    }

    if let Some(local_app_data) = dirs::data_local_dir() {
        let legacy_runtime = local_app_data.join(LEGACY_APP_DIR_NAME).join("runtime");
        if legacy_runtime.exists() {
            return Ok(legacy_runtime);
        }
    }

    Err(format!(
        "Runtime directory not found. Please download runtime binaries first. Expected: {}",
        app_paths.runtime_dir.display()
    ))
}

fn active_php_runtime_dir(runtime_dir: &Path) -> Option<PathBuf> {
    let php_id = crate::config::AppSettings::load().package_selection.php;
    let version_dir = runtime_dir.join("php_versions").join(&php_id);
    let version_marker = runtime_dir
        .join("php_versions")
        .join(format!("{}_installed.txt", php_id));

    if version_dir.exists() || version_marker.exists() {
        Some(version_dir)
    } else {
        None
    }
}

/// Detect Caddy binary based on platform
fn detect_caddy_binary(runtime_dir: &Path) -> Result<PathBuf, String> {
    // Caddy extraction creates different structures based on platform

    #[cfg(target_os = "windows")]
    {
        // Windows: caddy.exe might be at runtime/caddy.exe or runtime/caddy/caddy.exe
        let paths_to_check = vec![
            runtime_dir.join("caddy.exe"),
            runtime_dir.join("caddy").join("caddy.exe"),
        ];

        for path in paths_to_check {
            if path.exists() {
                return Ok(path);
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        // Unix: caddy binary might be at runtime/caddy or runtime/caddy/caddy
        let paths_to_check = vec![
            runtime_dir.join("caddy"),
            runtime_dir.join("caddy").join("caddy"),
        ];

        for path in paths_to_check {
            if path.exists() {
                return Ok(path);
            }
        }
    }

    Err(format!(
        "Caddy binary not found in {}. Please ensure runtime binaries are downloaded.",
        runtime_dir.display()
    ))
}

/// Detect PHP CGI binary based on platform
fn detect_php_binary(runtime_dir: &Path) -> Result<PathBuf, String> {
    if let Some(active_dir) = active_php_runtime_dir(runtime_dir) {
        if let Ok(path) = detect_php_binary_in_dir(&active_dir) {
            return Ok(path);
        }
    }

    detect_php_binary_in_dir(runtime_dir)
}

fn detect_php_binary_in_dir(runtime_dir: &Path) -> Result<PathBuf, String> {
    #[cfg(target_os = "windows")]
    {
        // Windows PHP distribution structure:
        // - runtime/php-8.4.16-Win32-vs17-x64/php-cgi.exe (versioned directory from zip)
        // - runtime/php/php-cgi.exe (renamed/extracted)
        // - runtime/php-cgi.exe (direct in runtime dir)

        // First, look for versioned PHP directories (like php-8.4.16-Win32-vs17-x64)
        if let Ok(entries) = fs::read_dir(runtime_dir) {
            for entry in entries.flatten() {
                if let Ok(name) = entry.file_name().into_string() {
                    if name.starts_with("php-") && name.contains("Win32") && entry.path().is_dir() {
                        let php_cgi_path = entry.path().join("php-cgi.exe");
                        if php_cgi_path.exists() {
                            return Ok(php_cgi_path);
                        }
                        // Also check for php.exe as fallback
                        let php_exe_path = entry.path().join("php.exe");
                        if php_exe_path.exists() {
                            return Ok(php_exe_path);
                        }
                    }
                }
            }
        }

        // Fallback paths
        let paths_to_check = vec![
            runtime_dir.join("php-cgi.exe"), // Direct in runtime dir
            runtime_dir.join("php").join("php-cgi.exe"),
            runtime_dir.join("php.exe"), // Fallback to CLI
            runtime_dir.join("php").join("php.exe"),
        ];

        for path in paths_to_check {
            if path.exists() {
                return Ok(path);
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        // macOS: static-php extracts to buildroot/bin/
        let paths_to_check = vec![
            runtime_dir.join("buildroot").join("bin").join("php-fpm"), // static-php FPM
            runtime_dir.join("buildroot").join("bin").join("php"),     // static-php CLI
            runtime_dir.join("php").join("bin").join("php-fpm"),
            runtime_dir.join("php").join("bin").join("php-cgi"),
            runtime_dir.join("php-fpm"), // Direct in runtime dir
            runtime_dir.join("php-cgi"), // Direct in runtime dir
            runtime_dir
                .join("usr")
                .join("local")
                .join("bin")
                .join("php"),
            runtime_dir.join("php").join("bin").join("php"),
            runtime_dir.join("php"),
        ];

        for path in paths_to_check {
            if path.exists() {
                return Ok(path);
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        // Linux: static-php extracts to buildroot/bin/ or directly as php-fpm
        let paths_to_check = vec![
            runtime_dir.join("php-fpm"), // Direct in runtime dir (static-php bulk)
            runtime_dir.join("buildroot").join("bin").join("php-fpm"), // static-php FPM
            runtime_dir.join("buildroot").join("bin").join("php"), // static-php CLI
            runtime_dir.join("php").join("bin").join("php-fpm"),
            runtime_dir.join("php").join("bin").join("php-cgi"),
            runtime_dir.join("php-cgi"), // Direct in runtime dir
            runtime_dir.join("php").join("bin").join("php"),
            runtime_dir.join("usr").join("bin").join("php"),
            runtime_dir.join("php"),
        ];

        for path in paths_to_check {
            if path.exists() {
                return Ok(path);
            }
        }

        // Check for wrong-platform binaries (Windows .exe files on Linux)
        let windows_php = runtime_dir.join("php-cgi.exe");
        if windows_php.exists() {
            return Err(format!(
                "Wrong platform: Windows PHP binaries found in {} but this is Linux. \
                 Please delete the runtime directory and re-download: {}",
                runtime_dir.display(),
                runtime_dir.display()
            ));
        }
    }

    Err(format!(
        "PHP binary not found in {}. Please ensure runtime binaries are downloaded.",
        runtime_dir.display()
    ))
}

/// Detect PHP configuration file
fn detect_php_ini(_runtime_dir: &Path) -> Result<PathBuf, String> {
    // PHP ini will be generated in config directory
    let app_paths = get_app_data_paths()?;
    let php_ini_path = app_paths.config_dir.join("php.ini");

    Ok(php_ini_path)
}

/// Detect PHP extension directory
fn detect_php_ext_dir(runtime_dir: &Path) -> Result<PathBuf, String> {
    let runtime_dir =
        active_php_runtime_dir(runtime_dir).unwrap_or_else(|| runtime_dir.to_path_buf());

    #[cfg(target_os = "windows")]
    {
        // First, look for versioned PHP directories (like php-8.4.16-Win32-vs17-x64)
        if let Ok(entries) = fs::read_dir(&runtime_dir) {
            for entry in entries.flatten() {
                if let Ok(name) = entry.file_name().into_string() {
                    if name.starts_with("php-") && name.contains("Win32") && entry.path().is_dir() {
                        let ext_path = entry.path().join("ext");
                        if ext_path.exists() {
                            return Ok(ext_path);
                        }
                    }
                }
            }
        }

        // Fallback paths
        let paths_to_check = vec![runtime_dir.join("php").join("ext"), runtime_dir.join("ext")];

        for path in paths_to_check {
            if path.exists() {
                return Ok(path);
            }
        }

        // Fallback: return the expected path even if it doesn't exist yet
        Ok(runtime_dir.join("php").join("ext"))
    }

    #[cfg(not(target_os = "windows"))]
    {
        // On Unix, static-php bundles extensions differently
        // Extensions are typically built-in, but check common locations
        let paths_to_check = vec![
            runtime_dir
                .join("php")
                .join("lib")
                .join("php")
                .join("extensions"),
            runtime_dir
                .join("buildroot")
                .join("lib")
                .join("php")
                .join("extensions"),
        ];

        for path in paths_to_check {
            if path.exists() {
                return Ok(path);
            }
        }

        // Fallback: return a reasonable default
        Ok(runtime_dir
            .join("php")
            .join("lib")
            .join("php")
            .join("extensions"))
    }
}

/// Detect MySQL/MariaDB server binary based on platform
///
/// **IMPORTANT Platform Differences:**
///
/// **Linux:**
/// - Uses MariaDB 12.x (binary name: mariadbd)
/// - Extracted from: mariadb-XX.X.X-linux-systemd-x86_64.tar.gz
/// - Directory: mariadb-XX.X.X-linux-systemd-x86_64/bin/mariadbd
///
/// **Windows/macOS:**
/// - Uses MySQL 8.x (binary name: mysqld)
/// - Extracted from platform-specific archives
/// - Directory: mysql-X.X.X/bin/mysqld
fn detect_mysql_binary(runtime_dir: &Path) -> Result<PathBuf, String> {
    #[cfg(target_os = "windows")]
    {
        // ============================================================
        // WINDOWS: MySQL Binary Detection
        // ============================================================
        // Binary: mysqld.exe
        // Source: https://dev.mysql.com/downloads/mysql/
        // Archive: mysql-VERSION-winx64.zip
        // Extracts to: mysql-VERSION/
        // Binary path: .../bin/mysqld.exe
        // ============================================================

        // Look for any directory starting with "mysql"
        if let Ok(entries) = fs::read_dir(runtime_dir) {
            for entry in entries.flatten() {
                if let Ok(name) = entry.file_name().into_string() {
                    if name.starts_with("mysql") && entry.path().is_dir() {
                        let mysqld_path = entry.path().join("bin").join("mysqld.exe");
                        if mysqld_path.exists() {
                            return Ok(mysqld_path);
                        }
                    }
                }
            }
        }

        // Fallback paths
        let paths_to_check = vec![
            runtime_dir.join("mysql").join("bin").join("mysqld.exe"),
            runtime_dir.join("bin").join("mysqld.exe"),
            runtime_dir.join("mysqld.exe"),
        ];

        for path in paths_to_check {
            if path.exists() {
                return Ok(path);
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        // ============================================================
        // MACOS: MySQL Binary Detection
        // ============================================================
        // Binary: mysqld
        // Source: https://dev.mysql.com/downloads/mysql/
        // Archive: mysql-VERSION-macos14-x86_64.tar.gz
        // Extracts to: mysql-VERSION/
        // Binary path: .../bin/mysqld
        // ============================================================

        if let Ok(entries) = fs::read_dir(runtime_dir) {
            for entry in entries.flatten() {
                if let Ok(name) = entry.file_name().into_string() {
                    if name.starts_with("mysql") && entry.path().is_dir() {
                        let mysqld_path = entry.path().join("bin").join("mysqld");
                        if mysqld_path.exists() {
                            return Ok(mysqld_path);
                        }
                    }
                }
            }
        }

        let paths_to_check = vec![
            runtime_dir.join("mysql").join("bin").join("mysqld"),
            runtime_dir.join("mysql").join("bin").join("mysqld"),
            runtime_dir.join("bin").join("mysqld"),
            runtime_dir.join("mysqld"),
        ];

        for path in paths_to_check {
            if path.exists() {
                return Ok(path);
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        // ============================================================
        // LINUX: MariaDB Binary Detection
        // ============================================================
        // Binary: mariadbd (MariaDB 10.2+)
        // Source: https://archive.mariadb.org/
        // Archive: mariadb-XX.X.X-linux-systemd-x86_64.tar.gz
        // Extracts to: mariadb-XX.X.X-linux-systemd-x86_64/
        // Binary path: .../bin/mariadbd
        //
        // Note: Older MariaDB versions (< 10.2) used mysqld,
        // so we check for both as a fallback.
        // ============================================================

        // First, look for official MySQL directories.
        if let Ok(entries) = fs::read_dir(runtime_dir) {
            for entry in entries.flatten() {
                if let Ok(name) = entry.file_name().into_string() {
                    if name.starts_with("mysql") && entry.path().is_dir() {
                        let mysqld_path = entry.path().join("bin").join("mysqld");
                        if mysqld_path.exists() {
                            return Ok(mysqld_path);
                        }
                    }
                }
            }
        }

        // Then look for MariaDB directories.
        if let Ok(entries) = fs::read_dir(runtime_dir) {
            for entry in entries.flatten() {
                if let Ok(name) = entry.file_name().into_string() {
                    // Check for MariaDB directories (mariadb-XX.X.X-*)
                    if name.starts_with("mariadb") && entry.path().is_dir() {
                        // Try mariadbd first (MariaDB 10.2+)
                        let mariadbd_path = entry.path().join("bin").join("mariadbd");
                        if mariadbd_path.exists() {
                            return Ok(mariadbd_path);
                        }
                        // Fallback to mysqld for older MariaDB versions
                        let mysqld_path = entry.path().join("bin").join("mysqld");
                        if mysqld_path.exists() {
                            return Ok(mysqld_path);
                        }
                    }
                }
            }
        }

        // Fallback paths - check both mariadbd and mysqld
        let paths_to_check = vec![
            runtime_dir.join("mysql").join("bin").join("mysqld"),
            runtime_dir.join("bin").join("mysqld"),
            runtime_dir.join("mariadb").join("bin").join("mariadbd"),
            runtime_dir.join("bin").join("mariadbd"),
            runtime_dir.join("mariadbd"),
            runtime_dir.join("mysqld"), // Fallback for older versions
        ];

        for path in paths_to_check {
            if path.exists() {
                return Ok(path);
            }
        }
    }

    Err(format!(
        "MariaDB binary not found in {}. Please ensure runtime binaries are downloaded.",
        runtime_dir.display()
    ))
}

/// Check if a binary is valid (exists and is executable)
#[allow(dead_code)]
pub fn is_valid_binary(path: &Path) -> bool {
    if !path.exists() {
        return false;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        match fs::metadata(path) {
            Ok(metadata) => {
                let permissions = metadata.permissions();
                let mode = permissions.mode();
                // Check if owner has execute permission
                mode & 0o100 != 0
            }
            Err(_) => false,
        }
    }

    #[cfg(windows)]
    {
        // On Windows, just check if the file exists
        true
    }
}

/// Verify all runtime binaries are present and valid
#[allow(dead_code)]
pub fn verify_runtime_binaries() -> Result<RuntimePaths, String> {
    let paths = locate_runtime_binaries()?;

    if !is_valid_binary(&paths.caddy) {
        return Err(format!(
            "Caddy binary not found or not executable: {}",
            paths.caddy.display()
        ));
    }

    if !is_valid_binary(&paths.php_cgi) {
        return Err(format!(
            "PHP binary not found or not executable: {}",
            paths.php_cgi.display()
        ));
    }

    if !is_valid_binary(&paths.mysql) {
        return Err(format!(
            "MariaDB binary not found or not executable: {}",
            paths.mysql.display()
        ));
    }

    Ok(paths)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use std::path::Path;
    use tempfile::TempDir;

    fn create_mock_binary(path: &Path) -> Result<(), String> {
        let mut file = File::create(path).map_err(|e| format!("Failed to create binary: {}", e))?;
        writeln!(file, "#! /bin/sh\n# mock binary").unwrap();
        Ok(())
    }

    #[cfg(unix)]
    fn set_executable(path: &Path) {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(path, perms).unwrap();
    }

    #[test]
    fn test_app_data_paths_creation() {
        let temp_dir = TempDir::new().unwrap();
        let base_dir = temp_dir.path().join("campp");
        fs::create_dir_all(&base_dir).unwrap();

        let paths = AppDataPaths {
            base_dir: base_dir.clone(),
            runtime_dir: base_dir.join("runtime"),
            config_dir: base_dir.join("config"),
            mysql_data_dir: base_dir.join("mysql").join("data"),
            logs_dir: base_dir.join("logs"),
            projects_dir: base_dir.join("projects"),
        };

        let result = paths.ensure_directories();
        assert!(result.is_ok());

        assert!(paths.config_dir.exists());
        assert!(paths.mysql_data_dir.exists());
        assert!(paths.logs_dir.exists());
        assert!(paths.projects_dir.exists());
    }

    #[test]
    fn test_is_valid_binary_with_nonexistent_file() {
        let temp_dir = TempDir::new().unwrap();
        let nonexistent = temp_dir.path().join("nonexistent.exe");

        assert!(!is_valid_binary(&nonexistent));
    }

    #[test]
    fn test_is_valid_binary_with_existing_file() {
        let temp_dir = TempDir::new().unwrap();
        let binary_path = temp_dir.path().join("test_binary");

        create_mock_binary(&binary_path).unwrap();

        #[cfg(unix)]
        set_executable(&binary_path);

        let is_valid = is_valid_binary(&binary_path);
        #[cfg(windows)]
        assert!(is_valid);

        #[cfg(unix)]
        assert!(is_valid);
    }

    #[test]
    fn test_runtime_paths_structure() {
        let temp_dir = TempDir::new().unwrap();

        let paths = RuntimePaths {
            caddy: temp_dir.path().join("caddy.exe"),
            php_cgi: temp_dir.path().join("php").join("php.exe"),
            php_ini: temp_dir.path().join("config").join("php.ini"),
            php_ext_dir: temp_dir.path().join("php").join("ext"),
            mysql: temp_dir.path().join("mysql").join("bin").join("mysqld.exe"),
            adminer: temp_dir.path().join("adminer"),
            mysql_data_dir: temp_dir.path().join("mysql").join("data"),
            logs_dir: temp_dir.path().join("logs"),
            config_dir: temp_dir.path().join("config"),
            projects_dir: temp_dir.path().join("projects"),
        };

        assert!(paths.caddy.ends_with("caddy.exe"));
        assert!(paths.php_cgi.ends_with("php.exe"));
        assert!(paths.php_ini.ends_with("php.ini"));
        assert!(paths.mysql.ends_with("mysqld.exe"));
        assert!(paths.adminer.ends_with("adminer"));
    }

    #[test]
    fn test_runtime_paths_clone() {
        let temp_dir = TempDir::new().unwrap();

        let paths1 = RuntimePaths {
            caddy: temp_dir.path().join("caddy.exe"),
            php_cgi: temp_dir.path().join("php").join("php.exe"),
            php_ini: temp_dir.path().join("config").join("php.ini"),
            php_ext_dir: temp_dir.path().join("php").join("ext"),
            mysql: temp_dir.path().join("mysql").join("bin").join("mysqld.exe"),
            adminer: temp_dir.path().join("adminer"),
            mysql_data_dir: temp_dir.path().join("mysql").join("data"),
            logs_dir: temp_dir.path().join("logs"),
            config_dir: temp_dir.path().join("config"),
            projects_dir: temp_dir.path().join("projects"),
        };

        let paths2 = paths1.clone();

        assert_eq!(paths1.caddy, paths2.caddy);
        assert_eq!(paths1.php_cgi, paths2.php_cgi);
        assert_eq!(paths1.mysql, paths2.mysql);
    }

    #[test]
    fn test_app_data_paths_clone() {
        let temp_dir = TempDir::new().unwrap();
        let base_dir = temp_dir.path().join("campp");

        let paths1 = AppDataPaths {
            base_dir: base_dir.clone(),
            runtime_dir: base_dir.join("runtime"),
            config_dir: base_dir.join("config"),
            mysql_data_dir: base_dir.join("mysql").join("data"),
            logs_dir: base_dir.join("logs"),
            projects_dir: base_dir.join("projects"),
        };

        let paths2 = paths1.clone();

        assert_eq!(paths1.base_dir, paths2.base_dir);
        assert_eq!(paths1.runtime_dir, paths2.runtime_dir);
        assert_eq!(paths1.config_dir, paths2.config_dir);
    }
}
