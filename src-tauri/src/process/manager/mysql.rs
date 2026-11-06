//! Starting MySQL and preparing its data directory.

use super::database::{
    prepare_init_staging_dir, promote_init_staging_dir, stop_process_holding_data_dir,
    wait_for_database_init, wait_port_accepting,
};
use super::logs::{format_process_exit_error, open_log_file_with_retry, LogOutput};
use super::platform::{
    configure_no_window, find_process_ids_by_executable, mysql_socket_path, process_exists,
    terminate_process_by_pid,
};
use super::platform::{
    find_processes_best_effort, stop_runtime_processes_by_executable, MYSQL_STOP_SETTLE,
};
use super::ports::{port_can_bind, tcp_port_accepts, wait_for_port_release};
use super::{adopt_existing_process, attach_started_process, ServiceProcess};
use crate::constants::{mysql_init_check_delay, process_stop_wait_timeout, MYSQL_INIT_MAX_RETRIES};
use crate::logs::append_line as append_log_line;
use crate::runtime::locator::RuntimePaths;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;

/// Start MySQL/MariaDB database server
///
/// **IMPORTANT Platform Differences:**
/// - **Linux**: Uses MariaDB 12.x (binary: mariadbd)
/// - **Windows/macOS**: Uses MySQL 8.x (binary: mysqld)
///
/// These are drop-in replacements for each other, but have different
/// initialization requirements and binary names.
pub(super) fn start_mysql(
    service_process: &mut ServiceProcess,
    paths: &RuntimePaths,
) -> Result<(), String> {
    crate::runtime::locator::require_database_binary(&paths.mysql, "MySQL")?;

    // Initialize MySQL data directory if needed
    initialize_mysql_data_dir(paths)?;

    // ตรวจสอบและหยุด MySQL processes ที่ซ้ำซ้อนอัตโนมัติ
    cleanup_duplicate_mysql_processes(paths, service_process.port);

    if let Some(pid) = find_running_mysql_pid(paths, service_process.port) {
        let log_path = paths.logs_dir.join("mysql.log");
        append_log_line(
            &log_path,
            &format!(
                "CHAMP found an existing MySQL process (pid {}) on 127.0.0.1:{} and will reuse it.",
                pid, service_process.port
            ),
        );
        adopt_existing_process(service_process, pid, log_path.clone());
        return Ok(());
    }

    // A CHAMP mysqld left running on another port still owns the data
    // directory; a second mysqld would abort with "ibdata1 must be writable".
    stop_mysql_holding_data_dir(paths)?;

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

        let stopped = stop_runtime_processes_by_executable(&paths.mysql, "MySQL");
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
                Please manually stop any MySQL processes and try again.\n\
                You can change the MySQL port in Settings.",
                service_process.port
            ));
        }
    }

    // Verify MySQL data directory integrity before starting
    verify_mysql_data_integrity(paths)?;

    // `--secure-file-priv` refuses to start when its folder does not exist.
    fs::create_dir_all(mysql_secure_file_dir(paths))
        .map_err(|e| format!("Failed to create MySQL file exchange folder: {}", e))?;

    // Check if we need to create 127.0.0.1 user (first run)
    let user_created_flag = paths.mysql_data_dir.join(".user_127_0_0_1_created");
    let needs_init_file = !user_created_flag.exists();

    let init_file_path = if needs_init_file {
        // Create init file to add root@127.0.0.1 user. The name is unique per start
        // so the cleanup of an earlier start can never delete this one's file.
        remove_stale_mysql_init_files(&paths.logs_dir);
        let init_file = paths.logs_dir.join(format!(
            "{}{}.sql",
            MYSQL_INIT_FILE_PREFIX,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|elapsed| elapsed.as_nanos())
                .unwrap_or(0)
        ));
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
    let log_file = open_log_file_with_retry(&log_path, "MySQL")?;

    // Build MySQL command with optional init file
    let mut cmd = configure_no_window(Command::new(&paths.mysql));
    cmd.args(mysql_server_args(
        paths,
        service_process.port,
        init_file_path.as_deref(),
    ));

    let mut child = cmd.log_output_to(log_file, "MySQL")?.spawn().map_err(|e| {
        let log_content =
            fs::read_to_string(&log_path).unwrap_or_else(|_| String::from("Could not read log"));
        format!(
            "Failed to start MySQL: {}\n\nMySQL log:\n{}",
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
            let mut error_msg =
                format_process_exit_error("MySQL exited immediately", status, Some(&log_path));

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
                let mysql_client = crate::runtime::locator::sibling_binary(&paths.mysql, "mysql");
                std::thread::spawn(move || {
                    for _ in 0..MYSQL_INIT_MAX_RETRIES {
                        if mysql_root_tcp_login_works(&mysql_client, mysql_port) {
                            let _ = fs::write(marker_path, "done");
                            if let Some(init_file) = init_cleanup_path {
                                let _ = fs::remove_file(init_file);
                            }
                            return;
                        }
                        std::thread::sleep(mysql_init_check_delay());
                    }
                    // Not confirmed yet. A slow mysqld may still have to read the
                    // file, and aborts if it is gone; the next start removes it.
                });
            }
            attach_started_process(service_process, child, log_path.clone(), "MySQL")?;
            Ok(())
        }
        Err(e) => {
            if let Some(init_file) = init_file_path {
                let _ = fs::remove_file(&init_file);
            }
            Err(format!("Failed to check MySQL process: {}", e))
        }
    }
}

