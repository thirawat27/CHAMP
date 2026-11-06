//! System metrics for the status bar.

use crate::constants::SYSTEM_METRICS_MIN_SAMPLE_INTERVAL_MS;
use serde::Serialize;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;
use sysinfo::{CpuRefreshKind, MemoryRefreshKind, Networks, RefreshKind, System};

/// Data transfer object for system metrics (CPU, memory, network)
#[derive(Clone, Debug, Serialize)]
pub struct SystemMetricsDto {
    pub cpu_usage: f32,
    pub memory_used_bytes: u64,
    pub memory_total_bytes: u64,
    pub network_rx_bps: u64,
    pub network_tx_bps: u64,
}

/// Minimum interval between system metrics samples to avoid excessive CPU usage
const SYSTEM_METRICS_MIN_SAMPLE_INTERVAL: std::time::Duration =
    std::time::Duration::from_millis(SYSTEM_METRICS_MIN_SAMPLE_INTERVAL_MS);

/// System metrics monitor with caching to reduce overhead
struct SystemMetricsMonitor {
    system: System,
    networks: Networks,
    last_network_received_bytes: u64,
    last_network_transmitted_bytes: u64,
    last_sample_time: Instant,
    cached_metrics: Option<SystemMetricsDto>,
}

impl SystemMetricsMonitor {
    fn new() -> Self {
        // Only CPU usage and RAM are shown, so skip processes, disks, etc.
        let mut system = System::new_with_specifics(
            RefreshKind::nothing()
                .with_cpu(CpuRefreshKind::nothing().with_cpu_usage())
                .with_memory(MemoryRefreshKind::nothing().with_ram()),
        );
        system.refresh_cpu_usage();
        system.refresh_memory();

        let mut networks = Networks::new_with_refreshed_list();
        networks.refresh(true);
        let (received, transmitted) = aggregate_network_totals(&networks);

        Self {
            system,
            networks,
            last_network_received_bytes: received,
            last_network_transmitted_bytes: transmitted,
            last_sample_time: Instant::now(),
            cached_metrics: None,
        }
    }

    fn collect(&mut self) -> SystemMetricsDto {
        if self.last_sample_time.elapsed() < SYSTEM_METRICS_MIN_SAMPLE_INTERVAL {
            if let Some(metrics) = &self.cached_metrics {
                return metrics.clone();
            }
        }

        self.system.refresh_cpu_usage();
        self.system.refresh_memory();
        self.networks.refresh(true);

        let now = Instant::now();
        let elapsed_seconds = now.duration_since(self.last_sample_time).as_secs_f64();
        let (current_received, current_transmitted) = aggregate_network_totals(&self.networks);

        let received_delta = current_received.saturating_sub(self.last_network_received_bytes);
        let transmitted_delta =
            current_transmitted.saturating_sub(self.last_network_transmitted_bytes);

        let (network_rx_bps, network_tx_bps) = if elapsed_seconds > 0.0 {
            (
                (received_delta as f64 / elapsed_seconds) as u64,
                (transmitted_delta as f64 / elapsed_seconds) as u64,
            )
        } else {
            (0, 0)
        };

        self.last_network_received_bytes = current_received;
        self.last_network_transmitted_bytes = current_transmitted;
        self.last_sample_time = now;

        let metrics = SystemMetricsDto {
            cpu_usage: self.system.global_cpu_usage(),
            memory_used_bytes: self.system.used_memory(),
            memory_total_bytes: self.system.total_memory(),
            network_rx_bps,
            network_tx_bps,
        };
        self.cached_metrics = Some(metrics.clone());
        metrics
    }
}

fn aggregate_network_totals(networks: &Networks) -> (u64, u64) {
    networks
        .iter()
        .fold((0, 0), |(received, transmitted), (_, data)| {
            (
                received.saturating_add(data.total_received()),
                transmitted.saturating_add(data.total_transmitted()),
            )
        })
}

static SYSTEM_METRICS_MONITOR: OnceLock<Mutex<SystemMetricsMonitor>> = OnceLock::new();

/// Current CPU, memory, and network figures, sampled at most once per interval.
pub fn current_metrics() -> Result<SystemMetricsDto, String> {
    let monitor = SYSTEM_METRICS_MONITOR.get_or_init(|| Mutex::new(SystemMetricsMonitor::new()));
    let mut monitor = monitor
        .lock()
        .map_err(|e| format!("Failed to acquire system metrics lock: {}", e))?;

    Ok(monitor.collect())
}
