use super::{ServiceInfo, ServiceMap, ServiceState, ServiceType};
use crate::constants::*;
use crate::runtime::locator::{locate_runtime_binaries, postgresql_initdb_binary, RuntimePaths};
use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::{Mutex, OnceLock, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

mod caddyfile;
mod db_tool;
mod htaccess;
mod php_config;
mod platform;
mod ports;

use caddyfile::*;
use db_tool::*;
use htaccess::*;
use php_config::*;
use platform::*;
use ports::*;

const ALL_SERVICES: [ServiceType; 4] = [
    ServiceType::Caddy,
    ServiceType::PhpFpm,
    ServiceType::MySQL,
    ServiceType::PostgreSQL,
];

fn active_database_service(database_tool_id: &str) -> ServiceType {
    if database_tool_id.starts_with("adminer") {
        ServiceType::PostgreSQL
    } else {
        ServiceType::MySQL
    }
}

fn stack_start_services(database_tool_id: &str) -> [ServiceType; 3] {
    [
        ServiceType::PhpFpm,
        active_database_service(database_tool_id),
        ServiceType::Caddy,
    ]
}

/// Open a log file with retry logic for Windows file locking
fn open_log_file_with_retry(log_path: &PathBuf, service_name: &str) -> Result<File, String> {
    for attempt in 0..MAX_LOG_FILE_RETRY {
        // Try to open the file, truncating if it exists (for fresh logs)
        // On subsequent retries, try to append in case another process has it open
        let result = if attempt == 0 {
            File::create(log_path)
        } else {
            OpenOptions::new().create(true).append(true).open(log_path)
        };

        match result {
            Ok(file) => return Ok(file),
            Err(e) => {
                if e.raw_os_error() == Some(32) && attempt < MAX_LOG_FILE_RETRY - 1 {
                    // Windows error 32: file is being used by another process
                    // Wait and retry
                    std::thread::sleep(log_file_retry_delay());
                } else {
                    return Err(format!(
                        "Failed to create {} log file after {} attempts: {}",
                        service_name,
                        attempt + 1,
                        e
                    ));
                }
            }
        }
    }

    Err(format!(
        "Failed to create {} log file: maximum retries exceeded",
        service_name
    ))
}

fn format_exit_status(status: ExitStatus) -> String {
    if let Some(code) = status.code() {
        return format!("exit code {}", code);
    }

    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(signal) = status.signal() {
            return format!("signal {}", signal);
        }
    }

    format!("{:?}", status)
}

fn read_log_tail(log_path: &Path, max_lines: usize) -> Option<String> {
    let content = fs::read_to_string(log_path).ok()?;
    let mut lines: Vec<&str> = content.lines().rev().take(max_lines).collect();
    lines.reverse();
    let tail = lines.join("\n").trim().to_string();
    if tail.is_empty() {
        None
    } else {
        Some(tail)
    }
}

fn format_process_exit_error(summary: &str, status: ExitStatus, log_path: Option<&Path>) -> String {
    let mut message = format!("{} ({})", summary, format_exit_status(status));
    if let Some(path) = log_path {
        message.push_str(&format!("\nLog file: {}", path.display()));
        if let Some(tail) = read_log_tail(path, MAX_LOG_TAIL_LINES) {
            message.push_str("\nLast log lines:\n");
            message.push_str(&tail);
        }
    }
    message
}

/// A running service process with its handle and configuration
pub struct ServiceProcess {
    pub name: ServiceType,
    pub child: Option<Child>,
    pub state: ServiceState,
    pub port: u16,
    /// Path to the log file for this service
    pub log_file: Option<PathBuf>,
    /// Error message if the service is in error state
    pub error_message: Option<String>,
    external_pid: Option<u32>,
}

/// Process manager for CHAMP services
pub struct ProcessManager {
    services: HashMap<ServiceType, ServiceProcess>,
    runtime_paths: Option<RuntimePaths>,
    settings: crate::config::AppSettings,
}

impl ProcessManager {
    pub fn new() -> Self {
        Self::with_settings(crate::config::AppSettings::load())
    }

    pub fn with_settings(settings: crate::config::AppSettings) -> Self {
        let mut services = HashMap::new();

        for service_type in ALL_SERVICES {
            services.insert(
                service_type,
                ServiceProcess {
                    name: service_type,
                    child: None,
                    state: ServiceState::Stopped,
                    port: Self::port_for_service(service_type, &settings),
                    log_file: None,
                    error_message: None,
                    external_pid: None,
                },
            );
        }

        Self {
            services,
            runtime_paths: None,
            settings,
        }
    }

    fn port_for_service(service_type: ServiceType, settings: &crate::config::AppSettings) -> u16 {
        match service_type {
            ServiceType::Caddy => settings.web_port,
            ServiceType::PhpFpm => settings.php_port,
            ServiceType::MySQL => settings.mysql_port,
            ServiceType::PostgreSQL => settings.postgresql_port,
        }
    }

    fn set_runtime_port(&mut self, service_type: ServiceType, port: u16) {
        match service_type {
            ServiceType::Caddy => self.settings.web_port = port,
            ServiceType::PhpFpm => self.settings.php_port = port,
            ServiceType::MySQL => self.settings.mysql_port = port,
            ServiceType::PostgreSQL => self.settings.postgresql_port = port,
        }

        if let Some(service_process) = self.services.get_mut(&service_type) {
            service_process.port = port;
        }
    }

    fn reserved_ports_except(&self, service_type: ServiceType) -> Vec<u16> {
        self.services
            .iter()
            .filter_map(|(ty, service)| (*ty != service_type).then_some(service.port))
            .collect()
    }

    fn web_port(&self) -> u16 {
        self.services
            .get(&ServiceType::Caddy)
            .map(|service| service.port)
            .unwrap_or(self.settings.web_port)
    }

    fn php_port(&self) -> u16 {
        self.services
            .get(&ServiceType::PhpFpm)
            .map(|service| service.port)
            .unwrap_or(self.settings.php_port)
    }

    fn mysql_port(&self) -> u16 {
        self.services
            .get(&ServiceType::MySQL)
            .map(|service| service.port)
            .unwrap_or(self.settings.mysql_port)
    }

    fn postgresql_port(&self) -> u16 {
        self.services
            .get(&ServiceType::PostgreSQL)
            .map(|service| service.port)
            .unwrap_or(self.settings.postgresql_port)
    }

    pub fn update_ports(&mut self, settings: &crate::config::AppSettings) {
        self.settings = settings.clone();
        for (service_type, service_process) in self.services.iter_mut() {
            service_process.port = Self::port_for_service(*service_type, settings);
        }
    }

    /// Initialize the process manager with runtime paths
    pub fn initialize(&mut self) -> Result<(), String> {
        let paths = locate_runtime_binaries()?;
        self.runtime_paths = Some(paths);

        // Ensure all required directories exist
        if let Some(ref paths) = self.runtime_paths {
            fs::create_dir_all(&paths.config_dir)
                .map_err(|e| format!("Failed to create config dir: {}", e))?;
            fs::create_dir_all(&paths.logs_dir)
                .map_err(|e| format!("Failed to create logs dir: {}", e))?;

            // Create PHP sessions directory for session storage
            let php_sessions_dir = paths.logs_dir.join("php-sessions");
            fs::create_dir_all(&php_sessions_dir)
                .map_err(|e| format!("Failed to create PHP sessions dir: {}", e))?;

            #[cfg(target_os = "linux")]
            fs::create_dir_all(&paths.mysql_data_dir)
                .map_err(|e| format!("Failed to create MariaDB data dir: {}", e))?;
            #[cfg(not(target_os = "linux"))]
            fs::create_dir_all(&paths.mysql_data_dir)
                .map_err(|e| format!("Failed to create MySQL data dir: {}", e))?;
            fs::create_dir_all(&paths.postgresql_data_dir)
                .map_err(|e| format!("Failed to create PostgreSQL data dir: {}", e))?;
            fs::create_dir_all(&paths.projects_dir)
                .map_err(|e| format!("Failed to create projects dir: {}", e))?;
        }

        Ok(())
    }

