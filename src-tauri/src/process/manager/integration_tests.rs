//! Integration tests - require actual runtime binaries installed
//! Run with: cargo test --lib -- --ignored --test-threads=1
//! IMPORTANT: Run with --test-threads=1 to prevent port conflicts

use super::platform::stop_runtime_processes_by_executable;
use super::ProcessManager;
use crate::process::{ServiceState, ServiceType};
use crate::runtime::locator::locate_runtime_binaries;

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
///
/// Matched by full path (R-03): killing by name would also take down a MySQL
/// or web server the developer runs for something else.
fn kill_lingering_processes() {
    if let Ok(paths) = locate_runtime_binaries() {
        for executable in [
            &paths.caddy,
            &paths.php_cgi,
            &paths.mysql,
            &paths.postgresql,
        ] {
            stop_runtime_processes_by_executable(executable, "test");
        }
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
    let is_running = wait_for_state(&mut manager, ServiceType::PhpFpm, ServiceState::Running, 5);
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
    let is_running = wait_for_state(&mut manager, ServiceType::MySQL, ServiceState::Running, 15);
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
    let caddy_running = wait_for_state(&mut manager, ServiceType::Caddy, ServiceState::Running, 10);
    let php_running = wait_for_state(&mut manager, ServiceType::PhpFpm, ServiceState::Running, 10);
    let mysql_running = wait_for_state(&mut manager, ServiceType::MySQL, ServiceState::Running, 20);

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
