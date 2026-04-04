use anyhow::{anyhow, Result};
use ios_control_contracts::capture::VideoSource;
use ios_control_contracts::control::ControlSetupChecklist;
use ios_control_contracts::session::{DeviceSessionStatus, DeviceSessionSummary};
use ios_control_session_orchestrator::{
    PluginPaths, SessionDiagnostics, SessionSupervisor, StartSessionRequest,
};

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct HostRuntimeBridge {
    statuses: Vec<DeviceSessionStatus>,
    pending_start_device_id: Option<String>,
}

impl HostRuntimeBridge {
    pub fn replace_statuses(&mut self, statuses: Vec<DeviceSessionStatus>) {
        self.statuses = statuses;
    }

    pub fn statuses(&self) -> &[DeviceSessionStatus] {
        &self.statuses
    }

    pub fn queue_start(&mut self, device_id: String) {
        self.pending_start_device_id = Some(device_id);
    }

    pub fn take_pending_start(&mut self) -> Option<String> {
        self.pending_start_device_id.take()
    }

    pub fn has_pending_start(&self) -> bool {
        self.pending_start_device_id.is_some()
    }
}

#[derive(Debug, Clone)]
pub struct HostRuntimeConfig {
    pub plugin_paths: PluginPaths,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeWorkspaceState {
    pub device_id: String,
    pub summary: DeviceSessionSummary,
    pub capture_sources: Vec<VideoSource>,
    pub selected_source_id: Option<String>,
    pub control_checklist: ControlSetupChecklist,
    pub diagnostics: SessionDiagnostics,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostRuntimeSnapshot {
    pub statuses: Vec<DeviceSessionStatus>,
    pub workspace: RuntimeWorkspaceState,
}

pub struct HostRuntime {
    tokio: tokio::runtime::Runtime,
    supervisor: SessionSupervisor,
    config: HostRuntimeConfig,
}

impl HostRuntime {
    pub fn new(config: HostRuntimeConfig) -> Result<Self> {
        Ok(Self {
            tokio: tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?,
            supervisor: SessionSupervisor::default(),
            config,
        })
    }

    pub fn start_session(
        &mut self,
        device_id: &str,
        device_name: &str,
        selected_source_id: Option<String>,
    ) -> Result<HostRuntimeSnapshot> {
        self.tokio
            .block_on(self.supervisor.start_or_replace_session(StartSessionRequest {
                device_id: device_id.into(),
                device_name: device_name.into(),
                selected_source_id,
                plugin_paths: self.config.plugin_paths.clone(),
            }))?;

        self.snapshot(device_id)
    }

    pub fn snapshot(&self, device_id: &str) -> Result<HostRuntimeSnapshot> {
        let status = self
            .supervisor
            .session_statuses()
            .get(device_id)
            .cloned()
            .ok_or_else(|| anyhow!("missing runtime status for {device_id}"))?;
        let active = self
            .supervisor
            .active_sessions()
            .get(device_id)
            .ok_or_else(|| anyhow!("missing active session for {device_id}"))?;

        Ok(HostRuntimeSnapshot {
            statuses: self
                .supervisor
                .session_statuses()
                .values()
                .cloned()
                .collect(),
            workspace: RuntimeWorkspaceState {
                device_id: device_id.into(),
                summary: status.summary().clone(),
                capture_sources: active.capture_sources.clone(),
                selected_source_id: active.selected_source_id.clone(),
                control_checklist: active.control_checklist.clone(),
                diagnostics: active.diagnostics.clone(),
            },
        })
    }

    pub fn stop_session(&mut self, device_id: &str) -> Result<()> {
        self.tokio.block_on(self.supervisor.stop_session(device_id))
    }
}