    /// Start a service
    pub fn start(&mut self, service: ServiceType) -> Result<(), String> {
        // Ensure we have runtime paths
        if self.runtime_paths.is_none() {
            self.initialize()?;
        }

        // Clone the paths we need before the mutable borrow
        let paths = self
            .runtime_paths
            .as_ref()
            .ok_or("Runtime paths not initialized")?
            .clone();

        let service_snapshot = self
            .services
            .get(&service)
            .ok_or_else(|| format!("Service {:?} not found", service))?;
        if service_snapshot.state.is_running() {
            return Ok(());
        }

        let current_port = service_snapshot.port;
        let reserved_ports = self.reserved_ports_except(service);
        let selected_port = match service {
            ServiceType::Caddy => select_caddy_port(current_port, &paths.caddy, &reserved_ports)?,
            ServiceType::PhpFpm | ServiceType::MySQL | ServiceType::PostgreSQL => {
                select_available_port(service, current_port, &reserved_ports)?
            }
        };
        if selected_port != current_port {
            self.record_port_fallback(service, current_port, selected_port, &paths.logs_dir);
        }

        let web_port = self.web_port();
        let php_port = self.php_port();
        let mysql_port = self.mysql_port();
        let postgresql_port = self.postgresql_port();
        let service_process = self
            .services
            .get_mut(&service)
            .ok_or_else(|| format!("Service {:?} not found", service))?;

        service_process.state = ServiceState::Starting;

        // Spawn the appropriate service
        let result = match service {
            ServiceType::Caddy => start_caddy(
                service_process,
                &paths,
                php_port,
                mysql_port,
                postgresql_port,
                &self.settings.package_selection.phpmyadmin,
            ),
            ServiceType::PhpFpm => start_php_fpm(
                service_process,
                &paths,
                web_port,
                mysql_port,
                postgresql_port,
            ),
            ServiceType::MySQL => start_mysql(service_process, &paths),
            ServiceType::PostgreSQL => start_postgresql(service_process, &paths),
        };

        match result {
            Ok(_) => {
                service_process.state = ServiceState::Running;
                service_process.error_message = None;
                Ok(())
            }
            Err(e) => {
                service_process.state = ServiceState::Error;
                service_process.error_message = Some(e.clone());
                Err(e)
            }
        }
    }

    /// Stop a service
    pub fn stop(&mut self, service: ServiceType) -> Result<(), String> {
        let service_process = self
            .services
            .get_mut(&service)
            .ok_or_else(|| format!("Service {:?} not found", service))?;

        if !service_process.state.is_running() {
            return Ok(());
        }

        service_process.state = ServiceState::Stopping;

        // Terminate the child process if it exists
        if let Some(ref mut child) = service_process.child {
            #[cfg(unix)]
            {
                // On Unix, send SIGTERM
                let _ = child.kill();
            }

            #[cfg(windows)]
            {
                let _ = child.kill();
            }

            // Wait up to ~5s for the process to exit after the kill signal above.
            // If it is still alive at the deadline, send another kill and reap once more.
            wait_child_with_timeout(child, std::time::Duration::from_secs(5));
        }

        if let Some(pid) = service_process.external_pid.take() {
            let _ = terminate_process_by_pid(pid);
        }

        // สำหรับ MySQL: ตรวจสอบและหยุด processes ที่เหลือทั้งหมด
        if service == ServiceType::MySQL {
            if let Some(ref paths) = self.runtime_paths {
                // หยุด MySQL processes ทั้งหมดที่ยังค้างอยู่
                let _ = force_stop_all_mysql_processes(&paths.mysql);

                // รอให้ port ว่างนานขึ้น
                let _ = wait_for_port_release(service_process.port, mysql_port_release_timeout());
            }
        } else if service == ServiceType::PostgreSQL {
            if let Some(ref paths) = self.runtime_paths {
                let _ = stop_runtime_processes_by_executable(&paths.postgresql, "PostgreSQL");
                let _ = wait_for_port_release(service_process.port, mysql_port_release_timeout());
            }
        } else if service == ServiceType::Caddy {
            // สำหรับ Caddy: ตรวจสอบและหยุด processes ที่เหลือทั้งหมด
            if let Some(ref paths) = self.runtime_paths {
                let _ = force_stop_all_caddy_processes(&paths.caddy);
                let _ = wait_for_port_release(service_process.port, default_port_release_timeout());
            }
        } else {
            let _ = wait_for_port_release(service_process.port, default_port_release_timeout());
        }

        service_process.child = None;
        service_process.state = ServiceState::Stopped;

        Ok(())
    }

    /// Restart a service
    pub fn restart(&mut self, service: ServiceType) -> Result<(), String> {
        self.stop(service)?;
        self.start(service)?;
        Ok(())
    }

    pub fn start_all(&mut self) -> Result<(), String> {
        self.initialize()?;
        self.prepare_stack_ports()?;
        for service in stack_start_services(&self.settings.package_selection.phpmyadmin) {
            self.start(service)?;
        }
        Ok(())
    }

    pub fn restart_all(&mut self) -> Result<(), String> {
        self.stop_stack()?;
        self.start_all()
    }

    fn prepare_stack_ports(&mut self) -> Result<(), String> {
        let paths = self
            .runtime_paths
            .as_ref()
            .ok_or("Runtime paths not initialized")?
            .clone();

        for service in stack_start_services(&self.settings.package_selection.phpmyadmin) {
            let Some(service_process) = self.services.get(&service) else {
                continue;
            };
            if service_process.state.is_running() {
                continue;
            }

            let current_port = service_process.port;
            let reserved_ports = self.reserved_ports_except(service);
            let selected_port = match service {
                ServiceType::Caddy => {
                    select_caddy_port(current_port, &paths.caddy, &reserved_ports)?
                }
                ServiceType::PhpFpm | ServiceType::MySQL | ServiceType::PostgreSQL => {
                    select_available_port(service, current_port, &reserved_ports)?
                }
            };

            if selected_port != current_port {
                self.record_port_fallback(service, current_port, selected_port, &paths.logs_dir);
            }
        }

        Ok(())
    }

    fn record_port_fallback(
        &mut self,
        service: ServiceType,
        current_port: u16,
        selected_port: u16,
        logs_dir: &Path,
    ) {
        let message = format!(
            "{} port {} is in use; using fallback port {}.",
            service.display_name(),
            current_port,
            selected_port
        );
        let log_path = logs_dir.join("port-fallback.log");
        append_log_line(&log_path, &message);
        self.set_runtime_port(service, selected_port);
        if let Some(service_process) = self.services.get_mut(&service) {
            service_process.error_message = Some(message);
        }
    }

    /// Get the status of a service
    pub fn status(&self, service: ServiceType) -> ServiceState {
        self.services
            .get(&service)
            .map(|s| s.state.clone())
            .unwrap_or(ServiceState::Stopped)
    }

    /// Get all service statuses
    pub fn get_all_statuses(&self) -> ServiceMap {
        self.services
            .iter()
            .map(|(ty, proc)| {
                (
                    *ty,
                    ServiceInfo {
                        service_type: *ty,
                        state: proc.state.clone(),
                        port: proc.port,
                        error_message: proc.error_message.clone(),
                    },
                )
            })
            .collect()
    }

    /// Update process health (check if processes are still running)
    pub fn update_health(&mut self) {
        for service_process in self.services.values_mut() {
            if let Some(ref mut child) = service_process.child {
                match child.try_wait() {
                    Ok(Some(status)) => {
                        // Process has exited
                        service_process.state = ServiceState::Error;
                        service_process.error_message = Some(format_process_exit_error(
                            &format!(
                                "{} process exited unexpectedly",
                                service_process.name.display_name()
                            ),
                            status,
                            service_process.log_file.as_deref(),
                        ));
                        service_process.child = None;
                    }
                    Ok(None) => {
                        // Still running
                        service_process.state = ServiceState::Running;
                        service_process.error_message = None;
                    }
                    Err(_) => {
                        // Error checking status
                        service_process.state = ServiceState::Error;
                        service_process.error_message =
                            Some("Failed to check process status".to_string());
                    }
                }
            } else if let Some(pid) = service_process.external_pid {
                if process_exists(pid) {
                    service_process.state = ServiceState::Running;
                    service_process.error_message = None;
                } else {
                    service_process.state = ServiceState::Error;
                    service_process.error_message = Some(format!(
                        "{} process exited unexpectedly (pid {})",
                        service_process.name.display_name(),
                        pid
                    ));
                    service_process.external_pid = None;
                }
            }
        }
    }

    /// Stop the web stack and any database service that may have been active before a tool switch.
    pub fn stop_stack(&mut self) -> Result<(), String> {
        for service in [
            ServiceType::Caddy,
            ServiceType::PhpFpm,
            ServiceType::MySQL,
            ServiceType::PostgreSQL,
        ] {
            let Some(service_process) = self.services.get(&service) else {
                continue;
            };
            if service_process.state.is_running() {
                let _ = self.stop(service);
            }
        }

        Ok(())
    }

    /// Stop all running services (called on app shutdown)
    pub fn stop_all(&mut self) -> Result<(), String> {
        let services_to_stop: Vec<ServiceType> = self
            .services
            .iter()
            .filter(|(_, s)| s.state.is_running())
            .map(|(ty, _)| *ty)
            .collect();

        for service in services_to_stop {
            // Ignore errors during shutdown, just try to stop everything
            let _ = self.stop(service);
        }

        Ok(())
    }
}

