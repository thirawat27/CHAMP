//! OS process primitives and runtime stale-process cleanup helpers.

use crate::constants::{
    child_exit_poll, database_shutdown_timeout, graceful_stop_timeout, process_probe_delay,
    process_stop_wait_timeout,
};
use crate::logs::append_line as append_log_line;
use crate::process::ServiceType;
use crate::runtime::locator::RuntimePaths;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Mutex, OnceLock, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

/// Hide the console window a spawned process would otherwise open on Windows.
pub(super) fn configure_no_window(mut command: Command) -> Command {
    crate::process::command::hide_console(&mut command);
    command
}

/// How long a force-killed process gets to disappear.
const KILLED_PROCESS_WAIT: Duration = Duration::from_secs(1);
/// Settle time after force-stopping Caddy, so its port is free for the next start.
pub(super) const CADDY_STOP_SETTLE: Duration = Duration::from_millis(500);
/// mysqld needs longer than Caddy to release its files after a force-stop.
pub(super) const MYSQL_STOP_SETTLE: Duration = Duration::from_secs(2);

/// Wait for a child process to exit, polling until `timeout` elapses.
///
/// The stop request is assumed to have already been sent. This polls `try_wait`
/// in short intervals so the caller is not blocked indefinitely on a process
/// that refuses to exit. If the process is still alive when the deadline is
/// reached, it is killed and given a moment to go. Returns whether it exited.
pub(super) fn wait_child_with_timeout(child: &mut Child, timeout: std::time::Duration) -> bool {
    if poll_child_exit(child, timeout) {
        return true;
    }

    let _ = child.kill();
    poll_child_exit(child, KILLED_PROCESS_WAIT)
}

fn poll_child_exit(child: &mut Child, timeout: std::time::Duration) -> bool {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return true,
            Ok(None) => {
                if std::time::Instant::now() >= deadline {
                    return false;
                }
                thread::sleep(child_exit_poll());
            }
            Err(_) => return false,
        }
    }
}

/// Run a helper command, killing it if it outlives `timeout`. True on success.
#[cfg(target_os = "windows")]
fn run_with_timeout(mut command: Command, timeout: std::time::Duration) -> bool {
    let Ok(mut child) = command
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .stdin(Stdio::null())
        .spawn()
    else {
        return false;
    };
    let deadline = std::time::Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return status.success(),
            Ok(None) if std::time::Instant::now() < deadline => {
                thread::sleep(std::time::Duration::from_millis(100));
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return false;
            }
        }
    }
}

/// How long `service` gets to exit on its own after a clean stop request.
pub(super) fn graceful_stop_timeout_for(service: ServiceType) -> std::time::Duration {
    match service {
        ServiceType::MySQL | ServiceType::PostgreSQL => database_shutdown_timeout(),
        ServiceType::Caddy | ServiceType::PhpFpm => graceful_stop_timeout(),
    }
}

