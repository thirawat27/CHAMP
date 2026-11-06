use crate::constants::{
    database_port_release_timeout, database_ready_timeout, default_port_release_timeout,
    process_stop_wait_timeout, ready_poll_interval, service_ready_timeout,
};
use crate::logs::append_line as append_log_line;
use crate::process::{ServiceInfo, ServiceMap, ServiceState, ServiceType};
use crate::runtime::locator::{locate_runtime_binaries, RuntimePaths};
use caddy::start_caddy;
use logs::format_process_exit_error;
use mysql::{mysql_port_held_by_champ, start_mysql};
use php::start_php_fpm;
use platform::{
    force_stop_all_caddy_processes, force_stop_all_mysql_processes, graceful_stop_timeout_for,
    process_exists, request_graceful_stop, stop_adopted_process, wait_child_with_timeout,
};
use ports::{select_available_port, select_caddy_port, tcp_port_accepts, wait_for_port_release};
use postgresql::{postgresql_port_held_by_champ, start_postgresql};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Child;
use std::thread;
use std::time::{Duration, Instant};

mod caddy;
mod caddyfile;
mod database;
mod db_tool;
mod htaccess;
mod logs;
mod mysql;
mod php;
mod php_config;
mod platform;
mod ports;
mod postgresql;
mod welcome;

pub(crate) use platform::stop_runtime_processes_by_executable;

const ALL_SERVICES: [ServiceType; 4] = [
    ServiceType::Caddy,
    ServiceType::PhpFpm,
    ServiceType::MySQL,
    ServiceType::PostgreSQL,
];

/// The engine chosen for the database role, independent of the web manager.
fn active_database_service(selection: &crate::runtime::packages::PackageSelection) -> ServiceType {
    if selection.is_postgresql() {
        ServiceType::PostgreSQL
    } else {
        ServiceType::MySQL
    }
}

