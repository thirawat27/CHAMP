//! Starting PostgreSQL and preparing its data directory.

use super::database::{
    prepare_init_staging_dir, promote_init_staging_dir, stop_process_holding_data_dir,
    wait_port_accepting,
};
use super::logs::{format_process_exit_error, open_log_file_with_retry, LogOutput};
use super::platform::stop_runtime_processes_by_executable;
use super::platform::{configure_no_window, find_process_ids_by_executable, process_exists};
use super::ports::{port_can_bind, tcp_port_accepts, wait_for_port_release};
use super::{adopt_existing_process, attach_started_process, ServiceProcess};
use crate::constants::process_stop_wait_timeout;
use crate::logs::append_line as append_log_line;
use crate::runtime::locator::{postgresql_initdb_binary, RuntimePaths};
use std::fs::{self, File};
use std::path::Path;
use std::process::Command;

pub(super) fn start_postgresql(
    service_process: &mut ServiceProcess,
    paths: &RuntimePaths,
) -> Result<(), String> {
    crate::runtime::locator::require_database_binary(&paths.postgresql, "PostgreSQL")?;
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
        adopt_existing_process(service_process, pid, log_path);
        return Ok(());
    }

    // A CHAMP postgres left running on another port still owns the data
    // directory; a second postmaster would exit on its lock file.
    if let Some(pid) = champ_postgresql_pid_for_data_dir(paths) {
        stop_process_holding_data_dir(pid, "PostgreSQL", &paths.logs_dir.join("postgresql.log"))?;
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

        let stopped = stop_runtime_processes_by_executable(&paths.postgresql, "PostgreSQL");
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
        .log_output_to(log_file, "PostgreSQL")?;

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

pub(super) fn initialize_postgresql_data_dir(paths: &RuntimePaths) -> Result<(), String> {
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

    // Initialize aside and move into place, so a crash or timeout cannot leave a
    // half-built directory that later starts mistake for a real one.
    let staging_dir = prepare_init_staging_dir(&paths.postgresql_data_dir, "PostgreSQL")?;

    let init_log_path = paths.logs_dir.join("postgresql_init.log");
    let init_log_file = File::create(&init_log_path)
        .map_err(|e| format!("Failed to create PostgreSQL init log file: {}", e))?;

    let mut cmd = configure_no_window(Command::new(&initdb));
    cmd.arg("-D")
        .arg(&staging_dir)
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
        .log_output_to(init_log_file, "PostgreSQL init")?;

    let status = cmd
        .status()
        .map_err(|e| format!("Failed to initialize PostgreSQL data directory: {}", e))?;
    if status.success() {
        return promote_init_staging_dir(&staging_dir, &paths.postgresql_data_dir);
    }

    Err(format_process_exit_error(
        "PostgreSQL initialization failed",
        status,
        Some(&init_log_path),
    ))
}

pub(super) fn cleanup_stale_postgresql_pid(paths: &RuntimePaths) {
    let pid_file = paths.postgresql_data_dir.join("postmaster.pid");
    let Some(pid) = read_postgresql_pid_file(&pid_file) else {
        return;
    };
    if !process_exists(pid) {
        let _ = fs::remove_file(pid_file);
    }
}

/// PID from `postmaster.pid`, only when it is a live process running CHAMP's own
/// postgres (a stale pid file may name a reused PID).
pub(super) fn champ_postgresql_pid_for_data_dir(paths: &RuntimePaths) -> Option<u32> {
    let pid = read_postgresql_pid_file(&paths.postgresql_data_dir.join("postmaster.pid"))?;
    if !process_exists(pid) {
        return None;
    }
    find_process_ids_by_executable(&paths.postgresql)
        .ok()?
        .contains(&pid)
        .then_some(pid)
}

pub(super) fn postgresql_port_held_by_champ(paths: &RuntimePaths, port: u16) -> bool {
    tcp_port_accepts(port) && champ_postgresql_pid_for_data_dir(paths).is_some()
}

pub(super) fn find_running_postgresql_pid(paths: &RuntimePaths, port: u16) -> Option<u32> {
    let pid = champ_postgresql_pid_for_data_dir(paths)?;

    wait_port_accepting(port).then_some(pid)
}

pub(super) fn read_postgresql_pid_file(pid_file: &Path) -> Option<u32> {
    fs::read_to_string(pid_file)
        .ok()?
        .lines()
        .next()?
        .trim()
        .parse::<u32>()
        .ok()
}