/// Start Caddy web server
fn start_caddy(
    service_process: &mut ServiceProcess,
    paths: &RuntimePaths,
    php_port: u16,
    mysql_port: u16,
    postgresql_port: u16,
    database_tool_id: &str,
) -> Result<(), String> {
    // ตรวจสอบและหยุด Caddy processes ที่ซ้ำซ้อนอัตโนมัติ (เหมือน MySQL)
    cleanup_duplicate_caddy_processes(paths, service_process.port)?;

    if !wait_for_port_release(service_process.port, port_check_timeout()) {
        let stopped = stop_runtime_processes_by_executable(&paths.caddy, "Caddy")?;
        if stopped > 0 {
            let _ = wait_for_port_release(service_process.port, process_stop_wait_timeout());
        }
    }

    if !wait_for_port_release(service_process.port, port_check_timeout()) {
        return Err(format!(
            "Port {} is still in use. Stop the existing web server on this port and try again.",
            service_process.port
        ));
    }

    // Prepare the selected database tool in the writable config directory. This avoids writing into
    // Program Files or any other install directory that may require elevation.
    ensure_database_tool(
        paths,
        service_process.port,
        mysql_port,
        postgresql_port,
        database_tool_id,
    )?;

    // Always regenerate Caddyfile with current port settings
    let caddyfile_path = paths.config_dir.join("Caddyfile");
    generate_caddyfile(&caddyfile_path, paths, service_process.port, php_port)?;

    // Open log file with retry logic for Windows file locking
    let log_path = paths.logs_dir.join("caddy.log");
    let log_file = open_log_file_with_retry(&log_path, "Caddy")?;

    // Start Caddy
    let child = configure_no_window(Command::new(&paths.caddy))
        .arg("run")
        .arg("--config")
        .arg(&caddyfile_path)
        .current_dir(&paths.config_dir)
        .stdout(Stdio::from(log_file.try_clone().map_err(|e| {
            format!("Failed to duplicate Caddy log handle: {}", e)
        })?))
        .stderr(Stdio::from(log_file))
        .spawn()
        .map_err(|e| format!("Failed to start Caddy: {}", e))?;

    attach_started_process(service_process, child, log_path, "Caddy")
}

/// Start PHP-FPM (using PHP-CGI for simplicity in MVP)
fn start_php_fpm(
    service_process: &mut ServiceProcess,
    paths: &RuntimePaths,
    web_port: u16,
    mysql_port: u16,
    postgresql_port: u16,
) -> Result<(), String> {
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
            .stdout(Stdio::from(log_file.try_clone().map_err(|e| {
                format!("Failed to duplicate PHP-FPM log handle: {}", e)
            })?))
            .stderr(Stdio::from(log_file));
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
            .stdout(Stdio::from(log_file.try_clone().map_err(|e| {
                format!("Failed to duplicate PHP-CGI log handle: {}", e)
            })?))
            .stderr(Stdio::from(log_file));
        cmd.spawn()
            .map_err(|e| format!("Failed to start PHP-CGI: {}", e))?
    };

    attach_started_process(service_process, child, log_path, "PHP")
}

/// Start MySQL/MariaDB database server
///
/// **IMPORTANT Platform Differences:**
/// - **Linux**: Uses MariaDB 12.x (binary: mariadbd)
/// - **Windows/macOS**: Uses MySQL 8.x (binary: mysqld)
///
/// These are drop-in replacements for each other, but have different
/// initialization requirements and binary names.
fn start_mysql(service_process: &mut ServiceProcess, paths: &RuntimePaths) -> Result<(), String> {
    // Initialize MySQL data directory if needed
    initialize_mysql_data_dir(paths)?;

    // ตรวจสอบและหยุด MySQL processes ที่ซ้ำซ้อนอัตโนมัติ
    cleanup_duplicate_mysql_processes(paths, service_process.port)?;

    if let Some(pid) = find_running_mysql_pid(paths, service_process.port) {
        let log_path = paths.logs_dir.join("mysql.log");
        append_log_line(
            &log_path,
            &format!(
                "CHAMP found an existing MySQL process (pid {}) on 127.0.0.1:{} and will reuse it.",
                pid, service_process.port
            ),
        );
        attach_existing_mysql_process(service_process, pid, log_path.clone());
        return Ok(());
    }

    // Check if port is available before attempting to start
    if !port_can_bind(service_process.port) {
        // พยายามหยุด process ที่ใช้ port นี้อยู่
        let log_path = paths.logs_dir.join("mysql.log");
        append_log_line(
            &log_path,
            &format!(
                "Port {} is in use. Attempting to stop conflicting MySQL processes...",
                service_process.port
            ),
        );

        let stopped = stop_runtime_processes_by_executable(&paths.mysql, "MySQL/MariaDB")?;
        if stopped > 0 {
            append_log_line(
                &log_path,
                &format!("Stopped {} conflicting MySQL process(es)", stopped),
            );
            // รอให้ port ว่าง
            let _ = wait_for_port_release(service_process.port, process_stop_wait_timeout());
        }

        // ตรวจสอบอีกครั้ง
        if !port_can_bind(service_process.port) {
            return Err(format!(
                "Port {} is still in use after cleanup. MySQL cannot start.\n\
                Please manually stop any MySQL/MariaDB processes and try again.\n\
                You can change the MySQL port in Settings.",
                service_process.port
            ));
        }
    }

    // Verify MySQL data directory integrity before starting
    verify_mysql_data_integrity(paths)?;

    // Clean path and use proper Windows format for MySQL
    let data_dir_str = paths.mysql_data_dir.to_string_lossy().to_string();
    let data_dir_str = data_dir_str.trim_end_matches('\\').trim_end_matches('/');

    // Check if we need to create 127.0.0.1 user (first run)
    let user_created_flag = paths.mysql_data_dir.join(".user_127_0_0_1_created");
    let needs_init_file = !user_created_flag.exists();

    let init_file_path = if needs_init_file {
        // Create init file to add root@127.0.0.1 user
        let init_file = paths.logs_dir.join("mysql_init_user.sql");
        fs::write(
            &init_file,
            "CREATE USER IF NOT EXISTS 'root'@'127.0.0.1' IDENTIFIED BY '';\n\
            GRANT ALL PRIVILEGES ON *.* TO 'root'@'127.0.0.1' WITH GRANT OPTION;\n\
            FLUSH PRIVILEGES;\n",
        )
        .map_err(|e| format!("Failed to create init file: {}", e))?;
        Some(init_file)
    } else {
        None
    };

    // Open log file with retry logic
    let log_path = paths.logs_dir.join("mysql.log");
    let log_file = open_log_file_with_retry(&log_path, "MariaDB")?;

    // Build MySQL command with optional init file
    let mut cmd = configure_no_window(Command::new(&paths.mysql));
    cmd.arg("--datadir")
        .arg(data_dir_str)
        .arg("--port")
        .arg(service_process.port.to_string())
        .arg("--bind-address=127.0.0.1")
        .arg("--console")
        .arg("--skip-name-resolve");

    // Add init file on first run
    if let Some(ref init_file) = init_file_path {
        cmd.arg("--init-file").arg(init_file);
    }

    let mut child = cmd
        .stdout(Stdio::from(log_file.try_clone().map_err(|e| {
            format!("Failed to duplicate MySQL log handle: {}", e)
        })?))
        .stderr(Stdio::from(log_file))
        .spawn()
        .map_err(|e| {
            let log_content = fs::read_to_string(&log_path)
                .unwrap_or_else(|_| String::from("Could not read log"));
            format!(
                "Failed to start MariaDB: {}\n\nMariaDB log:\n{}",
                e, log_content
            )
        })?;

    // Check if process is still running
    match child.try_wait() {
        Ok(Some(status)) => {
            // Clean up init file if it exists
            if let Some(init_file) = init_file_path {
                let _ = fs::remove_file(&init_file);
            }

            // Enhanced error message with troubleshooting tips
            let mut error_msg = format_process_exit_error(
                "MySQL/MariaDB exited immediately",
                status,
                Some(&log_path),
            );

            // Add troubleshooting suggestions
            error_msg.push_str("\n\n=== Troubleshooting Tips ===");
            error_msg.push_str("\n1. Check if port ");
            error_msg.push_str(&service_process.port.to_string());
            error_msg.push_str(" is already in use by another application");
            error_msg.push_str("\n2. Verify data directory permissions at: ");
            error_msg.push_str(&paths.mysql_data_dir.display().to_string());
            error_msg.push_str("\n3. Try stopping all services and restarting CHAMP");
            error_msg.push_str(
                "\n4. If the problem persists, you may need to reinitialize the database:",
            );
            error_msg.push_str("\n   - Stop CHAMP completely");
            error_msg.push_str("\n   - Delete the MySQL data directory");
            error_msg.push_str("\n   - Restart CHAMP to reinitialize");

            Err(error_msg)
        }
        Ok(None) => {
            if needs_init_file {
                let marker_path = user_created_flag.clone();
                let init_cleanup_path = init_file_path.clone();
                let mysql_port = service_process.port;
                let mysql_client = database_client_binary(&paths.mysql);
                std::thread::spawn(move || {
                    for _ in 0..MYSQL_INIT_MAX_RETRIES {
                        if mysql_root_tcp_login_works(&mysql_client, mysql_port) {
                            let _ = fs::write(marker_path, "done");
                            break;
                        }
                        std::thread::sleep(mysql_init_check_delay());
                    }
                    if let Some(init_file) = init_cleanup_path {
                        let _ = fs::remove_file(init_file);
                    }
                });
            }
            attach_started_process(service_process, child, log_path.clone(), "MariaDB")?;
            Ok(())
        }
        Err(e) => {
            if let Some(init_file) = init_file_path {
                let _ = fs::remove_file(&init_file);
            }
            Err(format!("Failed to check MariaDB process: {}", e))
        }
    }
}

