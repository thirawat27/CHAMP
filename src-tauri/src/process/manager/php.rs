//! Starting PHP (php-fpm, or php-cgi in FastCGI mode on Windows).

use super::logs::{open_log_file_with_retry, LogOutput};
use super::php_config::{apply_php_database_env, generate_php_fpm_conf, generate_php_ini};
use super::platform::configure_no_window;
use super::{attach_started_process, ServiceProcess};
use crate::runtime::locator::RuntimePaths;
use std::process::Command;

/// Start PHP-FPM (using PHP-CGI for simplicity in MVP)
pub(super) fn start_php_fpm(
    service_process: &mut ServiceProcess,
    paths: &RuntimePaths,
    web_port: u16,
    mysql_port: u16,
    postgresql_port: u16,
) -> Result<(), String> {
    crate::runtime::locator::require_database_binary(&paths.php_cgi, "PHP")?;

    // Regenerate php.ini on each start because it depends on the selected PHP runtime.
    generate_php_ini(&paths.php_ini, paths, web_port, mysql_port, postgresql_port)?;

    // Open log file with retry logic
    let log_path = paths.logs_dir.join("php-fpm.log");
    let log_file = open_log_file_with_retry(&log_path, "PHP-FPM")?;

    // Check if we have php-fpm (static-php on Linux/macOS) or php-cgi (Windows)
    let is_fpm = paths
        .php_cgi
        .file_name()
        .and_then(|n| n.to_str())
        .map(|n| n == "php-fpm")
        .unwrap_or(false);

    let child = if is_fpm {
        // php-fpm.conf depends on the runtime port, so always regenerate it. (M-05)
        let fpm_conf_path = paths.config_dir.join("php-fpm.conf");
        generate_php_fpm_conf(&fpm_conf_path, paths, service_process.port)?;

        // PHP-FPM requires -F to run in foreground and -y for config
        let mut cmd = configure_no_window(Command::new(&paths.php_cgi));
        apply_php_database_env(&mut cmd, web_port, mysql_port, postgresql_port);
        cmd.arg("-F") // Don't daemonize
            .arg("-y")
            .arg(&fpm_conf_path)
            .arg("-c")
            .arg(&paths.php_ini)
            .current_dir(&paths.config_dir)
            .log_output_to(log_file, "PHP-FPM")?;
        cmd.spawn()
            .map_err(|e| format!("Failed to start PHP-FPM: {}", e))?
    } else {
        // PHP-CGI (Windows) uses -b for FastCGI mode
        let mut cmd = configure_no_window(Command::new(&paths.php_cgi));
        apply_php_database_env(&mut cmd, web_port, mysql_port, postgresql_port);
        cmd.arg("-b")
            .arg(format!("127.0.0.1:{}", service_process.port))
            .arg("-c")
            .arg(&paths.php_ini)
            .current_dir(&paths.config_dir)
            .log_output_to(log_file, "PHP-CGI")?;
        cmd.spawn()
            .map_err(|e| format!("Failed to start PHP-CGI: {}", e))?
    };

    attach_started_process(service_process, child, log_path, "PHP")
}