fn stack_start_services(
    selection: &crate::runtime::packages::PackageSelection,
) -> [ServiceType; 3] {
    [
        ServiceType::PhpFpm,
        active_database_service(selection),
        ServiceType::Caddy,
    ]
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

    /// Record the port a service actually uses. `self.settings` keeps the
    /// configured port, so a temporary conflict does not move the service for
    /// good: every start tries the configured port first.
    fn set_runtime_port(&mut self, service_type: ServiceType, port: u16) {
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

    /// The port a service currently uses.
    fn port_of(&self, service: ServiceType) -> u16 {
        self.services
            .get(&service)
            .map(|service| service.port)
            .unwrap_or_else(|| Self::port_for_service(service, &self.settings))
    }

    pub fn update_ports(&mut self, settings: &crate::config::AppSettings) {
        self.settings = settings.clone();
        for (service_type, service_process) in self.services.iter_mut() {
            // A running service stays on its port until it restarts; stopping it
            // must wait for the port it really holds.
            if !service_process.state.is_running() {
                service_process.port = Self::port_for_service(*service_type, settings);
            }
        }
    }

    /// Follow a package selection saved outside `update_ports`, so the next start
    /// runs the chosen engine and database tool without an app restart.
    pub fn set_package_selection(&mut self, selection: crate::runtime::packages::PackageSelection) {
        self.settings.package_selection = selection;
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

            fs::create_dir_all(&paths.mysql_data_dir)
                .map_err(|e| format!("Failed to create MySQL data dir: {}", e))?;
            fs::create_dir_all(&paths.postgresql_data_dir)
                .map_err(|e| format!("Failed to create PostgreSQL data dir: {}", e))?;
            fs::create_dir_all(&paths.projects_dir)
                .map_err(|e| format!("Failed to create projects dir: {}", e))?;
        }

        Ok(())
    }

    /// Start a service, restarting whatever depends on its port if the port moved.
    pub fn start(&mut self, service: ServiceType) -> Result<(), String> {
        if self.start_service(service)? {
            for dependent in port_dependents(service) {
                self.restart_after_port_change(*dependent);
            }
        }
        Ok(())
    }

    /// A running dependent keeps talking to the old port until it restarts.
    fn restart_after_port_change(&mut self, service: ServiceType) {
        if !self.status(service).is_running() {
            return;
        }
        // Not `restart`: a dependent must not start another round of restarts.
        let result = self
            .stop(service)
            .and_then(|()| self.start_service(service).map(|_| ()));
        if let (Err(error), Some(paths)) = (result, self.runtime_paths.as_ref()) {
            append_log_line(
                &paths.logs_dir.join("port-fallback.log"),
                &format!(
                    "Could not restart {} after a port change: {}",
                    service.display_name(),
                    error
                ),
            );
        }
    }

    /// The port to start `service` on: the configured one when possible.
    fn select_start_port(&self, service: ServiceType, paths: &RuntimePaths) -> Result<u16, String> {
        let configured_port = Self::port_for_service(service, &self.settings);
        let reserved_ports = self.reserved_ports_except(service);
        match service {
            ServiceType::Caddy => select_caddy_port(configured_port, &paths.caddy, &reserved_ports),
            // CHAMP's own database still serving this port is reused by its start
            // function, so it must not count as a conflict.
            ServiceType::MySQL if mysql_port_held_by_champ(paths, configured_port) => {
                Ok(configured_port)
            }
            ServiceType::PostgreSQL if postgresql_port_held_by_champ(paths, configured_port) => {
                Ok(configured_port)
            }
            ServiceType::PhpFpm | ServiceType::MySQL | ServiceType::PostgreSQL => {
                select_available_port(service, configured_port, &reserved_ports)
            }
        }
    }

    fn apply_start_port(&mut self, service: ServiceType, port: u16, logs_dir: &Path) {
        let configured_port = Self::port_for_service(service, &self.settings);
        if port == configured_port {
            self.set_runtime_port(service, port);
        } else {
            self.record_port_fallback(service, configured_port, port, logs_dir);
        }
    }

    /// Returns whether the service moved to a different port than it last used.
    fn start_service(&mut self, service: ServiceType) -> Result<bool, String> {
        let service_snapshot = self
            .services
            .get(&service)
            .ok_or_else(|| format!("Service {:?} not found", service))?;
        if service_snapshot.state.is_running() {
            return Ok(false);
        }
        let previous_port = service_snapshot.port;

        // Always re-resolve: a PHP switch or a runtime installed since the last
        // start changes which binaries the next start must use.
        self.initialize()?;

        // Clone the paths we need before the mutable borrow
        let paths = self
            .runtime_paths
            .as_ref()
            .ok_or("Runtime paths not initialized")?
            .clone();

        let selected_port = self.select_start_port(service, &paths)?;
        self.apply_start_port(service, selected_port, &paths.logs_dir);

        let web_port = self.port_of(ServiceType::Caddy);
        let php_port = self.port_of(ServiceType::PhpFpm);
        let mysql_port = self.port_of(ServiceType::MySQL);
        let postgresql_port = self.port_of(ServiceType::PostgreSQL);
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
                &self.settings.package_selection,
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
        }
        .and_then(|()| wait_until_ready(service_process, ready_timeout_for(service)));

        match result {
            Ok(()) => {
                service_process.state = ServiceState::Running;
                service_process.error_message = None;
                if service == ServiceType::Caddy {
                    crate::tunnel::stop_if_not_forwarding_to(service_process.port);
                }
                Ok(selected_port != previous_port)
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
        let paths = self.runtime_paths.clone();
        let service_process = self
            .services
            .get_mut(&service)
            .ok_or_else(|| format!("Service {:?} not found", service))?;

        // Error/Starting still clean up: a service that reported an error can
        // leave its process (or a stray copy) running, and Stop must clear it.
        if service_process.state == ServiceState::Stopped {
            return Ok(());
        }

        service_process.state = ServiceState::Stopping;
        let port = service_process.port;
        let had_process = service_process.child.is_some() || service_process.external_pid.is_some();

        let target_pid = service_process
            .child
            .as_ref()
            .map(Child::id)
            .or(service_process.external_pid);
        let grace = graceful_stop_timeout_for(service);
        let graceful =
            target_pid.is_some_and(|pid| request_graceful_stop(service, paths.as_ref(), port, pid));

        let mut survivor = None;
        if let Some(child) = service_process.child.as_mut() {
            let exit_timeout = if graceful {
                grace
            } else {
                let _ = child.kill();
                process_stop_wait_timeout()
            };
            if !wait_child_with_timeout(child, exit_timeout) {
                survivor = Some(child.id());
            }
        }
        if let Some(pid) = service_process.external_pid {
            if stop_adopted_process(pid, graceful, grace) {
                service_process.external_pid = None;
            } else {
                survivor = Some(pid);
            }
        }

        if let Some(paths) = paths.as_ref() {
            match service {
                ServiceType::MySQL => {
                    let _ = force_stop_all_mysql_processes(&paths.mysql);
                }
                ServiceType::PostgreSQL => {
                    stop_runtime_processes_by_executable(&paths.postgresql, "PostgreSQL");
                }
                ServiceType::Caddy => {
                    let _ = force_stop_all_caddy_processes(&paths.caddy);
                }
                ServiceType::PhpFpm => {
                    stop_runtime_processes_by_executable(&paths.php_cgi, "PHP");
                }
            }
        }
        let release_timeout = match service {
            ServiceType::MySQL | ServiceType::PostgreSQL => database_port_release_timeout(),
            ServiceType::Caddy | ServiceType::PhpFpm => default_port_release_timeout(),
        };
        let port_released = wait_for_port_release(port, release_timeout);

        // Report what really happened. Saying "Stopped" here would leave the port
        // busy with no explanation, and a repeated Stop would return early.
        if let Some(pid) = survivor {
            let message = format!(
                "{} (pid {}) did not stop. End it manually and try again.",
                service.display_name(),
                pid
            );
            service_process.state = ServiceState::Error;
            service_process.error_message = Some(message.clone());
            return Err(message);
        }

        service_process.child = None;
        if had_process && !port_released {
            let message = format!(
                "{} stopped, but port {} is still in use by another process.",
                service.display_name(),
                port
            );
            service_process.state = ServiceState::Error;
            service_process.error_message = Some(message.clone());
            return Err(message);
        }

        service_process.state = ServiceState::Stopped;
        service_process.error_message = None;

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
        let running_before: Vec<ServiceType> = ALL_SERVICES
            .into_iter()
            .filter(|service| self.status(*service).is_running())
            .collect();
        let moved = self.prepare_stack_ports()?;
        for service in stack_start_services(&self.settings.package_selection) {
            self.start(service)?;
        }

        // Services started above already got the new ports; only one that was
        // running before this call can still be pointing at an old port.
        let mut stale: Vec<ServiceType> = Vec::new();
        for service in moved {
            for dependent in port_dependents(service) {
                if running_before.contains(dependent) && !stale.contains(dependent) {
                    stale.push(*dependent);
                }
            }
        }
        for service in stale {
            self.restart_after_port_change(service);
        }
        Ok(())
    }

    pub fn restart_all(&mut self) -> Result<(), String> {
        self.stop_stack()?;
        self.start_all()
    }

    /// Pick ports for the stack before anything starts, so every service is
    /// configured with its neighbors' final ports. Returns the services that moved.
    fn prepare_stack_ports(&mut self) -> Result<Vec<ServiceType>, String> {
        let paths = self
            .runtime_paths
            .as_ref()
            .ok_or("Runtime paths not initialized")?
            .clone();

        let mut moved = Vec::new();
        for service in stack_start_services(&self.settings.package_selection) {
            let Some(service_process) = self.services.get(&service) else {
                continue;
            };
            if service_process.state.is_running() {
                continue;
            }

            let previous_port = service_process.port;
            let selected_port = self.select_start_port(service, &paths)?;
            self.apply_start_port(service, selected_port, &paths.logs_dir);
            if selected_port != previous_port {
                moved.push(service);
            }
        }

        Ok(moved)
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
        for service in ALL_SERVICES {
            let Some(service_process) = self.services.get(&service) else {
                continue;
            };
            if service_process.state != ServiceState::Stopped {
                let _ = self.stop(service);
            }
        }

        Ok(())
    }

    /// Stop all running services (called on app shutdown)
    pub fn stop_all(&mut self) -> Result<(), String> {
        self.stop_stack()
    }
}

/// Services configured with another service's port when they start: PHP gets
/// the database and web ports through its environment and php.ini, and Caddy
/// gets the PHP port and writes the database port into the database tool config.
fn port_dependents(service: ServiceType) -> &'static [ServiceType] {
    match service {
        ServiceType::MySQL | ServiceType::PostgreSQL => &[ServiceType::PhpFpm, ServiceType::Caddy],
        ServiceType::PhpFpm => &[ServiceType::Caddy],
        ServiceType::Caddy => &[ServiceType::PhpFpm],
    }
}