fn start_postgresql(
    service_process: &mut ServiceProcess,
    paths: &RuntimePaths,
) -> Result<(), String> {
    initialize_postgresql_data_dir(paths)?;
    cleanup_stale_postgresql_pid(paths);

    if let Some(pid) = find_running_postgresql_pid(paths, service_process.port) {
        let log_path = paths.logs_dir.join("postgresql.log");
        append_log_line(
            &log_path,
            &format!(
                "CHAMP found an existing PostgreSQL process (pid {}) on 127.0.0.1:{} and will reuse it.",
                pid, service_process.port
            ),
        );
        service_process.child = None;
        service_process.external_pid = Some(pid);
        service_process.log_file = Some(log_path);
        return Ok(());
    }

    if !port_can_bind(service_process.port) {
        let log_path = paths.logs_dir.join("postgresql.log");
        append_log_line(
            &log_path,
            &format!(
                "Port {} is in use. Attempting to stop conflicting PostgreSQL processes...",
                service_process.port
            ),
        );

        let stopped = stop_runtime_processes_by_executable(&paths.postgresql, "PostgreSQL")?;
        if stopped > 0 {
            let _ = wait_for_port_release(service_process.port, process_stop_wait_timeout());
        }

        if !port_can_bind(service_process.port) {
            return Err(format!(
                "Port {} is still in use after cleanup. PostgreSQL cannot start. You can change the PostgreSQL port in Settings.",
                service_process.port
            ));
        }
    }

    let log_path = paths.logs_dir.join("postgresql.log");
    let log_file = open_log_file_with_retry(&log_path, "PostgreSQL")?;

    let mut cmd = configure_no_window(Command::new(&paths.postgresql));
    cmd.arg("-D")
        .arg(&paths.postgresql_data_dir)
        .arg("-p")
        .arg(service_process.port.to_string())
        .arg("-h")
        .arg("127.0.0.1")
        .stdout(Stdio::from(log_file.try_clone().map_err(|e| {
            format!("Failed to duplicate PostgreSQL log handle: {}", e)
        })?))
        .stderr(Stdio::from(log_file));

    let child = cmd.spawn().map_err(|e| {
        let log_content =
            fs::read_to_string(&log_path).unwrap_or_else(|_| "Could not read log".to_string());
        format!(
            "Failed to start PostgreSQL: {}\n\nPostgreSQL log:\n{}",
            e, log_content
        )
    })?;

    attach_started_process(service_process, child, log_path, "PostgreSQL")
}

fn initialize_postgresql_data_dir(paths: &RuntimePaths) -> Result<(), String> {
    if paths.postgresql_data_dir.join("PG_VERSION").exists() {
        return Ok(());
    }

    fs::create_dir_all(&paths.postgresql_data_dir)
        .map_err(|e| format!("Failed to create PostgreSQL data directory: {}", e))?;

    let initdb = postgresql_initdb_binary(&paths.postgresql);
    if !initdb.exists() {
        return Err(format!(
            "PostgreSQL initdb binary not found at {}. Please ensure the PostgreSQL runtime was downloaded correctly.",
            initdb.display()
        ));
    }

    let init_log_path = paths.logs_dir.join("postgresql_init.log");
    let init_log_file = File::create(&init_log_path)
        .map_err(|e| format!("Failed to create PostgreSQL init log file: {}", e))?;

    let mut cmd = configure_no_window(Command::new(&initdb));
    cmd.arg("-D")
        .arg(&paths.postgresql_data_dir)
        .arg("-U")
        .arg("postgres")
        // S-07: `trust` auth is acceptable here because CHAMP is a local-only
        // development tool. PostgreSQL is bound to 127.0.0.1 (see the `-h`
        // argument in start_postgresql), so it is not reachable from the
        // network, and trust auth avoids forcing users to manage a password
        // for throwaway local databases. Any concern about exposure via the
        // HTTPS Preview / tunnel feature is tracked separately as S-03.
        .arg("--auth=trust")
        .arg("-E")
        .arg("UTF8")
        .stdout(Stdio::from(init_log_file.try_clone().map_err(|e| {
            format!("Failed to duplicate PostgreSQL init log handle: {}", e)
        })?))
        .stderr(Stdio::from(init_log_file));

    let status = cmd
        .status()
        .map_err(|e| format!("Failed to initialize PostgreSQL data directory: {}", e))?;
    if status.success() {
        return Ok(());
    }

    Err(format_process_exit_error(
        "PostgreSQL initialization failed",
        status,
        Some(&init_log_path),
    ))
}

fn cleanup_stale_postgresql_pid(paths: &RuntimePaths) {
    let pid_file = paths.postgresql_data_dir.join("postmaster.pid");
    let Some(pid) = read_postgresql_pid_file(&pid_file) else {
        return;
    };
    if !process_exists(pid) {
        let _ = fs::remove_file(pid_file);
    }
}

fn find_running_postgresql_pid(paths: &RuntimePaths, port: u16) -> Option<u32> {
    let pid = read_postgresql_pid_file(&paths.postgresql_data_dir.join("postmaster.pid"))?;
    if !process_exists(pid) {
        return None;
    }

    for _ in 0..20 {
        if tcp_port_accepts(port) {
            return Some(pid);
        }
        thread::sleep(std::time::Duration::from_millis(250));
    }

    None
}

fn read_postgresql_pid_file(pid_file: &Path) -> Option<u32> {
    fs::read_to_string(pid_file)
        .ok()?
        .lines()
        .next()?
        .trim()
        .parse::<u32>()
        .ok()
}

fn attach_existing_mysql_process(
    service_process: &mut ServiceProcess,
    pid: u32,
    log_path: PathBuf,
) {
    service_process.child = None;
    service_process.external_pid = Some(pid);
    service_process.log_file = Some(log_path);
}

fn find_running_mysql_pid(paths: &RuntimePaths, port: u16) -> Option<u32> {
    let pid = read_mysql_pid_file(&paths.mysql_data_dir)?;
    if !process_exists(pid) {
        return None;
    }

    for _ in 0..20 {
        if tcp_port_accepts(port) {
            return Some(pid);
        }
        thread::sleep(std::time::Duration::from_millis(250));
    }

    None
}

fn read_mysql_pid_file(data_dir: &Path) -> Option<u32> {
    let entries = fs::read_dir(data_dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        let is_pid = path
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| value.eq_ignore_ascii_case("pid"))
            .unwrap_or(false);
        if !is_pid {
            continue;
        }

        let Ok(content) = fs::read_to_string(&path) else {
            continue;
        };
        if let Ok(pid) = content.trim().parse::<u32>() {
            return Some(pid);
        }
    }

    None
}

fn append_log_line(log_path: &Path, message: &str) {
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(log_path) {
        let _ = writeln!(file, "{}", message);
    }
}

fn attach_started_process(
    service_process: &mut ServiceProcess,
    mut child: Child,
    log_path: PathBuf,
    service_label: &str,
) -> Result<(), String> {
    match child.try_wait() {
        Ok(Some(status)) => Err(format_process_exit_error(
            &format!("{} exited immediately", service_label),
            status,
            Some(&log_path),
        )),
        Ok(None) => {
            service_process.child = Some(child);
            service_process.log_file = Some(log_path);
            Ok(())
        }
        Err(e) => Err(format!("Failed to check {} process: {}", service_label, e)),
    }
}

fn database_client_binary(server_binary: &Path) -> PathBuf {
    let client_name = if cfg!(target_os = "windows") {
        "mysql.exe"
    } else {
        "mysql"
    };

    server_binary
        .parent()
        .map(|bin_dir| bin_dir.join(client_name))
        .unwrap_or_else(|| PathBuf::from(client_name))
}

fn mysql_root_tcp_login_works(mysql_client: &Path, port: u16) -> bool {
    if !mysql_client.exists() {
        return false;
    }

    let port_arg = port.to_string();
    let mut cmd = configure_no_window(Command::new(mysql_client));
    cmd.args([
        "--protocol=TCP",
        "-h",
        "127.0.0.1",
        "-P",
        &port_arg,
        "-u",
        "root",
        "--password=",
        "--connect-timeout=2",
        "-e",
        "SELECT 1",
    ])
    .stdout(Stdio::null())
    .stderr(Stdio::null())
    .status()
    .map(|status| status.success())
    .unwrap_or(false)
}

