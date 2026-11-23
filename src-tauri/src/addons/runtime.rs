//! Running add-on processes: start, readiness, health, and stop.
//!
//! State lives in one map behind a short-lived lock. Waiting for a port or for
//! an exit never holds the lock, so status polling stays responsive while an
//! add-on starts or stops. Nothing here knows which add-on it runs: the
//! caller resolves a `LaunchPlan` (built-ins from `spec`, custom add-ons from
//! the user's settings).

use super::spec::StopMethod;
use crate::process::ServiceState;
use std::collections::HashMap;
use std::ffi::OsString;
use std::fs::File;
use std::io::Write;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Mutex, MutexGuard, OnceLock, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

const READY_TIMEOUT: Duration = Duration::from_secs(15);
const READY_POLL: Duration = Duration::from_millis(150);
const GRACEFUL_STOP_TIMEOUT: Duration = Duration::from_secs(5);
const KILL_WAIT_TIMEOUT: Duration = Duration::from_secs(3);
const PORT_RELEASE_TIMEOUT: Duration = Duration::from_secs(3);
const CONNECT_TIMEOUT: Duration = Duration::from_millis(250);

#[derive(Default)]
pub(super) struct AddonProcess {
    child: Option<Child>,
    pub state: Option<ServiceState>,
    pub error: Option<String>,
    /// Ports the running process was started on (they can differ from the
    /// settings after a port change, until the next restart).
    pub ports: Vec<u16>,
    /// Display name and stop method of the process last started.
    name: String,
    stop: Option<StopMethod>,
    /// Bumped on every start and stop, so a start that is still waiting for
    /// its port knows a Stop happened in the meantime.
    generation: u64,
}

impl AddonProcess {
    pub fn state(&self) -> ServiceState {
        self.state.clone().unwrap_or(ServiceState::Stopped)
    }
}

/// Keyed by add-on id: a built-in key (`redis`) or a custom add-on's id.
static RUNTIME: OnceLock<Mutex<HashMap<String, AddonProcess>>> = OnceLock::new();

pub(super) fn lock() -> MutexGuard<'static, HashMap<String, AddonProcess>> {
    RUNTIME
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        // A panic elsewhere must not make every later start/stop fail.
        .unwrap_or_else(PoisonError::into_inner)
}

/// Everything a start needs, resolved before the process is spawned.
pub(super) struct LaunchPlan {
    pub name: String,
    pub binary: PathBuf,
    pub args: Vec<OsString>,
    pub data_dir: PathBuf,
    pub log_path: PathBuf,
    pub ports: Vec<u16>,
    pub stop: StopMethod,
    /// The binary is CHAMP's own copy, so a stray process running it is a
    /// leftover from an earlier CHAMP session and may be stopped. A system
    /// install or a custom add-on is never touched: the user may run it for
    /// something else.
    pub owned_binary: bool,
    /// Extra environment variables for the process.
    pub env: Vec<(String, String)>,
}

/// Stop leftovers of CHAMP's own binary that still hold one of the ports,
/// for example after CHAMP crashed on a system without job objects.
fn clear_stale_copies(id: &str, plan: &LaunchPlan) {
    if !plan.owned_binary || plan.ports.iter().all(|port| port_is_free(*port)) {
        return;
    }
    let tracked = lock().get(id).is_some_and(|entry| entry.child.is_some());
    if tracked {
        return;
    }
    if crate::process::manager::stop_runtime_processes_by_executable(&plan.binary, &plan.name) > 0 {
        let deadline = Instant::now() + PORT_RELEASE_TIMEOUT;
        while plan.ports.iter().any(|port| !port_is_free(*port)) && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(100));
        }
    }
}

pub(super) fn build_command(plan: &LaunchPlan) -> Command {
    let mut command = Command::new(&plan.binary);
    command
        .args(&plan.args)
        .envs(plan.env.iter().map(|(key, value)| (key, value)))
        .current_dir(&plan.data_dir)
        .stdin(Stdio::null());
    crate::process::command::hide_console(&mut command);
    command
}

pub(super) fn port_is_free(port: u16) -> bool {
    TcpListener::bind(("127.0.0.1", port)).is_ok()
}

fn port_accepts(port: u16) -> bool {
    TcpStream::connect_timeout(&SocketAddr::from(([127, 0, 0, 1], port)), CONNECT_TIMEOUT).is_ok()
}

