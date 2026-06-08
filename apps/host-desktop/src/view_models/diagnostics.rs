use crate::inventory::model::{InventorySnapshot, Sessionability};
use crate::view_models::startup::StartupViewModel;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticsViewModel {
    pub host_error: Option<String>,
    pub control_summary: String,
    pub grounding_summary: String,
    pub startup_probe_runs: u64,
    pub inventory_refreshes: u64,
    pub inventory_rows: u64,
    pub inventory_startable_rows: u64,
    pub inventory_blocked_rows: u64,
    pub session_start_attempts: u64,
    pub session_start_successes: u64,
    pub session_start_failures: u64,
    pub log_lines: Vec<String>,
}

impl DiagnosticsViewModel {
    const MAX_LOG_LINES: usize = 12;

    pub fn record_host_log_line(&mut self, message: impl Into<String>) -> String {
        let message = message.into();
        self.push_log(message.clone());
        message
    }

    pub fn record_startup_view(&mut self, startup: &StartupViewModel) -> String {
        self.startup_probe_runs += 1;
        self.record_host_log_line(format!("startup probe {}", startup.summary))
    }

    pub fn record_inventory_snapshot(&mut self, snapshot: &InventorySnapshot) -> String {
        self.inventory_refreshes += 1;
        self.inventory_rows = snapshot.devices.len() as u64;
        self.inventory_startable_rows = snapshot
            .devices
            .iter()
            .filter(|device| {
                matches!(
                    device.sessionability,
                    Sessionability::StartableWithPreferredPath
                        | Sessionability::StartableWithFallback
                )
            })
            .count() as u64;
        self.inventory_blocked_rows = snapshot
            .devices
            .iter()
            .filter(|device| {
                matches!(
                    device.sessionability,
                    Sessionability::NotStartable | Sessionability::Unknown
                )
            })
            .count() as u64;
        self.record_host_log_line(format!(
            "inventory snapshot total={} startable={} blocked={}",
            self.inventory_rows, self.inventory_startable_rows, self.inventory_blocked_rows
        ))
    }

    pub fn record_session_start_attempt(
        &mut self,
        device_id: &str,
        source_id: Option<&str>,
    ) -> String {
        self.session_start_attempts += 1;
        self.record_host_log_line(format!(
            "session start requested device={} source={}",
            device_id,
            source_id.unwrap_or("auto")
        ))
    }

    pub fn record_session_start_success(
        &mut self,
        device_id: &str,
        source_id: Option<&str>,
    ) -> String {
        self.session_start_successes += 1;
        self.record_host_log_line(format!(
            "session start succeeded device={} source={}",
            device_id,
            source_id.unwrap_or("auto")
        ))
    }

    pub fn record_session_start_failure(&mut self, device_id: Option<&str>, error: &str) -> String {
        self.session_start_failures += 1;
        self.record_host_log_line(format!(
            "session start failed device={} error={error}",
            device_id.unwrap_or("none")
        ))
    }

    pub fn metric_lines(&self) -> Vec<String> {
        vec![
            format!("startup probes | {}", self.startup_probe_runs),
            format!("inventory refreshes | {}", self.inventory_refreshes),
            format!("inventory rows | {}", self.inventory_rows),
            format!(
                "inventory startable rows | {}",
                self.inventory_startable_rows
            ),
            format!("inventory blocked rows | {}", self.inventory_blocked_rows),
            format!("session start attempts | {}", self.session_start_attempts),
            format!("session start successes | {}", self.session_start_successes),
            format!("session start failures | {}", self.session_start_failures),
        ]
    }

    fn push_log(&mut self, message: String) {
        self.log_lines.push(message);
        if self.log_lines.len() > Self::MAX_LOG_LINES {
            let overflow = self.log_lines.len() - Self::MAX_LOG_LINES;
            self.log_lines.drain(0..overflow);
        }
    }
}
