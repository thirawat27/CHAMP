use super::{ServiceInfo, ServiceMap, ServiceState, ServiceType};
use crate::runtime::locator::{locate_runtime_binaries, RuntimePaths};
use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

// Windows-specific: Constant to hide console window
#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

/// Configure command to hide console window on Windows
#[cfg(target_os = "windows")]
fn configure_no_window(mut command: Command) -> Command {
    use std::os::windows::process::CommandExt;
    command.creation_flags(CREATE_NO_WINDOW);
    command
}

#[cfg(not(target_os = "windows"))]
fn configure_no_window(command: Command) -> Command {
    command
}

/// Open a log file with retry logic for Windows file locking
fn open_log_file_with_retry(log_path: &PathBuf, service_name: &str) -> Result<File, String> {
    let max_retries = 5;
    let retry_delay_ms = 100;

    for attempt in 0..max_retries {
        // Try to open the file, truncating if it exists (for fresh logs)
        // On subsequent retries, try to append in case another process has it open
        let result = if attempt == 0 {
            File::create(log_path)
        } else {
            OpenOptions::new()
                .write(true)
                .create(true)
                .append(true)
                .open(log_path)
        };

        match result {
            Ok(file) => return Ok(file),
            Err(e) => {
                if e.raw_os_error() == Some(32) && attempt < max_retries - 1 {
                    // Windows error 32: file is being used by another process
                    // Wait and retry
                    std::thread::sleep(std::time::Duration::from_millis(retry_delay_ms));
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

/// A running service process with its handle and configuration
pub struct ServiceProcess {
    #[allow(dead_code)]
    pub name: ServiceType,
    pub child: Option<Child>,
    pub state: ServiceState,
    pub port: u16,
    /// Path to the log file for this service
    pub log_file: Option<PathBuf>,
    /// Error message if the service is in error state
    pub error_message: Option<String>,
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

        for service_type in [ServiceType::Caddy, ServiceType::PhpFpm, ServiceType::MySQL] {
            services.insert(
                service_type,
                ServiceProcess {
                    name: service_type,
                    child: None,
                    state: ServiceState::Stopped,
                    port: Self::port_for_service(service_type, &settings),
                    log_file: None,
                    error_message: None,
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
        }
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

        let service_process = self
            .services
            .get_mut(&service)
            .ok_or_else(|| format!("Service {:?} not found", service))?;

        // Check if already running
        if service_process.state.is_running() {
            return Ok(());
        }

        service_process.state = ServiceState::Starting;

        // Spawn the appropriate service
        let result = match service {
            ServiceType::Caddy => start_caddy(
                service_process,
                &paths,
                self.settings.php_port,
                self.settings.mysql_port,
            ),
            ServiceType::PhpFpm => start_php_fpm(service_process, &paths),
            ServiceType::MySQL => start_mysql(service_process, &paths),
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

        // For MySQL/MariaDB on Windows, use taskkill to ensure the process is terminated
        // This is necessary because mysqld may spawn additional processes or the
        // child handle may not properly refer to the actual mysqld.exe process
        #[cfg(target_os = "windows")]
        if service == ServiceType::MySQL {
            let _ = Command::new("taskkill")
                .args(["/F", "/IM", "mysqld.exe"])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .output();

            // Give the process time to terminate
            std::thread::sleep(std::time::Duration::from_millis(500));
        }

        // Terminate the child process if it exists
        if let Some(ref mut child) = service_process.child {
            #[cfg(unix)]
            {
                // On Unix, send SIGTERM
                use std::os::unix::process::CommandExt;
                let _ = child.kill();
            }

            #[cfg(windows)]
            {
                // On Windows (for non-MySQL services), kill the process
                // For MySQL, we already used taskkill above
                if service != ServiceType::MySQL {
                    let _ = child.kill();
                }
            }

            // Wait for the process to exit (with timeout for safety)
            let _ = child.wait();
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
        for service in [ServiceType::PhpFpm, ServiceType::MySQL, ServiceType::Caddy] {
            self.start(service)?;
        }
        Ok(())
    }

    pub fn restart_all(&mut self) -> Result<(), String> {
        self.stop_all()?;
        self.start_all()
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
        for (_service_type, service_process) in self.services.iter_mut() {
            if let Some(ref mut child) = service_process.child {
                match child.try_wait() {
                    Ok(Some(status)) => {
                        // Process has exited
                        service_process.state = ServiceState::Error;
                        service_process.error_message = Some(format!(
                            "Process exited unexpectedly with status: {:?}",
                            status
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
            }
        }
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
) -> Result<(), String> {
    // Kill any existing Caddy processes to avoid port conflicts
    kill_existing_processes("caddy");

    // Prepare Adminer in the writable config directory. This avoids writing into
    // Program Files or any other install directory that may require elevation.
    ensure_adminer(paths, mysql_port)?;

    // Always regenerate Caddyfile with current port settings
    let caddyfile_path = paths.config_dir.join("Caddyfile");
    generate_caddyfile(&caddyfile_path, paths, service_process.port, php_port)?;

    // Open log file with retry logic for Windows file locking
    let log_path = paths.logs_dir.join("caddy.log");
    let log_file = open_log_file_with_retry(&log_path, "Caddy")?;

    // Start Caddy
    let mut child = configure_no_window(Command::new(&paths.caddy))
        .arg("run")
        .arg("--config")
        .arg(&caddyfile_path)
        .current_dir(&paths.config_dir)
        .stdout(Stdio::from(log_file.try_clone().unwrap()))
        .stderr(Stdio::from(log_file))
        .spawn()
        .map_err(|e| format!("Failed to start Caddy: {}", e))?;

    // Give it a moment to start
    std::thread::sleep(std::time::Duration::from_millis(500));

    // Check if process is still running
    match child.try_wait() {
        Ok(Some(status)) => Err(format!(
            "Caddy exited immediately with status: {:?}",
            status
        )),
        Ok(None) => {
            service_process.child = Some(child);
            service_process.log_file = Some(log_path);
            Ok(())
        }
        Err(e) => Err(format!("Failed to check Caddy process: {}", e)),
    }
}

/// Start PHP-FPM (using PHP-CGI for simplicity in MVP)
fn start_php_fpm(service_process: &mut ServiceProcess, paths: &RuntimePaths) -> Result<(), String> {
    // Kill any existing PHP processes to avoid port conflicts
    kill_existing_processes("php-fpm");
    kill_existing_processes("php-cgi");

    // Generate php.ini if it doesn't exist
    if !paths.php_ini.exists() {
        generate_php_ini(&paths.php_ini, paths)?;
    }

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

    let mut child = if is_fpm {
        // Generate php-fpm.conf if it doesn't exist
        let fpm_conf_path = paths.config_dir.join("php-fpm.conf");
        if !fpm_conf_path.exists() {
            generate_php_fpm_conf(&fpm_conf_path, paths, service_process.port)?;
        } else {
            // Regenerate with current port
            generate_php_fpm_conf(&fpm_conf_path, paths, service_process.port)?;
        }

        // PHP-FPM requires -F to run in foreground and -y for config
        configure_no_window(Command::new(&paths.php_cgi))
            .arg("-F") // Don't daemonize
            .arg("-y")
            .arg(&fpm_conf_path)
            .arg("-c")
            .arg(&paths.php_ini)
            .current_dir(&paths.config_dir)
            .stdout(Stdio::from(log_file.try_clone().unwrap()))
            .stderr(Stdio::from(log_file))
            .spawn()
            .map_err(|e| format!("Failed to start PHP-FPM: {}", e))?
    } else {
        // PHP-CGI (Windows) uses -b for FastCGI mode
        configure_no_window(Command::new(&paths.php_cgi))
            .arg("-b")
            .arg(format!("127.0.0.1:{}", service_process.port))
            .arg("-c")
            .arg(&paths.php_ini)
            .current_dir(&paths.config_dir)
            .stdout(Stdio::from(log_file.try_clone().unwrap()))
            .stderr(Stdio::from(log_file))
            .spawn()
            .map_err(|e| format!("Failed to start PHP-CGI: {}", e))?
    };

    // Give it a moment to start
    std::thread::sleep(std::time::Duration::from_millis(500));

    // Check if process is still running
    match child.try_wait() {
        Ok(Some(status)) => Err(format!("PHP exited immediately with status: {:?}", status)),
        Ok(None) => {
            service_process.child = Some(child);
            service_process.log_file = Some(log_path);
            Ok(())
        }
        Err(e) => Err(format!("Failed to check PHP process: {}", e)),
    }
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
    // Kill any existing database server processes to avoid port conflicts
    #[cfg(target_os = "linux")]
    {
        // Linux: Kill MariaDB processes (mariadbd)
        // Also kill mysqld in case of mixed installations
        kill_existing_processes("mariadbd");
        kill_existing_processes("mysqld");
    }

    #[cfg(not(target_os = "linux"))]
    {
        // Windows/macOS: Kill MySQL processes (mysqld)
        kill_existing_processes("mysqld");
    }

    // Initialize MySQL data directory if needed
    initialize_mysql_data_dir(paths)?;

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
        .arg(&data_dir_str)
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
        .stdout(Stdio::from(log_file.try_clone().unwrap()))
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

    // Give MariaDB more time to start (it's slower than other services)
    std::thread::sleep(std::time::Duration::from_secs(3));

    // Check if process is still running
    match child.try_wait() {
        Ok(Some(status)) => {
            // Clean up init file if it exists
            if let Some(init_file) = init_file_path {
                let _ = fs::remove_file(&init_file);
            }
            Err(format!(
                "MariaDB exited immediately with status: {:?}\n\nCheck logs at: {:?}",
                status, log_path
            ))
        }
        Ok(None) => {
            // Mark user as created after successful start
            if needs_init_file {
                let _ = fs::write(&user_created_flag, "done");
                eprintln!("MariaDB root@127.0.0.1 user created during startup");
            }
            service_process.child = Some(child);
            service_process.log_file = Some(log_path);
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

        let install_db_script = mariadbd_dir
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
            .stdout(Stdio::from(init_log_file.try_clone().unwrap()))
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

    #[cfg(not(target_os = "linux"))]
    {
        // ============================================================
        // WINDOWS/MAC: MySQL 8.x Initialization
        // ============================================================
        // MySQL 8.x requires explicit initialization using the
        // --initialize-insecure flag before first use.
        //
        // This creates the system database (mysql schema), sets up
        // privilege tables, and generates initial data files.
        // ============================================================
        eprintln!(
            "MySQL 8.x: Initializing data directory at: {}",
            data_dir_str
        );

        // MySQL 8.0+ uses mysqld --initialize-insecure instead of mysql_install_db
        let mysqld = &paths.mysql;

        let init_log_path = paths.logs_dir.join("mysql_init.log");
        let init_log_file = fs::File::create(&init_log_path)
            .map_err(|e| format!("Failed to create init log file: {}", e))?;

        let mut child = configure_no_window(Command::new(mysqld))
            .arg("--initialize-insecure")
            .arg("--datadir")
            .arg(&data_dir_str)
            .arg("--console")
            .stdout(Stdio::from(init_log_file.try_clone().unwrap()))
            .stderr(Stdio::from(init_log_file))
            .spawn()
            .map_err(|e| format!("Failed to start MySQL initialization: {}", e))?;

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
                        eprintln!("MySQL initialization timeout, killing process");
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
            eprintln!("MySQL initialization failed. Output:\n{}", output);
            return Err(format!(
                "MySQL initialization failed. Check the log file at: {:?}",
                init_log_path
            ));
        }

        eprintln!("MySQL initialization completed successfully");

        // Verify that mysql directory was created
        if !mysql_dir.exists() {
            return Err(format!(
                "MySQL initialization failed - mysql directory not created at {:?}. \
                 Check the log file at: {:?}",
                mysql_dir, init_log_path
            ));
        }
    }

    Ok(())
}

/// Kill any existing processes with the given name to avoid port conflicts
fn kill_existing_processes(process_name: &str) {
    #[cfg(windows)]
    {
        // Use taskkill on Windows to forcefully terminate processes by name
        let _ = Command::new("taskkill")
            .args(["/F", "/IM", &format!("{}.exe", process_name)])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .output();

        // Give the process time to terminate and release ports
        std::thread::sleep(std::time::Duration::from_millis(500));
    }

    #[cfg(unix)]
    {
        // Try multiple approaches to kill processes on Unix
        // 1. First try pkill (most common on Linux)
        let pkill_result = Command::new("pkill")
            .args(["-9", process_name])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .output();

        // If pkill failed (not found), try killall (more common on macOS)
        if pkill_result.is_err() {
            let _ = Command::new("killall")
                .args(["-9", process_name])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .output();
        }

        // Give the process time to terminate and release ports
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
}

/// Generate a basic Caddyfile
fn generate_caddyfile(
    path: &PathBuf,
    paths: &RuntimePaths,
    port: u16,
    php_port: u16,
) -> Result<(), String> {
    // Convert paths to use forward slashes for Caddyfile (cross-platform compatibility)
    let projects = paths
        .projects_dir
        .to_str()
        .ok_or("Invalid project path")?
        .replace('\\', "/");
    let log_file = paths
        .logs_dir
        .join("caddy-access.log")
        .to_str()
        .ok_or("Invalid log path")?
        .replace('\\', "/");
    let adminer = paths
        .adminer
        .to_str()
        .ok_or("Invalid Adminer path")?
        .replace('\\', "/");

    // Build the Caddyfile content
    let mut content = String::new();
    content.push_str(&format!(":{} {{\n", port));
    content.push_str("    # Adminer - must come before project root directives\n");
    content.push_str("    redir /adminer /adminer/\n");
    content.push_str("    redir /phpmyadmin /adminer/\n");
    content.push_str("\n");
    content.push_str("    handle_path /adminer/* {\n");
    content.push_str(&format!("        root * \"{}\"\n", adminer));
    content.push_str(&format!("        php_fastcgi 127.0.0.1:{}\n", php_port));
    content.push_str("        file_server\n");
    content.push_str("    }\n");
    content.push_str("\n");
    content.push_str("    # Root directory for serving files (default project root)\n");
    content.push_str(&format!("    root * \"{}\"\n", projects));
    content.push_str("\n");
    content.push_str("    # Enable PHP for all other requests\n");
    content.push_str(&format!("    php_fastcgi 127.0.0.1:{}\n", php_port));
    content.push_str("\n");
    content.push_str("    # File server for project files\n");
    content.push_str("    file_server browse\n");
    content.push_str("\n");
    content.push_str("    # Logging\n");
    content.push_str("    log {\n");
    content.push_str(&format!("        output file \"{}\"\n", log_file));
    content.push_str("        format json\n");
    content.push_str("    }\n");
    content.push_str("\n");
    content.push_str("    # Encode responses\n");
    content.push_str("    encode gzip\n");
    content.push_str("\n");
    content.push_str("    # Security headers\n");
    content.push_str("    header {\n");
    content.push_str("        X-Content-Type-Options nosniff\n");
    content.push_str("        X-Frame-Options SAMEORIGIN\n");
    content.push_str("        Referrer-Policy no-referrer\n");
    content.push_str("    }\n");
    content.push_str("}\n");

    let mut file = File::create(path).map_err(|e| format!("Failed to create Caddyfile: {}", e))?;
    file.write_all(content.as_bytes())
        .map_err(|e| format!("Failed to write Caddyfile: {}", e))?;

    Ok(())
}

/// Generate a basic php.ini
fn generate_php_ini(path: &PathBuf, paths: &RuntimePaths) -> Result<(), String> {
    // Get the PHP directory (parent of php_cgi binary) to find the ext folder
    let php_dir = paths
        .php_cgi
        .parent()
        .ok_or("Cannot determine PHP directory")?;

    // On Windows, extensions are in the ext/ subdirectory relative to PHP binary
    // Use absolute path with forward slashes (PHP accepts forward slashes on Windows)
    let ext_dir = php_dir.join("ext");
    let ext_dir_str = ext_dir.to_string_lossy().replace('\\', "/");

    // Get error log and session paths
    let error_log = paths
        .logs_dir
        .join("php-errors.log")
        .to_string_lossy()
        .replace('\\', "/");
    let session_path = paths
        .logs_dir
        .join("php-sessions")
        .to_string_lossy()
        .replace('\\', "/");

    let php_ini_content = format!(
        r#"; CHAMP PHP Configuration
; Basic PHP settings for development

[PHP]
; Error reporting - tuned for local development
error_reporting = E_ALL & ~E_DEPRECATED & ~E_WARNING
display_errors = On
display_startup_errors = Off
log_errors = On
error_log = "{}"

; Maximum execution time
max_execution_time = 300
max_input_time = 300

; Memory limit
memory_limit = 256M

; POST data limit
post_max_size = 100M
upload_max_filesize = 100M
max_input_vars = 5000

; Date timezone
date.timezone = UTC

; Extensions - use absolute path for reliability
; Note: zlib and session are built-in to PHP 8.3 and cannot be loaded as extensions
extension_dir = "{}"
extension=curl
extension=mbstring
extension=mysqli
extension=openssl
extension=pdo
extension=pdo_mysql

; Session settings - use absolute path for Windows compatibility
session.save_path = "{}"
session.cookie_httponly = 1
session.use_strict_mode = 1
session.use_cookies = 1
session.use_trans_sid = 0

; File uploads
upload_tmp_dir = "{}"

; CGI settings
cgi.force_redirect = 0
cgi.fix_pathinfo = 1

; Security settings
expose_php = Off

; OPcache settings optimized for local PHP projects
zend_extension=opcache
opcache.enable=1
opcache.memory_consumption=256
opcache.interned_strings_buffer=16
opcache.max_accelerated_files=40000
opcache.revalidate_freq=60
opcache.fast_shutdown=1
opcache.enable_cli=0
opcache.validate_timestamps=1
opcache.save_comments=1
opcache.jit=tracing
opcache.jit_buffer_size=128M

; Realpath cache for better file path resolution (doubled)
realpath_cache_size=8192K
realpath_cache_ttl=300
"#,
        error_log, ext_dir_str, session_path, session_path
    );

    let mut file = File::create(path).map_err(|e| format!("Failed to create php.ini: {}", e))?;
    file.write_all(php_ini_content.as_bytes())
        .map_err(|e| format!("Failed to write php.ini: {}", e))?;

    Ok(())
}

/// Generate php-fpm.conf for static-php builds
fn generate_php_fpm_conf(
    path: &PathBuf,
    paths: &RuntimePaths,
    php_port: u16,
) -> Result<(), String> {
    // Get current username from environment
    let user = std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "nobody".to_string());

    let fpm_conf_content = format!(
        r#"; CHAMP PHP-FPM Configuration
; Optimized for local PHP development

[global]
error_log = {logs_dir}/php-fpm.log
log_level = warning

[www]
user = {user}
group = {user}
listen = 127.0.0.1:{php_port}
listen.owner = {user}
listen.group = {user}
listen.mode = 0660

; Process manager - static for better performance (no spawning delays)
pm = static
pm.max_children = 10

; Worker recycling to prevent memory leaks
pm.max_requests = 1000

; Request settings for local tools and projects
request_terminate_timeout = 300
php_admin_value[error_log] = {logs_dir}/php-fpm.log
php_admin_flag[log_errors] = on
php_value[session.save_path] = {logs_dir}/php-sessions

; Performance tuning
php_value[memory_limit] = 256M
"#,
        logs_dir = paths.logs_dir.display().to_string().replace('\\', "/"),
        user = user,
        php_port = php_port,
    );

    let mut file =
        File::create(path).map_err(|e| format!("Failed to create php-fpm.conf: {}", e))?;
    file.write_all(fpm_conf_content.as_bytes())
        .map_err(|e| format!("Failed to write php-fpm.conf: {}", e))?;

    Ok(())
}

fn ensure_adminer(paths: &RuntimePaths, mysql_port: u16) -> Result<(), String> {
    fs::create_dir_all(&paths.adminer)
        .map_err(|e| format!("Failed to create Adminer directory: {}", e))?;

    let index_path = paths.adminer.join("index.php");
    if let Some(source) = find_adminer_source(paths) {
        fs::copy(&source, &index_path)
            .map_err(|e| format!("Failed to install Adminer from {}: {}", source.display(), e))?;
        return Ok(());
    }

    if index_path.exists() {
        return Ok(());
    }

    let placeholder = format!(
        r#"<?php
http_response_code(503);
header('Content-Type: text/html; charset=utf-8');
?>
<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <title>Adminer is not installed</title>
  <style>
    body {{ font-family: system-ui, -apple-system, Segoe UI, sans-serif; margin: 48px; line-height: 1.5; }}
    code {{ background: #f3f4f6; padding: 2px 6px; border-radius: 4px; }}
  </style>
</head>
<body>
  <h1>Adminer is not installed</h1>
  <p>Run the CHAMP runtime installer to install Adminer. After installation, open <code>/adminer</code> again.</p>
  <p>Default MySQL connection: server <code>127.0.0.1:{}</code>, user <code>root</code>, empty password.</p>
</body>
</html>
"#,
        mysql_port
    );

    fs::write(&index_path, placeholder)
        .map_err(|e| format!("Failed to create Adminer placeholder: {}", e))?;

    Ok(())
}

fn find_adminer_source(paths: &RuntimePaths) -> Option<PathBuf> {
    let mut roots = Vec::new();

    if let Some(base_dir) = paths.config_dir.parent() {
        roots.push(base_dir.join("runtime"));
    }

    if let Some(caddy_dir) = paths.caddy.parent() {
        roots.push(caddy_dir.to_path_buf());
        if let Some(parent) = caddy_dir.parent() {
            roots.push(parent.to_path_buf());
        }
    }

    for root in roots {
        if !root.exists() {
            continue;
        }

        let direct_candidates = [
            root.join("adminer").join("index.php"),
            root.join("adminer.php"),
        ];

        for candidate in direct_candidates {
            if candidate.exists() {
                return Some(candidate);
            }
        }

        if let Ok(entries) = fs::read_dir(&root) {
            for entry in entries.flatten() {
                let path = entry.path();
                let name = path
                    .file_name()
                    .and_then(|value| value.to_str())
                    .unwrap_or_default()
                    .to_ascii_lowercase();

                if path.is_file() && name.starts_with("adminer") && name.ends_with(".php") {
                    return Some(path);
                }
            }
        }
    }

    None
}

impl Default for ProcessManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_manager_new() {
        let manager = ProcessManager::new();

        assert_eq!(manager.services.len(), 3);

        let caddy = manager.services.get(&ServiceType::Caddy).unwrap();
        assert_eq!(caddy.name, ServiceType::Caddy);
        assert_eq!(caddy.state, ServiceState::Stopped);
        assert_eq!(caddy.port, 8080);
        assert!(caddy.child.is_none());
    }

    #[test]
    fn test_process_manager_default() {
        let manager = ProcessManager::default();
        assert_eq!(manager.services.len(), 3);
        assert!(manager.runtime_paths.is_none());
    }

    #[test]
    fn test_status_of_service() {
        let manager = ProcessManager::new();

        assert_eq!(manager.status(ServiceType::Caddy), ServiceState::Stopped);
        assert_eq!(manager.status(ServiceType::PhpFpm), ServiceState::Stopped);
        assert_eq!(manager.status(ServiceType::MySQL), ServiceState::Stopped);
    }

    #[test]
    fn test_get_all_statuses() {
        let manager = ProcessManager::new();
        let statuses = manager.get_all_statuses();

        assert_eq!(statuses.len(), 3);

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
    }

    #[test]
    fn test_port_assignment_for_services() {
        let manager = ProcessManager::new();

        let caddy = manager.services.get(&ServiceType::Caddy).unwrap();
        assert_eq!(caddy.port, 8080);

        let php = manager.services.get(&ServiceType::PhpFpm).unwrap();
        assert_eq!(php.port, 9000);

        let mysql = manager.services.get(&ServiceType::MySQL).unwrap();
        assert_eq!(mysql.port, 3307);
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
        is_port_available(8080) && is_port_available(9000) && is_port_available(3307)
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
            return Err("Required ports (8080, 9000, 3307) are not available. \
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
                .args(&["/F", "/IM", "caddy.exe"])
                .output();
            let _ = Command::new("taskkill")
                .args(&["/F", "/IM", "php-cgi.exe"])
                .output();
            let _ = Command::new("taskkill")
                .args(&["/F", "/IM", "mysqld.exe"])
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