fn ready_timeout_for(service: ServiceType) -> Duration {
    match service {
        ServiceType::MySQL | ServiceType::PostgreSQL => database_ready_timeout(),
        ServiceType::Caddy | ServiceType::PhpFpm => service_ready_timeout(),
    }
}

/// Wait until a just-spawned service accepts connections, so a bad config or a
/// bind failure is reported by Start itself, and the next caller (an agent's
/// `create_database` right after `start_stack`) finds the service usable.
///
/// A service still loading at the timeout is left running: slow is not failed.
fn wait_until_ready(service_process: &mut ServiceProcess, timeout: Duration) -> Result<(), String> {
    let deadline = Instant::now() + timeout;
    loop {
        let Some(child) = service_process.child.as_mut() else {
            // Adopted process: it was already serving when it was adopted.
            return Ok(());
        };
        match child.try_wait() {
            Ok(Some(status)) => {
                service_process.child = None;
                return Err(format_process_exit_error(
                    &format!(
                        "{} exited during start",
                        service_process.name.display_name()
                    ),
                    status,
                    service_process.log_file.as_deref(),
                ));
            }
            Ok(None) => {}
            Err(error) => {
                return Err(format!(
                    "Failed to check {} process: {}",
                    service_process.name.display_name(),
                    error
                ));
            }
        }

        if tcp_port_accepts(service_process.port) {
            return Ok(());
        }
        if Instant::now() >= deadline {
            eprintln!(
                "{} is still loading after {:?}; leaving it running",
                service_process.name.display_name(),
                timeout
            );
            return Ok(());
        }
        thread::sleep(ready_poll_interval());
    }
}

fn adopt_existing_process(service_process: &mut ServiceProcess, pid: u32, log_path: PathBuf) {
    crate::process::job::bind_pid_to_app_lifetime(pid);
    service_process.child = None;
    service_process.external_pid = Some(pid);
    service_process.log_file = Some(log_path);
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
            crate::process::job::bind_child_to_app_lifetime(&child);
            service_process.child = Some(child);
            service_process.log_file = Some(log_path);
            Ok(())
        }
        Err(e) => Err(format!("Failed to check {} process: {}", service_label, e)),
    }
}

impl Default for ProcessManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod integration_tests;
#[cfg(test)]
mod tests;
