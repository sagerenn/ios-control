use anyhow::{anyhow, Result};
use std::ffi::OsString;

use ios_control_contracts::capture::{
    CaptureStatus, CaptureStreamDescriptor, VideoFrameDescriptor, VideoSource,
};
use ios_control_contracts::control::{
    ControlInputEvent, ControlSessionPhase, ControlSetupChecklist, ExecutionSummary,
};
use ios_control_contracts::session::{DeviceSessionStatus, DeviceSessionSummary};
use ios_control_session_orchestrator::{
    CaptureBackend, PluginPaths, SessionDiagnostics, SessionSupervisor, StartSessionRequest,
};

use crate::preferences::{
    direct_preview_width_for_height, DEFAULT_DIRECT_PREVIEW_FPS, DEFAULT_DIRECT_PREVIEW_HEIGHT,
    MAX_DIRECT_PREVIEW_FPS, MAX_DIRECT_PREVIEW_HEIGHT, MIN_DIRECT_PREVIEW_FPS,
    MIN_DIRECT_PREVIEW_HEIGHT,
};

#[derive(Debug, Clone)]
pub struct HostRuntimeConfig {
    pub plugin_paths: PluginPaths,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DirectPreviewConfig {
    pub height: u32,
    pub fps: u32,
}

impl DirectPreviewConfig {
    pub fn new(height: u32, fps: u32) -> Self {
        Self {
            height: height.clamp(MIN_DIRECT_PREVIEW_HEIGHT, MAX_DIRECT_PREVIEW_HEIGHT),
            fps: fps.clamp(MIN_DIRECT_PREVIEW_FPS, MAX_DIRECT_PREVIEW_FPS),
        }
    }

    pub fn width(self) -> u32 {
        direct_preview_width_for_height(self.height)
    }
}

impl Default for DirectPreviewConfig {
    fn default() -> Self {
        Self::new(DEFAULT_DIRECT_PREVIEW_HEIGHT, DEFAULT_DIRECT_PREVIEW_FPS)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeWorkspaceState {
    pub device_id: String,
    pub summary: DeviceSessionSummary,
    pub capture_sources: Vec<VideoSource>,
    pub capture_stream: Option<CaptureStreamDescriptor>,
    pub capture_status: Option<CaptureStatus>,
    pub latest_frame: Option<VideoFrameDescriptor>,
    pub selected_source_id: Option<String>,
    pub control_checklist: ControlSetupChecklist,
    pub control_phase: ControlSessionPhase,
    pub execution_observed_change: Option<bool>,
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
    direct_preview: DirectPreviewConfig,
}

impl HostRuntime {
    pub fn new(config: HostRuntimeConfig) -> Result<Self> {
        Ok(Self {
            tokio: tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?,
            supervisor: SessionSupervisor::default(),
            config,
            direct_preview: DirectPreviewConfig::default(),
        })
    }

    pub fn set_direct_preview_config(&mut self, config: DirectPreviewConfig) {
        self.direct_preview = config;
    }

    pub fn start_session(
        &mut self,
        device_id: &str,
        device_name: &str,
        selected_source_id: Option<String>,
        capture_backend: CaptureBackend,
    ) -> Result<HostRuntimeSnapshot> {
        let _env = if capture_backend == CaptureBackend::Direct {
            Some(DirectPreviewEnv::apply(self.direct_preview))
        } else {
            None
        };

        self.tokio.block_on(
            self.supervisor
                .start_or_replace_session(StartSessionRequest {
                    device_id: device_id.into(),
                    device_name: device_name.into(),
                    selected_source_id,
                    capture_backend,
                    plugin_paths: self.config.plugin_paths.clone(),
                }),
        )?;

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
                capture_stream: active.capture_stream.clone(),
                capture_status: active.capture_status.clone(),
                latest_frame: active.latest_frame.clone(),
                selected_source_id: active.selected_source_id.clone(),
                control_checklist: active.control_checklist.clone(),
                control_phase: active.diagnostics.control_phase,
                execution_observed_change: active
                    .execution_result
                    .as_ref()
                    .map(|result| result.observed_change),
                diagnostics: active.diagnostics.clone(),
            },
        })
    }

    pub fn stop_session(&mut self, device_id: &str) -> Result<()> {
        self.tokio.block_on(self.supervisor.stop_session(device_id))
    }

    pub fn refresh_session(&mut self, device_id: &str) -> Result<HostRuntimeSnapshot> {
        self.tokio
            .block_on(self.supervisor.refresh_session(device_id))?;
        self.snapshot(device_id)
    }

    pub fn forward_control_input(
        &mut self,
        device_id: &str,
        event: ControlInputEvent,
    ) -> Result<ExecutionSummary> {
        self.tokio
            .block_on(self.supervisor.forward_control_input(device_id, event))
    }
}

struct EnvOverride {
    key: &'static str,
    original: Option<OsString>,
}

impl EnvOverride {
    fn set(key: &'static str, value: impl ToString) -> Self {
        let original = std::env::var_os(key);
        std::env::set_var(key, value.to_string());
        Self { key, original }
    }
}

impl Drop for EnvOverride {
    fn drop(&mut self) {
        if let Some(value) = self.original.take() {
            std::env::set_var(self.key, value);
        } else {
            std::env::remove_var(self.key);
        }
    }
}

struct DirectPreviewEnv {
    _guards: [EnvOverride; 3],
}

impl DirectPreviewEnv {
    fn apply(config: DirectPreviewConfig) -> Self {
        Self {
            _guards: [
                EnvOverride::set("IOS_CONTROL_DIRECT_PREVIEW_WIDTH", config.width()),
                EnvOverride::set("IOS_CONTROL_DIRECT_PREVIEW_HEIGHT", config.height),
                EnvOverride::set("IOS_CONTROL_DIRECT_PREVIEW_FPS", config.fps),
            ],
        }
    }
}