/// Start add-on `id` and wait until every port accepts connections.
pub(super) fn start(id: &str, plan: LaunchPlan) -> Result<(), String> {
    clear_stale_copies(id, &plan);
    let generation = {
        let mut runtime = lock();
        let entry = runtime.entry(id.to_string()).or_default();
        refresh_entry(entry);
        if matches!(
            entry.state(),
            ServiceState::Running | ServiceState::Starting
        ) {
            return Ok(());
        }

        if let Some(busy) = plan.ports.iter().copied().find(|port| !port_is_free(*port)) {
            let message = format!(
                "{} cannot start: port {} is already in use by another program. Stop that program or change the port in Add-on services.",
                plan.name, busy
            );
            entry.state = Some(ServiceState::Error);
            entry.error = Some(message.clone());
            return Err(message);
        }

        std::fs::create_dir_all(&plan.data_dir)
            .map_err(|e| format!("Failed to create {} data folder: {}", plan.name, e))?;
        let log = File::create(&plan.log_path)
            .map_err(|e| format!("Failed to create {} log: {}", plan.name, e))?;
        let log_err = log
            .try_clone()
            .map_err(|e| format!("Failed to open {} log: {}", plan.name, e))?;

        let mut command = build_command(&plan);
        command
            .stdout(Stdio::from(log))
            .stderr(Stdio::from(log_err));
        let child = match command.spawn() {
            Ok(child) => child,
            Err(error) => {
                let message = format!("Failed to start {}: {}", plan.name, error);
                entry.state = Some(ServiceState::Error);
                entry.error = Some(message.clone());
                return Err(message);
            }
        };
        crate::process::job::bind_child_to_app_lifetime(&child);
        entry.child = Some(child);
        entry.state = Some(ServiceState::Starting);
        entry.error = None;
        entry.ports = plan.ports.clone();
        entry.name = plan.name.clone();
        entry.stop = Some(plan.stop);
        entry.generation += 1;
        entry.generation
    };

    wait_until_ready(id, generation, &plan)
}

fn wait_until_ready(id: &str, generation: u64, plan: &LaunchPlan) -> Result<(), String> {
    let deadline = Instant::now() + READY_TIMEOUT;
    loop {
        {
            let mut runtime = lock();
            let Some(entry) = runtime.get_mut(id) else {
                return Ok(());
            };
            if entry.generation != generation {
                // Stopped (or restarted) while starting: that caller owns it now.
                return Ok(());
            }
            if let Some(child) = entry.child.as_mut() {
                if let Ok(Some(status)) = child.try_wait() {
                    entry.child = None;
                    let message = exit_message(&plan.name, &status.to_string(), &plan.log_path);
                    entry.state = Some(ServiceState::Error);
                    entry.error = Some(message.clone());
                    return Err(message);
                }
            }
            if plan.ports.iter().all(|port| port_accepts(*port)) {
                entry.state = Some(ServiceState::Running);
                return Ok(());
            }
            if Instant::now() >= deadline {
                // Slow is not failed: leave it running and let health checks decide.
                entry.state = Some(ServiceState::Running);
                return Ok(());
            }
        }
        thread::sleep(READY_POLL);
    }
}

/// The last log lines explain most failed starts (bad flag, missing DLL).
fn exit_message(name: &str, status: &str, log_path: &Path) -> String {
    let tail = crate::logs::read_tail_lines(log_path)
        .map(|lines| {
            let start = lines.len().saturating_sub(5);
            lines[start..].join("\n")
        })
        .unwrap_or_default();
    if tail.trim().is_empty() {
        format!("{name} exited during start ({status}).")
    } else {
        format!("{name} exited during start ({status}). Last log lines:\n{tail}")
    }
}

/// Notice a process that exited on its own.
fn refresh_entry(entry: &mut AddonProcess) {
    let Some(child) = entry.child.as_mut() else {
        if matches!(
            entry.state(),
            ServiceState::Running | ServiceState::Starting
        ) {
            entry.state = Some(ServiceState::Stopped);
        }
        return;
    };
    match child.try_wait() {
        Ok(Some(status)) => {
            entry.child = None;
            entry.state = Some(ServiceState::Error);
            entry.error = Some(format!(
                "{} exited unexpectedly ({}). Check its log for the reason.",
                entry.name, status
            ));
        }
        Ok(None) => {}
        Err(error) => {
            entry.state = Some(ServiceState::Error);
            entry.error = Some(format!("Failed to check {} process: {}", entry.name, error));
        }
    }
}