/// Verify MySQL data directory integrity
/// Checks for common issues that cause MySQL to fail on startup
fn verify_mysql_data_integrity(paths: &RuntimePaths) -> Result<(), String> {
    let mysql_dir = paths.mysql_data_dir.join("mysql");

    // Check if mysql system directory exists
    if !mysql_dir.exists() {
        return Err(format!(
            "MySQL system directory not found at {:?}. \
            The data directory may be corrupted. \
            Try stopping all services and restarting CHAMP.",
            mysql_dir
        ));
    }

    // Check for lock files that might prevent startup
    let lock_files = [
        paths.mysql_data_dir.join("ibdata1.lock"),
        paths.mysql_data_dir.join("ib_logfile0.lock"),
        paths.mysql_data_dir.join("mysql.sock.lock"),
    ];

    for lock_file in &lock_files {
        if lock_file.exists() {
            eprintln!(
                "Warning: Found stale lock file at {:?}, removing...",
                lock_file
            );
            let _ = fs::remove_file(lock_file);
        }
    }

    // Check for PID files from crashed processes
    if let Ok(entries) = fs::read_dir(&paths.mysql_data_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("pid") {
                if let Ok(content) = fs::read_to_string(&path) {
                    if let Ok(pid) = content.trim().parse::<u32>() {
                        if !process_exists(pid) {
                            eprintln!("Warning: Found stale PID file at {:?}, removing...", path);
                            let _ = fs::remove_file(&path);
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

/// Initialize MySQL/MariaDB data directory
///
/// **IMPORTANT Platform Differences:**
///
/// **Linux (MariaDB 12.x):**
/// - MariaDB 12.x removed the --initialize-insecure flag
/// - Server auto-initializes on first startup
/// - No manual initialization required
///
/// **Windows/macOS (MySQL 8.x):**
/// - Uses --initialize-insecure flag
/// - Requires explicit initialization before first use
/// - Creates system tables and sets up data directory
fn initialize_mysql_data_dir(paths: &RuntimePaths) -> Result<(), String> {
    // Check if already initialized by looking for mysql system tables
    let mysql_dir = paths.mysql_data_dir.join("mysql");
    if mysql_dir.exists() {
        // MySQL 8.4+ uses .sdi files (Schema Data Information) for table metadata
        // MariaDB 12.x also uses similar system
        // Check if any .sdi files exist in the mysql directory
        let entries: Vec<_> = mysql_dir
            .read_dir()
            .and_then(|e| e.collect::<Result<_, _>>())
            .unwrap_or_default();

        let has_sdi_files = entries.iter().any(|entry| {
            entry
                .path()
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext.eq_ignore_ascii_case("sdi"))
                .unwrap_or(false)
        });

        if has_sdi_files {
            // Already initialized
            #[cfg(target_os = "linux")]
            eprintln!("MariaDB data directory already initialized");
            #[cfg(not(target_os = "linux"))]
            eprintln!("MySQL data directory already initialized");
            return Ok(());
        }
    }

    // Create the data directory if it doesn't exist
    #[cfg(target_os = "linux")]
    fs::create_dir_all(&paths.mysql_data_dir)
        .map_err(|e| format!("Failed to create MariaDB data directory: {}", e))?;
    #[cfg(not(target_os = "linux"))]
    fs::create_dir_all(&paths.mysql_data_dir)
        .map_err(|e| format!("Failed to create MySQL data directory: {}", e))?;

    // Get clean path with forward slashes (Windows fix)
    let data_dir_str = paths.mysql_data_dir.to_string_lossy().replace('\\', "/");

    #[cfg(target_os = "linux")]
    {
        let uses_mysql_server = paths
            .mysql
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| name == "mysqld")
            .unwrap_or(false);

        if uses_mysql_server {
            initialize_mysqld_data_dir(paths, &data_dir_str, &mysql_dir)?;
        } else {
            // ============================================================
            // LINUX: MariaDB 12.x Initialization
            // ============================================================
            // MariaDB 12.x does NOT support --initialize-insecure flag
            // (removed in MariaDB 10.4+)
            //
            // Instead, we use the mariadb-install-db script which:
            // - Creates the mysql system database
            // - Initializes privilege tables
            // - Sets up default users (root@localhost with no password)
            // ============================================================

            eprintln!("MariaDB 12.x: Initializing data directory using mariadb-install-db");

            // Find the mariadb-install-db script
            let mariadbd_dir = paths
                .mysql
                .parent()
                .ok_or("Failed to get MariaDB binary directory")?;

            let mut install_db_script = mariadbd_dir
                .parent()
                .ok_or("Failed to get MariaDB base directory")?
                .join("scripts")
                .join("mariadb-install-db");

            if !install_db_script.exists() {
                // Fallback to mysql_install_db (older name)
                let install_db_script_fallback = mariadbd_dir
                    .parent()
                    .ok_or("Failed to get MariaDB base directory")?
                    .join("scripts")
                    .join("mysql_install_db");

                if !install_db_script_fallback.exists() {
                    return Err(format!(
                        "MariaDB installation script not found. Tried:\n  - {}\n  - {}\n\
                    Please ensure the MariaDB runtime was downloaded correctly.",
                        install_db_script.display(),
                        install_db_script_fallback.display()
                    ));
                }

                install_db_script = install_db_script_fallback;
            }

            let init_log_path = paths.logs_dir.join("mysql_init.log");
            let init_log_file = fs::File::create(&init_log_path)
                .map_err(|e| format!("Failed to create init log file: {}", e))?;

            // Run mariadb-install-db
            // Key parameters:
            // --datadir=DIR: Location of database files
            // --basedir=PATH: Path to MariaDB installation
            // --user=: Run as current user (not root)
            let mut cmd = configure_no_window(Command::new(&install_db_script));
            cmd.arg(format!("--datadir={}", data_dir_str))
                .arg(format!(
                    "--basedir={}",
                    mariadbd_dir.parent().unwrap().display()
                ))
                .arg("--user=") // Empty string = current user
                .stdout(Stdio::from(init_log_file.try_clone().map_err(|e| {
                    format!("Failed to duplicate MariaDB init log handle: {}", e)
                })?))
                .stderr(Stdio::from(init_log_file));

            let mut child = cmd
                .spawn()
                .map_err(|e| format!("Failed to start MariaDB initialization: {}", e))?;

            // Wait for initialization with longer timeout (120 seconds)
            let timeout = std::time::Duration::from_secs(120);
            let start = std::time::Instant::now();

            let mut output = String::new();
            let success = loop {
                match child.try_wait() {
                    Ok(Some(status)) => {
                        // Read any remaining output
                        let _ = fs::read_to_string(&init_log_path).map(|s| output = s);
                        break status.success();
                    }
                    Ok(None) => {
                        if start.elapsed() > timeout {
                            eprintln!("MariaDB initialization timeout, killing process");
                            let _ = child.kill();
                            // Force wait to get final status
                            let _ = child.wait();
                            let _ = fs::read_to_string(&init_log_path).map(|s| output = s);
                            break false;
                        }
                        std::thread::sleep(std::time::Duration::from_millis(500));
                    }
                    Err(_) => {
                        let _ = fs::read_to_string(&init_log_path).map(|s| output = s);
                        break false;
                    }
                }
            };

            if !success {
                eprintln!("MariaDB initialization failed. Output:\n{}", output);
                return Err(format!(
                    "MariaDB initialization failed. Check the log file at: {:?}",
                    init_log_path
                ));
            }

            eprintln!("MariaDB initialization completed successfully");

            // Verify that mysql directory was created
            if !mysql_dir.exists() {
                return Err(format!(
                    "MariaDB initialization failed - mysql directory not created at {:?}. \
                 Check the log file at: {:?}",
                    mysql_dir, init_log_path
                ));
            }
        }
    }

    #[cfg(not(target_os = "linux"))]
    {
        initialize_mysqld_data_dir(paths, &data_dir_str, &mysql_dir)?;
    }

    Ok(())
}

fn initialize_mysqld_data_dir(
    paths: &RuntimePaths,
    data_dir_str: &str,
    mysql_dir: &Path,
) -> Result<(), String> {
    // MySQL 8.0+ and 9.x initialize local data directories with mysqld.
    eprintln!("MySQL: Initializing data directory at: {}", data_dir_str);

    let init_log_path = paths.logs_dir.join("mysql_init.log");
    let init_log_file = fs::File::create(&init_log_path)
        .map_err(|e| format!("Failed to create init log file: {}", e))?;

    let mut child = configure_no_window(Command::new(&paths.mysql))
        .arg("--initialize-insecure")
        .arg("--datadir")
        .arg(data_dir_str)
        .arg("--console")
        .stdout(Stdio::from(init_log_file.try_clone().map_err(|e| {
            format!("Failed to duplicate MySQL init log handle: {}", e)
        })?))
        .stderr(Stdio::from(init_log_file))
        .spawn()
        .map_err(|e| format!("Failed to start MySQL initialization: {}", e))?;

    let timeout = std::time::Duration::from_secs(120);
    let start = std::time::Instant::now();

    let mut output = String::new();
    let success = loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let _ = fs::read_to_string(&init_log_path).map(|s| output = s);
                break status.success();
            }
            Ok(None) => {
                if start.elapsed() > timeout {
                    eprintln!("MySQL initialization timeout, killing process");
                    let _ = child.kill();
                    let _ = child.wait();
                    let _ = fs::read_to_string(&init_log_path).map(|s| output = s);
                    break false;
                }
                std::thread::sleep(std::time::Duration::from_millis(500));
            }
            Err(_) => {
                let _ = fs::read_to_string(&init_log_path).map(|s| output = s);
                break false;
            }
        }
    };

    if !success {
        eprintln!("MySQL initialization failed. Output:\n{}", output);
        return Err(format!(
            "MySQL initialization failed. Check the log file at: {:?}",
            init_log_path
        ));
    }

    eprintln!("MySQL initialization completed successfully");

    if !mysql_dir.exists() {
        return Err(format!(
            "MySQL initialization failed - mysql directory not created at {:?}. \
             Check the log file at: {:?}",
            mysql_dir, init_log_path
        ));
    }

    Ok(())
}

impl Default for ProcessManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    fn nonzero_exit_status() -> ExitStatus {
        Command::new("cmd")
            .args(["/C", "exit 7"])
            .status()
            .expect("failed to create nonzero exit status")
    }

    #[cfg(not(windows))]
    fn nonzero_exit_status() -> ExitStatus {
        Command::new("sh")
            .args(["-c", "exit 7"])
            .status()
            .expect("failed to create nonzero exit status")
    }

    // R-03: MySQL cleanup must match processes by *full path*, not by executable
    // name. A binary named `mysqld[.exe]` living under a CHAMP runtime path that
    // has nothing running from it must yield zero PIDs, even if the host has its
    // own MySQL/MariaDB of the same name running elsewhere. This guards against
    // force-killing a user's own database. The name-only `find_all_mysql_processes`
    // helper was removed; MySQL now shares Caddy/PostgreSQL's path-verified finder.
    #[test]
    fn test_find_process_ids_by_executable_ignores_same_name_other_path() {
        let temp = tempfile::tempdir().expect("failed to create temp dir");
        let exe_name = if cfg!(windows) {
            "mysqld.exe"
        } else {
            "mysqld"
        };
        // A path under a throwaway dir: nothing is ever launched from here, so
        // regardless of any real mysqld running on the machine, the result is empty.
        let fake_mysql = temp.path().join("runtime").join("mysql").join(exe_name);

        let pids = find_process_ids_by_executable(&fake_mysql)
            .expect("finder should succeed even when no matching process exists");

        assert!(
            pids.is_empty(),
            "expected no PIDs for a same-named binary at an unused path, got {:?}",
            pids
        );
    }

    #[test]
    fn test_format_exit_status_human_readable() {
        let status = nonzero_exit_status();
        let formatted = format_exit_status(status);
        assert!(formatted.contains("7"), "unexpected format: {}", formatted);
        assert!(
            !formatted.contains("ExitStatus("),
            "status should be user-friendly: {}",
            formatted
        );
    }

    #[test]
    fn test_format_process_exit_error_includes_log_tail() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time before UNIX_EPOCH")
            .as_nanos();
        let log_path = std::env::temp_dir().join(format!("champ-process-error-{}.log", unique));
        std::fs::write(
            &log_path,
            "line 1\nline 2\nline 3\nfatal startup error on line 4\n",
        )
        .expect("failed to write temp log");

        let status = nonzero_exit_status();
        let message = format_process_exit_error(
            "Service exited unexpectedly",
            status,
            Some(log_path.as_path()),
        );

        assert!(message.contains("exit code 7"));
        assert!(message.contains("Log file:"));
        assert!(message.contains("fatal startup error on line 4"));

        let _ = std::fs::remove_file(log_path);
    }

    #[test]
    fn test_process_manager_new() {
        let manager = ProcessManager::new();

        assert_eq!(manager.services.len(), 4);

        let caddy = manager.services.get(&ServiceType::Caddy).unwrap();
        assert_eq!(caddy.name, ServiceType::Caddy);
        assert_eq!(caddy.state, ServiceState::Stopped);
        assert_eq!(caddy.port, 8080);
        assert!(caddy.child.is_none());

        let postgresql = manager.services.get(&ServiceType::PostgreSQL).unwrap();
        assert_eq!(postgresql.name, ServiceType::PostgreSQL);
        assert_eq!(postgresql.state, ServiceState::Stopped);
        assert_eq!(postgresql.port, 5432);
    }

    #[test]
    fn test_process_manager_default() {
        let manager = ProcessManager::default();
        assert_eq!(manager.services.len(), 4);
        assert!(manager.runtime_paths.is_none());
    }

    #[test]
    fn test_status_of_service() {
        let manager = ProcessManager::new();

        assert_eq!(manager.status(ServiceType::Caddy), ServiceState::Stopped);
        assert_eq!(manager.status(ServiceType::PhpFpm), ServiceState::Stopped);
        assert_eq!(manager.status(ServiceType::MySQL), ServiceState::Stopped);
        assert_eq!(
            manager.status(ServiceType::PostgreSQL),
            ServiceState::Stopped
        );
    }

    #[test]
    fn test_get_all_statuses() {
        let manager = ProcessManager::new();
        let statuses = manager.get_all_statuses();

        assert_eq!(statuses.len(), 4);

        let caddy_info = statuses.get(&ServiceType::Caddy).unwrap();
        assert_eq!(caddy_info.service_type, ServiceType::Caddy);
        assert_eq!(caddy_info.state, ServiceState::Stopped);
        assert_eq!(caddy_info.port, 8080);
    }

    #[test]
    fn test_stop_already_stopped_service() {
        let mut manager = ProcessManager::new();

        let result = manager.stop(ServiceType::Caddy);
        assert!(result.is_ok());
        assert_eq!(manager.status(ServiceType::Caddy), ServiceState::Stopped);
    }

    #[test]
    fn test_service_error_state_handling() {
        let mut manager = ProcessManager::new();

        let service = manager.services.get_mut(&ServiceType::MySQL).unwrap();
        service.state = ServiceState::Error;
        service.error_message = Some("Test error".to_string());

        assert_eq!(manager.status(ServiceType::MySQL), ServiceState::Error);

        let statuses = manager.get_all_statuses();
        let mysql_info = statuses.get(&ServiceType::MySQL).unwrap();
        assert_eq!(mysql_info.state, ServiceState::Error);
        assert_eq!(mysql_info.error_message, Some("Test error".to_string()));
    }

    #[test]
    fn test_update_health_with_no_processes() {
        let mut manager = ProcessManager::new();

        manager.update_health();

        assert_eq!(manager.status(ServiceType::Caddy), ServiceState::Stopped);
        assert_eq!(manager.status(ServiceType::PhpFpm), ServiceState::Stopped);
        assert_eq!(manager.status(ServiceType::MySQL), ServiceState::Stopped);
        assert_eq!(
            manager.status(ServiceType::PostgreSQL),
            ServiceState::Stopped
        );
    }

    #[test]
    fn test_port_assignment_for_services() {
        let manager = ProcessManager::new();

        let caddy = manager.services.get(&ServiceType::Caddy).unwrap();
        assert_eq!(caddy.port, 8080);

        let php = manager.services.get(&ServiceType::PhpFpm).unwrap();
        assert_eq!(php.port, 9000);

        let mysql = manager.services.get(&ServiceType::MySQL).unwrap();
        assert_eq!(mysql.port, 3306);

        let postgresql = manager.services.get(&ServiceType::PostgreSQL).unwrap();
        assert_eq!(postgresql.port, 5432);
    }

    #[test]
    fn test_find_available_port_skips_busy_preferred_port() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("failed to bind temporary port");
        let preferred_port = listener
            .local_addr()
            .expect("failed to read temporary port")
            .port();

        let selected = find_available_port_excluding(ServiceType::Caddy, preferred_port, &[])
            .expect("expected fallback port");

        assert_ne!(selected, preferred_port);
    }

    #[test]
    fn test_find_available_port_skips_reserved_stack_ports() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("failed to bind temporary port");
        let preferred_port = listener
            .local_addr()
            .expect("failed to read temporary port")
            .port();
        drop(listener);

        let selected =
            find_available_port_excluding(ServiceType::PhpFpm, preferred_port, &[preferred_port])
                .expect("expected fallback port");

        assert_ne!(selected, preferred_port);
    }

    #[test]
    fn test_mysql_default_fallback_does_not_use_first_adjacent_port() {
        assert_eq!(
            first_fallback_port(ServiceType::MySQL, crate::config::DEFAULT_PORTS.mysql),
            crate::config::DEFAULT_PORTS.mysql + 2
        );
    }

    #[test]
    fn test_htaccess_parser_maps_extensionless_php_and_error_document() {
        let temp = tempfile::tempdir().expect("failed to create temp dir");
        let project = temp.path().join("IT-NVC");
        std::fs::create_dir_all(&project).expect("failed to create project dir");
        std::fs::write(
            project.join(".htaccess"),
            r#"
RewriteEngine On
ErrorDocument 404 /404.php
RewriteCond %{REQUEST_FILENAME} !-d
RewriteCond %{REQUEST_FILENAME} !-f
RewriteRule (.*) $1.php [L]
"#,
        )
        .expect("failed to write htaccess");

        let rules = discover_htaccess_rules(temp.path());

        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].request_prefix, "/IT-NVC");
        assert_eq!(rules[0].project_prefix, "/IT-NVC");
        assert!(rules[0].extensionless_php);
        assert_eq!(rules[0].error_404.as_deref(), Some("/IT-NVC/404.php"));
    }

    #[test]
    fn test_htaccess_parser_maps_nested_relative_front_controller() {
        let temp = tempfile::tempdir().expect("failed to create temp dir");
        let public = temp.path().join("demo").join("public");
        std::fs::create_dir_all(&public).expect("failed to create public dir");
        std::fs::write(
            public.join(".htaccess"),
            r#"
RewriteEngine On
RewriteCond %{REQUEST_FILENAME} !-d
RewriteCond %{REQUEST_FILENAME} !-f
RewriteRule ^ index.php [L]
Options -Indexes
"#,
        )
        .expect("failed to write htaccess");

        let rules = discover_htaccess_rules(temp.path());

        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].request_prefix, "/demo/public");
        assert_eq!(
            rules[0].front_controller.as_deref(),
            Some("/demo/public/index.php")
        );
        assert!(rules[0].disable_indexes);
    }

    #[test]
    fn test_htaccess_parser_maps_php_directory_index() {
        let temp = tempfile::tempdir().expect("failed to create temp dir");
        let project = temp.path().join("demo");
        std::fs::create_dir_all(&project).expect("failed to create project dir");
        std::fs::write(
            project.join(".htaccess"),
            "DirectoryIndex home.php index.html index.php\n",
        )
        .expect("failed to write htaccess");

        let rules = discover_htaccess_rules(temp.path());

        assert_eq!(rules.len(), 1);
        assert_eq!(
            rules[0].directory_indexes,
            vec!["home.php".to_string(), "index.php".to_string()]
        );
    }

    #[test]
    fn test_generate_caddyfile_includes_htaccess_compatibility_rules() {
        let temp = tempfile::tempdir().expect("failed to create temp dir");
        let projects = temp.path().join("projects");
        let config = temp.path().join("config");
        let logs = temp.path().join("logs");
        let adminer = temp.path().join("adminer");
        let project = projects.join("IT-NVC");
        std::fs::create_dir_all(&project).expect("failed to create project dir");
        std::fs::create_dir_all(&config).expect("failed to create config dir");
        std::fs::create_dir_all(&logs).expect("failed to create logs dir");
        std::fs::create_dir_all(&adminer).expect("failed to create adminer dir");
        std::fs::write(
            project.join(".htaccess"),
            "RewriteEngine On\nRewriteRule (.*) $1.php [L]\nErrorDocument 404 /404.php\n",
        )
        .expect("failed to write htaccess");

        let runtime_paths = RuntimePaths {
            caddy: temp.path().join("caddy"),
            php_cgi: temp.path().join("php-cgi"),
            php_ini: temp.path().join("php.ini"),
            mysql: temp.path().join("mysql"),
            postgresql: temp.path().join("postgres"),
            adminer,
            node: None,
            python: None,
            go: None,
            ruby: None,
            mysql_data_dir: temp.path().join("mysql-data"),
            postgresql_data_dir: temp.path().join("postgresql-data"),
            logs_dir: logs,
            config_dir: config.clone(),
            projects_dir: projects,
        };
        let caddyfile = config.join("Caddyfile");

        generate_caddyfile(&caddyfile, &runtime_paths, 8080, 9000)
            .expect("failed to generate Caddyfile");
        let content = std::fs::read_to_string(caddyfile).expect("failed to read Caddyfile");

        assert!(content.contains("@htaccessExtPhp_0_IT_NVC"));
        assert!(content.contains("rewrite * {file_match.relative}"));
        assert!(content.contains("@htaccessError404_0_IT_NVC"));
        assert!(content.contains("rewrite * /IT-NVC/404.php"));
    }

    #[test]
    fn test_multiple_services_have_independent_states() {
        let mut manager = ProcessManager::new();

        let caddy = manager.services.get_mut(&ServiceType::Caddy).unwrap();
        caddy.state = ServiceState::Running;

        let php = manager.services.get_mut(&ServiceType::PhpFpm).unwrap();
        php.state = ServiceState::Starting;

        let mysql = manager.services.get_mut(&ServiceType::MySQL).unwrap();
        mysql.state = ServiceState::Stopped;

        assert_eq!(manager.status(ServiceType::Caddy), ServiceState::Running);
        assert_eq!(manager.status(ServiceType::PhpFpm), ServiceState::Starting);
        assert_eq!(manager.status(ServiceType::MySQL), ServiceState::Stopped);
    }

    #[test]
    fn test_all_services_use_correct_binary_names() {
        let manager = ProcessManager::new();

        for (service_type, process) in &manager.services {
            assert_eq!(process.name, *service_type);
            assert_eq!(process.name.binary_name(), service_type.binary_name());
        }
    }
}

