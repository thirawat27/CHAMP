//! Port availability and fallback selection helpers.

use super::platform::stop_runtime_processes_by_executable;
use crate::constants::{port_check_timeout, port_release_poll, process_stop_wait_timeout};
use crate::process::ServiceType;
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::thread;

/// How far above the preferred port a fallback is searched for first.
const FALLBACK_PORT_WINDOW: u16 = 100;
/// IANA dynamic range, the last resort when the window above is full.
const EPHEMERAL_PORTS_FIRST: u16 = 49152;

pub(super) fn tcp_port_accepts(port: u16) -> bool {
    TcpStream::connect_timeout(
        &std::net::SocketAddr::from(([127, 0, 0, 1], port)),
        port_check_timeout(),
    )
    .is_ok()
}

/// Intentionally bind-only: used in tight service start/stop loops where the
/// question is strictly "can I bind this port right now?". This is distinct from
/// `config::is_port_available`, which also does a connect probe (correct for
/// user-facing validation but wrong here).
pub(super) fn port_can_bind(port: u16) -> bool {
    TcpListener::bind(("127.0.0.1", port)).is_ok()
}

pub(super) fn select_available_port(
    service_type: ServiceType,
    preferred_port: u16,
    reserved_ports: &[u16],
) -> Result<u16, String> {
    find_available_port_excluding(service_type, preferred_port, reserved_ports).ok_or_else(|| {
        format!(
            "No available fallback port found for {} near {}.",
            service_type.display_name(),
            preferred_port
        )
    })
}

pub(super) fn select_caddy_port(
    preferred_port: u16,
    caddy_executable: &Path,
    reserved_ports: &[u16],
) -> Result<u16, String> {
    if !reserved_ports.contains(&preferred_port) && port_can_bind(preferred_port) {
        return Ok(preferred_port);
    }

    let stopped = stop_runtime_processes_by_executable(caddy_executable, "Caddy");
    if stopped > 0 {
        let _ = wait_for_port_release(preferred_port, process_stop_wait_timeout());
        if !reserved_ports.contains(&preferred_port) && port_can_bind(preferred_port) {
            return Ok(preferred_port);
        }
    }

    select_available_port(ServiceType::Caddy, preferred_port, reserved_ports)
}

pub(super) fn find_available_port_excluding(
    service_type: ServiceType,
    preferred_port: u16,
    reserved_ports: &[u16],
) -> Option<u16> {
    if preferred_port > 0
        && !reserved_ports.contains(&preferred_port)
        && port_can_bind(preferred_port)
    {
        return Some(preferred_port);
    }

    let first_fallback = first_fallback_port(service_type, preferred_port);
    let last_fallback = preferred_port
        .saturating_add(FALLBACK_PORT_WINDOW)
        .max(first_fallback);
    for port in first_fallback..=last_fallback {
        if port > 0 && !reserved_ports.contains(&port) && port_can_bind(port) {
            return Some(port);
        }
    }

    (EPHEMERAL_PORTS_FIRST..=u16::MAX)
        .find(|&port| !reserved_ports.contains(&port) && port_can_bind(port))
}

pub(super) fn first_fallback_port(service_type: ServiceType, preferred_port: u16) -> u16 {
    if service_type == ServiceType::MySQL && preferred_port == crate::config::DEFAULT_PORTS.mysql {
        preferred_port.saturating_add(2)
    } else {
        preferred_port.saturating_add(1)
    }
}

pub(super) fn wait_for_port_release(port: u16, timeout: std::time::Duration) -> bool {
    let start = std::time::Instant::now();
    while start.elapsed() < timeout {
        if port_can_bind(port) {
            return true;
        }
        thread::sleep(port_release_poll());
    }
    false
}