/// Ask the process to exit cleanly: Redis saves its data on `SHUTDOWN`, and
/// Mailpit closes its database on SIGTERM. A kill would skip both.
fn request_graceful_stop(stop: Option<StopMethod>, port: Option<u16>, pid: u32) -> bool {
    if let (Some(StopMethod::RedisShutdown), Some(port)) = (stop, port) {
        if let Ok(mut stream) =
            TcpStream::connect_timeout(&SocketAddr::from(([127, 0, 0, 1], port)), CONNECT_TIMEOUT)
        {
            let _ = stream.set_write_timeout(Some(CONNECT_TIMEOUT));
            if stream.write_all(b"*1\r\n$8\r\nSHUTDOWN\r\n").is_ok() {
                return true;
            }
        }
    }
    #[cfg(unix)]
    {
        Command::new("kill")
            .args(["-TERM", &pid.to_string()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
        false
    }
}

fn wait_for_exit(child: &mut Child, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return true,
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(100)),
            Ok(None) => return false,
            Err(_) => return false,
        }
    }
}

/// Stop add-on `id`. Returns an error only when the process refuses to exit.
pub(super) fn stop(id: &str) -> Result<(), String> {
    let (child, ports, name, method) = {
        let mut runtime = lock();
        let Some(entry) = runtime.get_mut(id) else {
            return Ok(());
        };
        entry.generation += 1;
        let Some(child) = entry.child.take() else {
            entry.state = Some(ServiceState::Stopped);
            entry.error = None;
            return Ok(());
        };
        entry.state = Some(ServiceState::Stopping);
        (child, entry.ports.clone(), entry.name.clone(), entry.stop)
    };

    let mut child = child;
    let exited = (request_graceful_stop(method, ports.first().copied(), child.id())
        && wait_for_exit(&mut child, GRACEFUL_STOP_TIMEOUT))
        || {
            let _ = child.kill();
            wait_for_exit(&mut child, KILL_WAIT_TIMEOUT)
        };

    let deadline = Instant::now() + PORT_RELEASE_TIMEOUT;
    while exited && ports.iter().any(|port| !port_is_free(*port)) && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(100));
    }

    let mut runtime = lock();
    let entry = runtime.entry(id.to_string()).or_default();
    if exited {
        entry.state = Some(ServiceState::Stopped);
        entry.error = None;
        Ok(())
    } else {
        let message = format!(
            "{} (pid {}) did not stop. End it in Task Manager and try again.",
            name,
            child.id()
        );
        // Keep the handle so a later Stop can try again.
        entry.child = Some(child);
        entry.state = Some(ServiceState::Error);
        entry.error = Some(message.clone());
        Err(message)
    }
}

/// Record an error for an add-on that could not even be launched.
pub(super) fn set_error(id: &str, message: &str) {
    let mut runtime = lock();
    let entry = runtime.entry(id.to_string()).or_default();
    if entry.child.is_none() {
        entry.state = Some(ServiceState::Error);
        entry.error = Some(message.to_string());
    }
}

/// Clear a stale error, for example after the add-on was disabled.
pub(super) fn clear_error(id: &str) {
    let mut runtime = lock();
    if let Some(entry) = runtime.get_mut(id) {
        if entry.child.is_none() {
            entry.state = Some(ServiceState::Stopped);
            entry.error = None;
        }
    }
}

/// Forget a removed custom add-on (it must already be stopped).
pub(super) fn forget(id: &str) {
    let mut runtime = lock();
    if runtime.get(id).is_some_and(|entry| entry.child.is_none()) {
        runtime.remove(id);
    }
}

/// `(state, error, ports)` for add-on `id`.
pub(super) fn snapshot(id: &str) -> (ServiceState, Option<String>, Vec<u16>) {
    let mut runtime = lock();
    let entry = runtime.entry(id.to_string()).or_default();
    refresh_entry(entry);
    (entry.state(), entry.error.clone(), entry.ports.clone())
}