// Integration tests - require actual runtime binaries installed
// Run with: cargo test --lib -- --ignored --test-threads=1
// IMPORTANT: Run with --test-threads=1 to prevent port conflicts
#[cfg(test)]
mod integration_tests {
    use super::*;
    use std::sync::Mutex;

    // Global mutex to ensure tests run serially even if run with multiple threads
    static TEST_MUTEX: Mutex<()> = Mutex::new(());

    /// Check if runtime binaries are available for integration testing
    fn has_runtime_binaries() -> bool {
        if let Ok(paths) = locate_runtime_binaries() {
            paths.caddy.exists() && paths.php_cgi.exists() && paths.mysql.exists()
        } else {
            false
        }
    }

    /// Check if a port is available
    fn is_port_available(port: u16) -> bool {
        use std::net::TcpListener;
        TcpListener::bind(format!("127.0.0.1:{}", port)).is_ok()
    }

    /// Check if all required ports are available
    fn are_ports_available() -> bool {
        is_port_available(8080) && is_port_available(9000) && is_port_available(3306)
    }

    /// Wait for a service to reach a specific state, with timeout
    fn wait_for_state(
        manager: &mut ProcessManager,
        service: ServiceType,
        expected_state: ServiceState,
        timeout_secs: u64,
    ) -> bool {
        let start = std::time::Instant::now();
        let timeout = std::time::Duration::from_secs(timeout_secs);

        while start.elapsed() < timeout {
            let current_state = manager.status(service);
            if current_state == expected_state {
                return true;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
            manager.update_health();
        }
        false
    }

    /// Clean up any running services after test
    fn cleanup_services(manager: &mut ProcessManager) {
        for service in [ServiceType::Caddy, ServiceType::PhpFpm, ServiceType::MySQL] {
            let _ = manager.stop(service);
        }
        // Give processes time to fully exit
        std::thread::sleep(std::time::Duration::from_millis(500));
    }

    /// Read log file contents for debugging
    fn read_log_file(manager: &ProcessManager, service: ServiceType) -> String {
        // First try to get the log file from the service process
        if let Some(process) = manager.services.get(&service) {
            if let Some(ref log_path) = process.log_file {
                return std::fs::read_to_string(log_path)
                    .unwrap_or_else(|e| format!("Could not read log: {}", e));
            }
        }

        // If not available, try to read from the expected location
        if let Some(ref paths) = manager.runtime_paths {
            let log_name = match service {
                ServiceType::Caddy => "caddy.log",
                ServiceType::PhpFpm => "php-fpm.log",
                ServiceType::MySQL => "mysql.log",
                ServiceType::PostgreSQL => "postgresql.log",
            };
            let log_path = paths.logs_dir.join(log_name);
            if log_path.exists() {
                return std::fs::read_to_string(&log_path)
                    .unwrap_or_else(|e| format!("Log exists but could not read: {}", e));
            }
        }

        "No log file available".to_string()
    }

    /// Setup test with proper checks, returns error message if setup fails
    fn setup_test() -> Result<ProcessManager, String> {
        if !has_runtime_binaries() {
            return Err("Runtime binaries not found. Run download_runtime first.".to_string());
        }

        // Kill any lingering processes from previous tests
        kill_lingering_processes();

        // Wait a bit for ports to be released
        std::thread::sleep(std::time::Duration::from_millis(500));

        if !are_ports_available() {
            return Err("Required ports (8080, 9000, 3306) are not available. \
                       Please stop any services using these ports."
                .to_string());
        }

        let mut manager = ProcessManager::new();
        manager.initialize()?;

        Ok(manager)
    }

    /// Kill any lingering service processes from previous test runs
    fn kill_lingering_processes() {
        #[cfg(windows)]
        {
            use std::process::Command;
            let _ = Command::new("taskkill")
                .args(["/F", "/IM", "caddy.exe"])
                .output();
            let _ = Command::new("taskkill")
                .args(["/F", "/IM", "php-cgi.exe"])
                .output();
            let _ = Command::new("taskkill")
                .args(["/F", "/IM", "mysqld.exe"])
                .output();
        }

        #[cfg(unix)]
        {
            use std::process::Command;
            let _ = Command::new("pkill").args(&["-9", "caddy"]).output();
            let _ = Command::new("pkill").args(&["-9", "php-cgi"]).output();
            let _ = Command::new("pkill").args(&["-9", "mysqld"]).output();
        }
    }

    #[test]
    #[ignore]
    fn test_integration_check_binaries_and_ports() {
        // This test checks prerequisites without starting services
        match setup_test() {
            Ok(_) => println!("SUCCESS: All binaries found and ports available"),
            Err(e) => println!("PREREQUISITE FAILED: {}", e),
        }
    }

    #[test]
    #[ignore]
    fn test_integration_initialize_and_directories() {
        let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());

        let mut manager = match setup_test() {
            Ok(m) => m,
            Err(e) => {
                println!("Skipping: {}", e);
                return;
            }
        };

        // Verify directories were created
        assert!(
            manager.runtime_paths.is_some(),
            "Runtime paths should be set"
        );

        if let Some(ref paths) = manager.runtime_paths {
            assert!(paths.config_dir.exists(), "Config directory should exist");
            assert!(paths.logs_dir.exists(), "Logs directory should exist");
            assert!(
                paths.mysql_data_dir.exists(),
                "MySQL data directory should exist"
            );
            assert!(
                paths.projects_dir.exists(),
                "Projects directory should exist"
            );
        }

        cleanup_services(&mut manager);
    }