/// Ask a service to exit cleanly. True when the request was delivered, so the
/// caller should wait for the exit instead of killing it right away.
///
/// A killed database has to run crash recovery on its next start, loses
/// PostgreSQL's unlogged tables, and leaves stale pid files behind. A killed
/// php-fpm master leaves workers holding its port.
#[cfg(not(target_os = "windows"))]
pub(super) fn request_graceful_stop(
    service: ServiceType,
    _paths: Option<&RuntimePaths>,
    _port: u16,
    pid: u32,
) -> bool {
    // PostgreSQL's SIGTERM ("smart") would wait for every client to disconnect;
    // SIGINT is its fast shutdown.
    let signal = if service == ServiceType::PostgreSQL {
        "-INT"
    } else {
        "-TERM"
    };
    Command::new("kill")
        .args([signal, &pid.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

/// Windows has no signals for console-less processes, so databases are asked
/// through their own admin tools. Other services are simply terminated.
#[cfg(target_os = "windows")]
pub(super) fn request_graceful_stop(
    service: ServiceType,
    paths: Option<&RuntimePaths>,
    port: u16,
    _pid: u32,
) -> bool {
    let Some(paths) = paths else {
        return false;
    };
    let timeout = database_shutdown_timeout();
    let command = match service {
        ServiceType::MySQL => {
            let admin = crate::runtime::locator::sibling_binary(&paths.mysql, "mysqladmin");
            if !admin.exists() {
                return false;
            }
            let mut command = configure_no_window(Command::new(admin));
            command.args([
                "--no-defaults",
                "--protocol=TCP",
                "-h",
                "127.0.0.1",
                "-P",
                &port.to_string(),
                "-u",
                "root",
                "--password=",
                "--connect-timeout=2",
                &format!("--shutdown-timeout={}", timeout.as_secs()),
                "shutdown",
            ]);
            command
        }
        ServiceType::PostgreSQL => {
            let pg_ctl = crate::runtime::locator::sibling_binary(&paths.postgresql, "pg_ctl");
            if !pg_ctl.exists() {
                return false;
            }
            let mut command = configure_no_window(Command::new(pg_ctl));
            command
                .arg("stop")
                .arg("-D")
                .arg(&paths.postgresql_data_dir)
                .args(["-m", "fast", "-w", "-t", &timeout.as_secs().to_string()]);
            command
        }
        ServiceType::Caddy | ServiceType::PhpFpm => return false,
    };
    // A little over the tools' own limit, so they get to report failure themselves.
    run_with_timeout(command, timeout + std::time::Duration::from_secs(2))
}

fn wait_pid_exit(pid: u32, timeout: std::time::Duration) -> bool {
    let deadline = std::time::Instant::now() + timeout;
    while process_exists_uncached(pid) {
        if std::time::Instant::now() >= deadline {
            return false;
        }
        thread::sleep(process_probe_delay());
    }
    true
}

#[cfg(target_os = "windows")]
fn force_kill_pid(pid: u32) {
    let _ = terminate_process_by_pid(pid);
}

#[cfg(not(target_os = "windows"))]
fn force_kill_pid(pid: u32) {
    let _ = Command::new("kill")
        .args(["-KILL", &pid.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

/// Stop a process CHAMP adopted by PID and has no `Child` handle for.
/// Returns whether it is gone.
pub(super) fn stop_adopted_process(
    pid: u32,
    graceful_requested: bool,
    grace: std::time::Duration,
) -> bool {
    if graceful_requested && wait_pid_exit(pid, grace) {
        return true;
    }
    let _ = terminate_process_by_pid(pid);
    if wait_pid_exit(pid, process_stop_wait_timeout()) {
        return true;
    }
    force_kill_pid(pid);
    wait_pid_exit(pid, KILLED_PROCESS_WAIT)
}

/// Unix socket paths are limited to about 100 bytes (`sun_path`).
const MAX_UNIX_SOCKET_PATH_BYTES: usize = 100;

/// Where CHAMP's mysqld keeps its Unix socket, instead of a shared default such
/// as /tmp/mysql.sock that a system MySQL may own. `None` on Windows, and when
/// the data folder path is too long for a socket (TCP still works then).
pub(super) fn mysql_socket_path(paths: &RuntimePaths) -> Option<PathBuf> {
    if cfg!(target_os = "windows") {
        return None;
    }
    let socket = paths.mysql_data_dir.join("mysql.sock");
    (socket.as_os_str().len() <= MAX_UNIX_SOCKET_PATH_BYTES).then_some(socket)
}

/// Stale-process cleanup is best effort: when PowerShell or WMI is unavailable
/// (AppLocker, broken WMI) the service can still start, so log it and go on.
pub(super) fn find_processes_best_effort(executable: &Path, log_path: &Path) -> Vec<u32> {
    find_process_ids_by_executable(executable).unwrap_or_else(|error| {
        append_log_line(log_path, &format!("Skipped stale-process check: {error}"));
        Vec::new()
    })
}

pub(crate) fn stop_runtime_processes_by_executable(
    executable: &Path,
    service_label: &str,
) -> usize {
    let pids = match find_process_ids_by_executable(executable) {
        Ok(pids) => pids,
        Err(error) => {
            eprintln!("Skipped stale {service_label} process check: {error}");
            return 0;
        }
    };
    let mut stopped = 0;
    for pid in pids {
        if terminate_process_by_pid(pid).is_ok() {
            stopped += 1;
        }
    }
    if stopped > 0 {
        thread::sleep(process_probe_delay());
    }
    eprintln!(
        "Stopped {} stale {} process(es) for {}",
        stopped,
        service_label,
        executable.display()
    );
    stopped
}

#[cfg(target_os = "windows")]
pub(super) fn find_process_ids_by_executable(executable: &Path) -> Result<Vec<u32>, String> {
    // Derive the process name (e.g. "caddy.exe") from the path so this function
    // works for any runtime binary, not just Caddy.
    let proc_name = executable
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("caddy.exe")
        .replace('\'', "''");

    // dunce strips the `\\?\` verbatim prefix that std's canonicalize adds on
    // Windows. Win32_Process.ExecutablePath never carries it, so comparing
    // against a verbatim path matched nothing and every stale-process cleanup
    // silently found zero processes.
    let full_path = dunce::canonicalize(executable)
        .unwrap_or_else(|_| executable.to_path_buf())
        .to_string_lossy()
        .replace('\'', "''");

    let script = format!(
        "$target = '{}'; \
         Get-CimInstance Win32_Process -Filter \"Name = '{}'\" | \
         Where-Object {{ $_.ExecutablePath -and ([System.IO.Path]::GetFullPath($_.ExecutablePath) -ieq $target) }} | \
         ForEach-Object {{ $_.ProcessId }}",
        full_path, proc_name
    );

    let mut cmd = configure_no_window(Command::new("powershell"));
    let output = cmd
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .output()
        .map_err(|e| format!("Failed to inspect {} processes: {}", proc_name, e))?;

    if !output.status.success() {
        return Err(format!(
            "Failed to inspect {} processes: {}",
            proc_name,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| line.trim().parse::<u32>().ok())
        .collect())
}

/// Unix implementation: uses `pgrep -f <path>` to find processes whose
/// command line contains the exact executable path, then verifies each
/// candidate by resolving its `/proc/<pid>/exe` symlink (Linux) or via
/// `lsof` (macOS) so we never accidentally kill a system binary with the
/// same name.
#[cfg(not(target_os = "windows"))]
pub(super) fn find_process_ids_by_executable(executable: &Path) -> Result<Vec<u32>, String> {
    // Resolve to a canonical absolute path so the comparison is reliable
    // even if the caller passed a relative path.
    let canonical = executable
        .canonicalize()
        .unwrap_or_else(|_| executable.to_path_buf());
    let canonical_str = canonical.to_string_lossy();

    // `pgrep -f` searches the full command line, giving us candidate PIDs
    // that *mention* our executable path.  We then verify each one below.
    let output = Command::new("pgrep")
        .arg("-f")
        .arg(canonical_str.as_ref())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output();

    let output = match output {
        Ok(o) => o,
        Err(_) => {
            // pgrep is not available (rare, but handle gracefully)
            return Ok(Vec::new());
        }
    };

    // pgrep exits with code 1 when no matches found – that is not an error.
    if !output.status.success() && output.stdout.is_empty() {
        return Ok(Vec::new());
    }

    let candidate_pids: Vec<u32> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| line.trim().parse::<u32>().ok())
        .collect();

    // Secondary verification: make sure the resolved executable of each
    // PID is exactly our binary (guards against false positives from
    // processes that just have our path as an argument).
    let verified: Vec<u32> = candidate_pids
        .into_iter()
        .filter(|&pid| {
            // Skip our own process
            if pid == std::process::id() {
                return false;
            }

            #[cfg(target_os = "linux")]
            {
                // On Linux, /proc/<pid>/exe is a symlink to the real binary.
                let proc_exe = format!("/proc/{}/exe", pid);
                match std::fs::read_link(&proc_exe) {
                    Ok(resolved) => resolved == canonical,
                    Err(_) => false,
                }
            }

            #[cfg(target_os = "macos")]
            {
                // On macOS there is no /proc; ask `lsof` for the process's
                // "txt" (text/executable) entries in its field output format,
                // which unlike the column view is safe to parse.
                let lsof_out = Command::new("lsof")
                    .args(["-a", "-p", &pid.to_string(), "-d", "txt", "-Fn"])
                    .stdout(Stdio::piped())
                    .stderr(Stdio::null())
                    .output();

                match lsof_out {
                    Ok(o) => {
                        lsof_lists_executable(&String::from_utf8_lossy(&o.stdout), &canonical_str)
                    }
                    Err(_) => {
                        // lsof not available: accept the pgrep match as-is
                        true
                    }
                }
            }

            // Fallback for any other Unix-like OS
            #[cfg(not(any(target_os = "linux", target_os = "macos")))]
            {
                true
            }
        })
        .collect();

    Ok(verified)
}

/// `lsof -Fn` prints one field per line; file names start with `n`.
#[cfg(any(target_os = "macos", test))]
pub(super) fn lsof_lists_executable(field_output: &str, executable: &str) -> bool {
    field_output
        .lines()
        .any(|line| line.strip_prefix('n') == Some(executable))
}

// R-11: `update_health` polls process liveness on every frontend refresh
// (~3.5s) whenever CHAMP adopted an existing/external process. On Windows each
// check spawns `tasklist`, so an adopted process spawns a process continuously.
// A short time-based cache collapses bursts of repeated checks for the same PID.
pub(super) const PROCESS_EXISTS_TTL: Duration = Duration::from_millis(1500);

#[allow(clippy::type_complexity)]
pub(super) static PROCESS_EXISTS_CACHE: OnceLock<Mutex<HashMap<u32, (Instant, bool)>>> =
    OnceLock::new();

pub(super) fn process_exists(pid: u32) -> bool {
    let cache = PROCESS_EXISTS_CACHE.get_or_init(|| Mutex::new(HashMap::new()));

    // Return a fresh cached result if one exists and is still within the TTL.
    {
        let guard = cache.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(&(recorded_at, result)) = guard.get(&pid) {
            if recorded_at.elapsed() < PROCESS_EXISTS_TTL {
                return result;
            }
        }
    }

    let result = process_exists_uncached(pid);

    {
        let mut guard = cache.lock().unwrap_or_else(PoisonError::into_inner);
        guard.insert(pid, (Instant::now(), result));
    }

    result
}

#[cfg(target_os = "windows")]
pub(super) fn process_exists_uncached(pid: u32) -> bool {
    let mut cmd = configure_no_window(Command::new("tasklist"));
    cmd.args(["/FI", &format!("PID eq {}", pid), "/FO", "CSV", "/NH"])
        .output()
        .map(|output| {
            let stdout = String::from_utf8_lossy(&output.stdout);
            stdout.contains(&format!("\"{}\"", pid))
        })
        .unwrap_or(false)
}

#[cfg(not(target_os = "windows"))]
pub(super) fn process_exists_uncached(pid: u32) -> bool {
    Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

#[cfg(target_os = "windows")]
pub(super) fn terminate_process_by_pid(pid: u32) -> Result<(), String> {
    let mut cmd = configure_no_window(Command::new("taskkill"));
    cmd.args(["/PID", &pid.to_string(), "/T", "/F"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|e| format!("Failed to stop process pid {}: {}", pid, e))
        .and_then(|status| {
            if status.success() {
                Ok(())
            } else {
                Err(format!("taskkill failed for process pid {}", pid))
            }
        })
}

#[cfg(not(target_os = "windows"))]
pub(super) fn terminate_process_by_pid(pid: u32) -> Result<(), String> {
    Command::new("kill")
        .arg(pid.to_string())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|e| format!("Failed to stop process pid {}: {}", pid, e))
        .and_then(|status| {
            if status.success() {
                Ok(())
            } else {
                Err(format!("kill failed for process pid {}", pid))
            }
        })
}

/// Force-stop every process running `executable` (used when stopping a service),
/// even if the child handle was already lost, for example after a crash. Matching
/// is full-path verified, so a user's own install is never touched. (R-03)
pub(super) fn force_stop_all_processes(
    executable: &Path,
    label: &str,
    settle: Duration,
) -> Result<usize, String> {
    let pids = find_process_ids_by_executable(executable)?;

    if pids.is_empty() {
        return Ok(0);
    }

    eprintln!("Force stopping {} {} process(es)...", pids.len(), label);

    let mut stopped = 0;
    for pid in pids {
        if terminate_process_by_pid(pid).is_ok() {
            stopped += 1;
            eprintln!("Stopped {} process (pid {})", label, pid);
        }
    }

    if stopped > 0 {
        thread::sleep(settle);
    }

    Ok(stopped)
}

pub(super) fn force_stop_all_caddy_processes(caddy_executable: &Path) -> Result<usize, String> {
    force_stop_all_processes(caddy_executable, "Caddy", CADDY_STOP_SETTLE)
}

pub(super) fn force_stop_all_mysql_processes(mysql_executable: &Path) -> Result<usize, String> {
    force_stop_all_processes(mysql_executable, "MySQL", MYSQL_STOP_SETTLE)
}