pub(super) fn find_running_mysql_pid(paths: &RuntimePaths, port: u16) -> Option<u32> {
    let pid = read_mysql_pid_file(&paths.mysql_data_dir)?;
    if !process_exists(pid) {
        return None;
    }

    wait_port_accepting(port).then_some(pid)
}

/// PID from the data directory's pid file, only when it is a live process
/// running CHAMP's own mysqld (a stale pid file may name a reused PID).
pub(super) fn champ_mysql_pid_for_data_dir(paths: &RuntimePaths) -> Option<u32> {
    let pid = read_mysql_pid_file(&paths.mysql_data_dir)?;
    if !process_exists(pid) {
        return None;
    }
    find_process_ids_by_executable(&paths.mysql)
        .ok()?
        .contains(&pid)
        .then_some(pid)
}

pub(super) fn mysql_port_held_by_champ(paths: &RuntimePaths, port: u16) -> bool {
    tcp_port_accepts(port) && champ_mysql_pid_for_data_dir(paths).is_some()
}

pub(super) fn stop_mysql_holding_data_dir(paths: &RuntimePaths) -> Result<(), String> {
    let Some(pid) = champ_mysql_pid_for_data_dir(paths) else {
        return Ok(());
    };
    stop_process_holding_data_dir(pid, "MySQL", &paths.logs_dir.join("mysql.log"))
}