    #[test]
    #[ignore]
    fn test_integration_start_stop_caddy() {
        let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());

        let mut manager = match setup_test() {
            Ok(m) => m,
            Err(e) => {
                println!("Skipping: {}", e);
                return;
            }
        };

        // Start Caddy
        let result = manager.start(ServiceType::Caddy);
        if let Err(e) = &result {
            let logs = read_log_file(&manager, ServiceType::Caddy);
            panic!("Caddy failed to start: {}\n\nLogs:\n{}", e, logs);
        }

        // Wait for Caddy to be running
        let is_running = wait_for_state(&mut manager, ServiceType::Caddy, ServiceState::Running, 5);
        assert!(is_running, "Caddy should be in Running state");

        // Stop Caddy
        manager.stop(ServiceType::Caddy).expect("Caddy should stop");

        cleanup_services(&mut manager);
    }

    #[test]
    #[ignore]
    fn test_integration_start_stop_php() {
        let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());

        let mut manager = match setup_test() {
            Ok(m) => m,
            Err(e) => {
                println!("Skipping: {}", e);
                return;
            }
        };

        // Start PHP
        let result = manager.start(ServiceType::PhpFpm);
        if let Err(e) = &result {
            let logs = read_log_file(&manager, ServiceType::PhpFpm);
            panic!("PHP failed to start: {}\n\nLogs:\n{}", e, logs);
        }

        // Wait for PHP to be running
        let is_running =
            wait_for_state(&mut manager, ServiceType::PhpFpm, ServiceState::Running, 5);
        assert!(is_running, "PHP should be in Running state");

        // Stop PHP
        manager.stop(ServiceType::PhpFpm).expect("PHP should stop");

        cleanup_services(&mut manager);
    }

    #[test]
    #[ignore]
    fn test_integration_start_stop_mysql() {
        let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());

        let mut manager = match setup_test() {
            Ok(m) => m,
            Err(e) => {
                println!("Skipping: {}", e);
                return;
            }
        };

        // Start MySQL
        let result = manager.start(ServiceType::MySQL);
        if let Err(e) = &result {
            let logs = read_log_file(&manager, ServiceType::MySQL);
            panic!("MySQL failed to start: {}\n\nLogs:\n{}", e, logs);
        }

        // Wait for MySQL to be running (longer timeout)
        let is_running =
            wait_for_state(&mut manager, ServiceType::MySQL, ServiceState::Running, 15);
        assert!(is_running, "MySQL should be in Running state");

        // Stop MySQL
        manager.stop(ServiceType::MySQL).expect("MySQL should stop");

        cleanup_services(&mut manager);
    }

    #[test]
    #[ignore]
    fn test_integration_restart_caddy() {
        let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());

        let mut manager = match setup_test() {
            Ok(m) => m,
            Err(e) => {
                println!("Skipping: {}", e);
                return;
            }
        };

        // Start Caddy
        if let Err(e) = manager.start(ServiceType::Caddy) {
            let logs = read_log_file(&manager, ServiceType::Caddy);
            panic!("Caddy failed to start: {}\n\nLogs:\n{}", e, logs);
        }
        wait_for_state(&mut manager, ServiceType::Caddy, ServiceState::Running, 5);

        // Restart Caddy
        let result = manager.restart(ServiceType::Caddy);
        assert!(result.is_ok(), "Restart should succeed");

        // Should be running again after restart
        let is_running = wait_for_state(&mut manager, ServiceType::Caddy, ServiceState::Running, 5);
        assert!(is_running, "Caddy should be running after restart");

        cleanup_services(&mut manager);
    }

    #[test]
    #[ignore]
    fn test_integration_all_services_concurrent() {
        let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());

        let mut manager = match setup_test() {
            Ok(m) => m,
            Err(e) => {
                println!("Skipping: {}", e);
                return;
            }
        };

        // Start all services
        if let Err(e) = manager.start(ServiceType::Caddy) {
            let logs = read_log_file(&manager, ServiceType::Caddy);
            panic!("Caddy failed to start: {}\n\nLogs:\n{}", e, logs);
        }
        if let Err(e) = manager.start(ServiceType::PhpFpm) {
            let logs = read_log_file(&manager, ServiceType::PhpFpm);
            panic!("PHP failed to start: {}\n\nLogs:\n{}", e, logs);
        }
        if let Err(e) = manager.start(ServiceType::MySQL) {
            let logs = read_log_file(&manager, ServiceType::MySQL);
            panic!("MySQL failed to start: {}\n\nLogs:\n{}", e, logs);
        }

        // Wait for all to be running
        let caddy_running =
            wait_for_state(&mut manager, ServiceType::Caddy, ServiceState::Running, 10);
        let php_running =
            wait_for_state(&mut manager, ServiceType::PhpFpm, ServiceState::Running, 10);
        let mysql_running =
            wait_for_state(&mut manager, ServiceType::MySQL, ServiceState::Running, 20);

        if !caddy_running {
            let logs = read_log_file(&manager, ServiceType::Caddy);
            panic!("Caddy not running. Logs:\n{}", logs);
        }
        if !php_running {
            let logs = read_log_file(&manager, ServiceType::PhpFpm);
            panic!("PHP not running. Logs:\n{}", logs);
        }
        if !mysql_running {
            let logs = read_log_file(&manager, ServiceType::MySQL);
            panic!("MySQL not running. Logs:\n{}", logs);
        }

        // Stop all services
        manager.stop(ServiceType::MySQL).ok();
        manager.stop(ServiceType::PhpFpm).ok();
        manager.stop(ServiceType::Caddy).ok();

        cleanup_services(&mut manager);
    }

    #[test]
    #[ignore]
    fn test_integration_health_check() {
        let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());

        let mut manager = match setup_test() {
            Ok(m) => m,
            Err(e) => {
                println!("Skipping: {}", e);
                return;
            }
        };

        // Start Caddy
        if let Err(e) = manager.start(ServiceType::Caddy) {
            let logs = read_log_file(&manager, ServiceType::Caddy);
            panic!("Caddy failed to start: {}\n\nLogs:\n{}", e, logs);
        }
        wait_for_state(&mut manager, ServiceType::Caddy, ServiceState::Running, 5);

        // Update health should maintain Running state
        manager.update_health();
        assert_eq!(manager.status(ServiceType::Caddy), ServiceState::Running);

        // Kill the process and check health detects it
        if let Some(ref mut child) = manager.services.get_mut(&ServiceType::Caddy).unwrap().child {
            let _ = child.kill();
            let _ = child.wait();
        }

        manager.update_health();

        // Health check should detect process is gone
        let state = manager.status(ServiceType::Caddy);
        assert!(
            state == ServiceState::Error || state == ServiceState::Stopped,
            "State should be Error or Stopped after process dies, got {:?}",
            state
        );

        cleanup_services(&mut manager);
    }

    #[test]
    #[ignore]
    fn test_integration_log_files_created() {
        let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());

        let mut manager = match setup_test() {
            Ok(m) => m,
            Err(e) => {
                println!("Skipping: {}", e);
                return;
            }
        };

        // Start Caddy
        if let Err(e) = manager.start(ServiceType::Caddy) {
            let logs = read_log_file(&manager, ServiceType::Caddy);
            panic!("Caddy failed to start: {}\n\nLogs:\n{}", e, logs);
        }
        wait_for_state(&mut manager, ServiceType::Caddy, ServiceState::Running, 5);

        // Check log file was created
        let caddy_process = manager.services.get(&ServiceType::Caddy).unwrap();
        if let Some(ref log_path) = caddy_process.log_file {
            assert!(log_path.exists(), "Log file should exist at {:?}", log_path);
        } else {
            panic!("Log file path should be set");
        }

        cleanup_services(&mut manager);
    }
}
