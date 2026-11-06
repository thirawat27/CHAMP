/// Port detection and allocation
use std::net::{TcpListener, TcpStream};
use std::time::Duration;

/// Check if a port is available (not in use by any service)
///
/// This uses two checks:
/// 1. Try to connect - if successful, something is listening
/// 2. Try to bind - if successful, nothing is using it
///
/// This is the richer "is anyone using this port?" check, intended for
/// user-facing validation (settings, `check_ports`). It is intentionally
/// distinct from `process::manager::port_can_bind`, which is a cheaper
/// bind-only check used in tight service start/stop loops where the question
/// is strictly "can I bind right now?" (a connect check would be wrong there).
pub fn is_port_available(port: u16) -> bool {
    let addr = format!("127.0.0.1:{}", port);

    // First, try to connect - if we can connect, something is listening
    if TcpStream::connect_timeout(&addr.parse().unwrap(), Duration::from_millis(100)).is_ok() {
        return false; // Port is in use
    }

    // If we can't connect, try to bind to make sure we can use it
    TcpListener::bind(&addr).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_port_with_a_listener_is_not_available() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("failed to bind test port");
        let port = listener.local_addr().expect("no local addr").port();

        assert!(!is_port_available(port));

        drop(listener);
        assert!(is_port_available(port));
    }
}
