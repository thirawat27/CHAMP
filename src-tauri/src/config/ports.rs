/// Port detection and allocation
use std::net::{TcpListener, TcpStream};
use std::time::Duration;

/// Find an available port starting from the preferred port
#[allow(dead_code)]
pub fn find_available_port(preferred: u16) -> u16 {
    if is_port_available(preferred) {
        return preferred;
    }

    // Try alternative ports
    for port in (preferred + 1)..65535 {
        if is_port_available(port) {
            return port;
        }
    }

    preferred // Fallback
}

/// Check if a port is available (not in use by any service)
///
/// This uses two checks:
/// 1. Try to connect - if successful, something is listening
/// 2. Try to bind - if successful, nothing is using it
pub fn is_port_available(port: u16) -> bool {
    let addr = format!("127.0.0.1:{}", port);

    // First, try to connect - if we can connect, something is listening
    if TcpStream::connect_timeout(&addr.parse().unwrap(), Duration::from_millis(100)).is_ok() {
        return false; // Port is in use
    }

    // If we can't connect, try to bind to make sure we can use it
    TcpListener::bind(&addr).is_ok()
}

/// Check if a port is in use (something is listening on it)
#[allow(dead_code)]
pub fn is_port_in_use(port: u16) -> bool {
    let addr = format!("127.0.0.1:{}", port);

    TcpStream::connect_timeout(&addr.parse().unwrap(), Duration::from_millis(100)).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_port_available_for_unused_port() {
        // Port 59999 is unlikely to be in use
        // This test just verifies the function doesn't panic
        let _result = is_port_available(59999);
    }

    #[test]
    fn test_find_available_port_returns_preferred_if_available() {
        // Find a port that's definitely available
        let test_port = 59998;
        if is_port_available(test_port) {
            let found = find_available_port(test_port);
            assert_eq!(found, test_port);
        }
    }
}