pub(super) fn read_mysql_pid_file(data_dir: &Path) -> Option<u32> {
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

const MYSQL_INIT_FILE_PREFIX: &str = "mysql_init_user_";

/// Init files of earlier starts. Only call this while CHAMP's mysqld is not running.
pub(super) fn remove_stale_mysql_init_files(logs_dir: &Path) {
    let Ok(entries) = fs::read_dir(logs_dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let is_init_file = (name.starts_with(MYSQL_INIT_FILE_PREFIX) && name.ends_with(".sql"))
            || name == "mysql_init_user.sql";
        if is_init_file {
            let _ = fs::remove_file(entry.path());
        }
    }
}

/// The only folder `SELECT ... INTO OUTFILE` and `LOAD_FILE` may touch.
pub(super) fn mysql_secure_file_dir(paths: &RuntimePaths) -> PathBuf {
    paths
        .mysql_data_dir
        .parent()
        .unwrap_or(&paths.mysql_data_dir)
        .join("secure-files")
}

pub(super) fn is_mariadb_binary(server_binary: &Path) -> bool {
    server_binary
        .file_name()
        .is_some_and(|name| name.to_string_lossy().starts_with("mariadb"))
}

pub(super) fn mysql_server_args(
    paths: &RuntimePaths,
    port: u16,
    init_file: Option<&Path>,
) -> Vec<std::ffi::OsString> {
    let data_dir = paths.mysql_data_dir.to_string_lossy();
    let data_dir = data_dir.trim_end_matches(['\\', '/']);

    // `--no-defaults` must come first. Without it mysqld also reads global option
    // files, such as a distro's /etc/mysql/my.cnf with paths it cannot write.
    let mut args: Vec<std::ffi::OsString> = vec![
        "--no-defaults".into(),
        "--datadir".into(),
        data_dir.into(),
        "--port".into(),
        port.to_string().into(),
        "--bind-address=127.0.0.1".into(),
        "--console".into(),
        "--skip-name-resolve".into(),
    ];

    let mut secure_file_priv = std::ffi::OsString::from("--secure-file-priv=");
    secure_file_priv.push(mysql_secure_file_dir(paths));
    args.push(secure_file_priv);

    // `--bind-address` does not cover the X Plugin, which otherwise listens on
    // every interface. MariaDB has no X Plugin and aborts on unknown options.
    if !is_mariadb_binary(&paths.mysql) {
        args.push("--mysqlx=OFF".into());
    }

    if let Some(socket) = mysql_socket_path(paths) {
        let mut socket_arg = std::ffi::OsString::from("--socket=");
        socket_arg.push(socket);
        args.push(socket_arg);
    }

    if let Some(init_file) = init_file {
        args.push("--init-file".into());
        args.push(init_file.into());
    }

    args
}

pub(super) fn mysql_root_tcp_login_works(mysql_client: &Path, port: u16) -> bool {
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
pub(super) fn verify_mysql_data_integrity(paths: &RuntimePaths) -> Result<(), String> {
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
pub(super) fn initialize_mysql_data_dir(paths: &RuntimePaths) -> Result<(), String> {
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
            eprintln!("MySQL data directory already initialized");
            return Ok(());
        }
    }

    // Create the data directory if it doesn't exist
    fs::create_dir_all(&paths.mysql_data_dir)
        .map_err(|e| format!("Failed to create MySQL data directory: {}", e))?;

    #[cfg(target_os = "linux")]
    {
        // Get clean path with forward slashes
        let data_dir_str = paths.mysql_data_dir.to_string_lossy().replace('\\', "/");

        let uses_mysql_server = paths
            .mysql
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| name == "mysqld")
            .unwrap_or(false);

        if uses_mysql_server {
            initialize_mysqld_data_dir(paths, &mysql_dir)?;
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
                .log_output_to(init_log_file, "MariaDB init")?;

            let mut child = cmd
                .spawn()
                .map_err(|e| format!("Failed to start MariaDB initialization: {}", e))?;

            wait_for_database_init(&mut child, &init_log_path, "MariaDB")?;

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
        initialize_mysqld_data_dir(paths, &mysql_dir)?;
    }

    Ok(())
}

pub(super) fn initialize_mysqld_data_dir(
    paths: &RuntimePaths,
    mysql_dir: &Path,
) -> Result<(), String> {
    // Initialize aside and move into place, so a crash or timeout cannot leave a
    // half-built directory that every later start refuses.
    let staging_dir = prepare_init_staging_dir(&paths.mysql_data_dir, "MySQL")?;
    let data_dir_str = staging_dir.to_string_lossy().replace('\\', "/");

    // MySQL 8.0+ and 9.x initialize local data directories with mysqld.
    eprintln!("MySQL: Initializing data directory at: {}", data_dir_str);

    let init_log_path = paths.logs_dir.join("mysql_init.log");
    let init_log_file = fs::File::create(&init_log_path)
        .map_err(|e| format!("Failed to create init log file: {}", e))?;

    let mut child = configure_no_window(Command::new(&paths.mysql))
        .arg("--no-defaults")
        .arg("--initialize-insecure")
        .arg("--datadir")
        .arg(&data_dir_str)
        .arg("--console")
        .log_output_to(init_log_file, "MySQL init")?
        .spawn()
        .map_err(|e| format!("Failed to start MySQL initialization: {}", e))?;

    wait_for_database_init(&mut child, &init_log_path, "MySQL")?;

    promote_init_staging_dir(&staging_dir, &paths.mysql_data_dir)?;

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

/// Cleanup duplicate MySQL processes automatically
/// This prevents the "exit code 1" error caused by multiple MySQL instances
pub(super) fn cleanup_duplicate_mysql_processes(paths: &RuntimePaths, port: u16) {
    let log_path = paths.logs_dir.join("mysql.log");

    // ค้นหา MySQL processes ที่เป็นของ CHAMP เท่านั้น (ตรวจ full path ไม่ใช่แค่ชื่อไฟล์)
    // เพื่อไม่ให้ไปฆ่า MySQL/MariaDB ที่ผู้ใช้ติดตั้งเองบนเครื่อง (R-03)
    let mysql_pids = find_processes_best_effort(&paths.mysql, &log_path);

    if mysql_pids.is_empty() {
        return;
    }

    // ถ้ามีมากกว่า 1 process แสดงว่ามีซ้ำซ้อน
    if mysql_pids.len() > 1 {
        append_log_line(
            &log_path,
            &format!(
                "WARNING: Found {} MySQL processes running. Cleaning up duplicates...",
                mysql_pids.len()
            ),
        );

        // หยุดทุก process
        let mut stopped = 0;
        for pid in &mysql_pids {
            if terminate_process_by_pid(*pid).is_ok() {
                stopped += 1;
                append_log_line(
                    &log_path,
                    &format!("Stopped duplicate MySQL process (pid {})", pid),
                );
            }
        }

        if stopped > 0 {
            append_log_line(
                &log_path,
                &format!("Cleaned up {} duplicate MySQL process(es)", stopped),
            );

            // รอให้ processes หยุดและ port ว่าง
            thread::sleep(MYSQL_STOP_SETTLE);
            let _ = wait_for_port_release(port, process_stop_wait_timeout());
        }
    } else if mysql_pids.len() == 1 {
        // มี 1 process แต่อาจจะไม่ใช่ของเรา หรือ port ไม่ตรง
        let pid = mysql_pids[0];

        // ตรวจสอบว่า process นี้ใช้ port ที่เราต้องการหรือไม่
        if !tcp_port_accepts(port) {
            // Process มีอยู่แต่ไม่ได้ใช้ port ของเรา - อาจจะ crashed หรือ starting
            if !process_exists(pid) {
                append_log_line(
                    &log_path,
                    &format!("Found stale MySQL PID {} (process not running)", pid),
                );
            }
        }
    }
}
