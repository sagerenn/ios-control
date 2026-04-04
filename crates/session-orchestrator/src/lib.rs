use std::collections::BTreeMap;
use std::path::PathBuf;

use anyhow::{anyhow, Result};
use ios_control_capability_registry::CapabilityRegistry;
use ios_control_contracts::capture::{SourceKind, VideoFrameDescriptor, VideoSource};
use ios_control_contracts::control::{
    ControlCapability, ControlSessionPhase, ControlSetupChecklist, ExecutionPhase, ExecutionSummary,
};
use ios_control_contracts::grounding::{
    GroundingFailure, GroundingPlan, GroundingRequest, TargetInput,
};
use ios_control_contracts::plugin::{PluginDescriptor, PluginHealth};
use ios_control_contracts::session::{DeviceSessionStatus, DeviceSessionSummary, SessionPhase};
use ios_control_device_registry::{DeviceRecord, DeviceRegistry};
use ios_control_plugin_protocol::{HostToPlugin, PluginToHost};
use ios_control_plugin_runtime::RunningPlugin;
use ios_control_telemetry_store::{TelemetryEvent, TelemetryStore};
use plugin_grounding_core::execution_monitor::{ExecutionDecision, ExecutionMonitor};
use plugin_grounding_core::recovery_controller::RecoveryController;

mod session_actor;

#[derive(Debug, Clone)]
pub struct RequestedPlugins {
    pub capture: String,
    pub control: String,
    pub grounding: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PluginPaths {
    pub capture: PathBuf,
    pub control_ble: PathBuf,
    pub control_fallback: PathBuf,
    pub grounding: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct StartSessionRequest {
    pub device_id: String,
    pub device_name: String,
    pub selected_source_id: Option<String>,
    pub plugin_paths: PluginPaths,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionDiagnostics {
    pub control_summary: String,
    pub grounding_summary: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionResult {
    pub applied: bool,
    pub observed_change: bool,
    pub phase: ExecutionPhase,
    pub summary: String,
    pub attempts: u8,
    pub grounding_failure: Option<GroundingFailure>,
    pub failure_reason: Option<String>,
}

pub struct ActiveSessionState {
    pub summary: DeviceSessionSummary,
    pub selected_source_id: Option<String>,
    pub capture_sources: Vec<VideoSource>,
    pub latest_frame: Option<VideoFrameDescriptor>,
    pub control_checklist: ControlSetupChecklist,
    pub diagnostics: SessionDiagnostics,
    pub execution_result: Option<ExecutionResult>,
    capture_plugin: Option<RunningPlugin>,
    control_plugin: Option<RunningPlugin>,
    grounding_plugin: Option<RunningPlugin>,
}

impl std::fmt::Debug for ActiveSessionState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ActiveSessionState")
            .field("summary", &self.summary)
            .field("selected_source_id", &self.selected_source_id)
            .field("capture_sources", &self.capture_sources)
            .field("latest_frame", &self.latest_frame)
            .field("control_checklist", &self.control_checklist)
            .field("diagnostics", &self.diagnostics)
            .field("execution_result", &self.execution_result)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Default)]
pub struct SessionOrchestrator {
    pub capabilities: CapabilityRegistry,
    pub devices: DeviceRegistry,
    pub telemetry: TelemetryStore,
}

#[derive(Debug, Default)]
pub struct SessionSupervisor {
    orchestrator: SessionOrchestrator,
    sessions: BTreeMap<String, DeviceSessionStatus>,
    active: BTreeMap<String, ActiveSessionState>,
}

impl SessionOrchestrator {
    /// Compatibility shim for callers that only need a requested-plugin summary.
    /// This does not spawn plugin processes, perform handshakes, or create a live session.
    #[deprecated(note = "use start_session_with_plugins for a live plugin-backed session")]
    pub async fn start_session(
        &self,
        device_id: &str,
        requested: RequestedPlugins,
    ) -> Result<DeviceSessionSummary> {
        Ok(DeviceSessionSummary {
            device_id: device_id.into(),
            device_name: device_id.into(),
            phase: SessionPhase::Connecting,
            plugin_health: PluginHealth::Unknown,
            capture_plugin: Some(requested.capture),
            control_plugin: Some(requested.control),
            grounding_plugin: requested.grounding,
        })
    }

    pub async fn start_session_with_plugins(
        &mut self,
        request: StartSessionRequest,
    ) -> Result<ActiveSessionState> {
        let session_id = request.device_id.clone();
        let mut staged_capabilities = Vec::new();
        let mut staged_telemetry = Vec::new();

        let mut capture = RunningPlugin::spawn(&request.plugin_paths.capture).await?;
        let capture_descriptor = capture.handshake().await?;
        staged_capabilities.push((capture_descriptor.plugin_id.clone(), true, None));
        staged_telemetry.push(TelemetryEvent {
            session_id: session_id.clone(),
            message: format!("capture plugin ready: {}", capture_descriptor.plugin_id),
        });

        let capture_sources = request_capture_sources(&mut capture, &capture_descriptor).await?;
        let selected_source_id = select_source_id(
            request.selected_source_id,
            &capture_sources,
            &capture_descriptor,
        )?;
        let latest_frame =
            request_capture_frame(&mut capture, &capture_descriptor, &selected_source_id).await?;
        staged_telemetry.push(TelemetryEvent {
            session_id: session_id.clone(),
            message: format!("capture source selected: {selected_source_id}"),
        });

        let mut control = RunningPlugin::spawn(&request.plugin_paths.control_ble).await?;
        let control_descriptor = control.handshake().await?;
        let control_capability = request_control_capability(&mut control).await?;
        staged_capabilities.push((
            control_descriptor.plugin_id.clone(),
            control_capability.supported,
            control_capability.reason.clone(),
        ));
        let (control_phase, control_checklist) = request_control_session(&mut control).await?;
        staged_telemetry.push(TelemetryEvent {
            session_id: session_id.clone(),
            message: format!("control prepared: {control_phase:?}"),
        });

        let (
            grounding_plugin_id,
            grounding_summary,
            grounding_plugin,
            execution_result,
            latest_frame,
        ) = if let Some(path) = request.plugin_paths.grounding.as_ref() {
            let mut grounding = RunningPlugin::spawn(path).await?;
            let grounding_descriptor = grounding.handshake().await?;
            let plan = request_grounding_plan(&mut grounding).await?;
            let (execution_result, latest_frame) = execute_grounding_plan(
                &mut capture,
                &capture_descriptor,
                &mut control,
                &plan,
                &selected_source_id,
                &latest_frame,
            )
            .await?;
            staged_capabilities.push((grounding_descriptor.plugin_id.clone(), true, None));
            staged_telemetry.push(TelemetryEvent {
                session_id: session_id.clone(),
                message: format!("grounding planned: {}", plan.summary),
            });
            staged_telemetry.push(TelemetryEvent {
                session_id: session_id.clone(),
                message: format!("execution result: {}", execution_result.summary),
            });
            (
                Some(grounding_descriptor.plugin_id),
                Some(plan.summary),
                Some(grounding),
                Some(execution_result),
                Some(latest_frame),
            )
        } else {
            (None, None, None, None, Some(latest_frame))
        };

        let mut plugin_health = if control_capability.supported {
            PluginHealth::Healthy
        } else {
            PluginHealth::Degraded
        };
        let control_summary = if control_capability.supported {
            format!("control supported; phase {control_phase:?}")
        } else {
            format!(
                "control unsupported; phase {control_phase:?}: {}",
                control_capability
                    .reason
                    .as_deref()
                    .unwrap_or("no reason provided")
            )
        };
        let summary = DeviceSessionSummary {
            device_id: request.device_id.clone(),
            device_name: request.device_name.clone(),
            phase: SessionPhase::Streaming,
            plugin_health,
            capture_plugin: Some(capture_descriptor.plugin_id.clone()),
            control_plugin: Some(control_descriptor.plugin_id.clone()),
            grounding_plugin: grounding_plugin_id.clone(),
        };

        let device_record = DeviceRecord {
            device_id: request.device_id.clone(),
            device_name: request.device_name.clone(),
            preferred_capture_plugin: capture_descriptor.plugin_id.clone(),
            preferred_control_plugin: control_descriptor.plugin_id.clone(),
            preferred_grounding_plugin: grounding_plugin_id.clone(),
            last_source_id: Some(selected_source_id.clone()),
        };
        staged_telemetry.push(TelemetryEvent {
            session_id,
            message: "session started".into(),
        });

        for (plugin_id, supported, reason) in staged_capabilities {
            self.capabilities.record(plugin_id, supported, reason);
        }
        self.devices.upsert(device_record);
        for event in staged_telemetry {
            self.telemetry.push(event);
        }

        let mut summary = summary;
        if let Some(result) = execution_result.as_ref() {
            if result.phase == ExecutionPhase::Failed {
                summary.phase = SessionPhase::Degraded;
                plugin_health = PluginHealth::Degraded;
                summary.plugin_health = plugin_health;
            }
        }

        Ok(ActiveSessionState {
            summary,
            selected_source_id: Some(selected_source_id),
            capture_sources,
            latest_frame,
            control_checklist,
            diagnostics: SessionDiagnostics {
                control_summary,
                grounding_summary,
            },
            execution_result,
            capture_plugin: Some(capture),
            control_plugin: Some(control),
            grounding_plugin,
        })
    }
}

impl SessionSupervisor {
    pub async fn start_or_replace_session(
        &mut self,
        request: StartSessionRequest,
    ) -> Result<DeviceSessionStatus> {
        let device_id = request.device_id.clone();
        let active = session_actor::start_session_actor(&mut self.orchestrator, request).await?;
        let status = session_actor::status_snapshot(&active)?;

        if let Some(previous) = self.active.remove(&device_id) {
            previous.shutdown().await?;
        }

        self.active.insert(device_id.clone(), active);
        self.sessions.insert(device_id, status.clone());
        Ok(status)
    }

    pub fn session_statuses(&self) -> &BTreeMap<String, DeviceSessionStatus> {
        &self.sessions
    }

    pub fn active_sessions(&self) -> &BTreeMap<String, ActiveSessionState> {
        &self.active
    }

    pub async fn stop_session(&mut self, device_id: &str) -> Result<()> {
        let shutdown_result = if let Some(active) = self.active.remove(device_id) {
            active.shutdown().await
        } else {
            Ok(())
        };
        self.sessions.remove(device_id);
        shutdown_result
    }
}

impl ActiveSessionState {
    pub async fn shutdown(mut self) -> Result<()> {
        let mut first_error = None;

        if let Some(mut grounding) = self.grounding_plugin.take() {
            if let Err(error) = grounding.stop().await {
                first_error = Some(error);
            }
        }
        if let Some(mut control) = self.control_plugin.take() {
            if let Err(error) = control.stop().await {
                if first_error.is_none() {
                    first_error = Some(error);
                }
            }
        }
        if let Some(mut capture) = self.capture_plugin.take() {
            if let Err(error) = capture.stop().await {
                if first_error.is_none() {
                    first_error = Some(error);
                }
            }
        }

        match first_error {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
}

async fn request_capture_sources(
    capture: &mut RunningPlugin,
    descriptor: &PluginDescriptor,
) -> Result<Vec<VideoSource>> {
    match descriptor.plugin_id.as_str() {
        "capture.direct" => Ok(vec![VideoSource {
            source_id: "direct-1".into(),
            display_name: descriptor.display_name.clone(),
            kind: SourceKind::DirectReceiver,
        }]),
        _ => match request_plugin(capture, &HostToPlugin::ListCaptureSources).await? {
            PluginToHost::CaptureSources { sources } => Ok(sources),
            other => Err(anyhow!("unexpected capture sources response: {other:?}")),
        },
    }
}

fn select_source_id(
    requested_source_id: Option<String>,
    capture_sources: &[VideoSource],
    descriptor: &PluginDescriptor,
) -> Result<String> {
    if let Some(source_id) = requested_source_id {
        if capture_sources
            .iter()
            .any(|source| source.source_id == source_id)
        {
            return Ok(source_id);
        }
        return Err(anyhow!(
            "requested capture source `{source_id}` is unavailable for {}",
            descriptor.plugin_id
        ));
    }

    capture_sources
        .first()
        .map(|source| source.source_id.clone())
        .ok_or_else(|| anyhow!("no capture sources available for {}", descriptor.plugin_id))
}

async fn request_capture_frame(
    capture: &mut RunningPlugin,
    descriptor: &PluginDescriptor,
    selected_source_id: &str,
) -> Result<VideoFrameDescriptor> {
    let response = if descriptor.plugin_id == "capture.direct" {
        match request_plugin(
            capture,
            &HostToPlugin::OpenCaptureStream {
                source_id: selected_source_id.into(),
            },
        )
        .await?
        {
            PluginToHost::CaptureStreamOpened { .. } => {}
            other => return Err(anyhow!("unexpected capture stream response: {other:?}")),
        }

        let frame = match request_plugin(capture, &HostToPlugin::ReadCaptureFrame).await? {
            PluginToHost::CaptureFrame { frame } => frame,
            other => return Err(anyhow!("unexpected capture frame response: {other:?}")),
        };

        match request_plugin(capture, &HostToPlugin::CloseCaptureStream).await? {
            PluginToHost::Ack => {}
            other => return Err(anyhow!("unexpected capture close response: {other:?}")),
        }

        return Ok(frame);
    } else {
        request_plugin(
            capture,
            &HostToPlugin::GetCaptureFrame {
                source_id: selected_source_id.into(),
            },
        )
        .await?
    };

    match response {
        PluginToHost::CaptureFrame { frame } => Ok(frame),
        other => Err(anyhow!("unexpected capture frame response: {other:?}")),
    }
}

async fn request_control_capability(control: &mut RunningPlugin) -> Result<ControlCapability> {
    match request_plugin(control, &HostToPlugin::ProbeControl).await? {
        PluginToHost::ControlCapability { capability } => Ok(capability),
        other => Err(anyhow!("unexpected control capability response: {other:?}")),
    }
}

async fn request_control_session(
    control: &mut RunningPlugin,
) -> Result<(ControlSessionPhase, ControlSetupChecklist)> {
    match request_plugin(control, &HostToPlugin::PrepareControl).await? {
        PluginToHost::ControlSession { phase, checklist } => Ok((phase, checklist)),
        other => Err(anyhow!("unexpected control session response: {other:?}")),
    }
}

async fn request_grounding_plan(grounding: &mut RunningPlugin) -> Result<GroundingPlan> {
    match request_plugin(
        grounding,
        &HostToPlugin::PlanGrounding {
            request: GroundingRequest {
                target: TargetInput {
                    semantic_label: Some("Settings".into()),
                    visual_region: Some((20, 20, 120, 44)),
                    confidence: 0.94,
                },
                device_size: (1179, 2556),
                pointer_estimate: (60.0, 40.0),
                uncertainty_radius: 8.0,
                focus_confidence: 0.75,
                keyboard_preferred: false,
            },
        },
    )
    .await?
    {
        PluginToHost::GroundingPlan { plan } => Ok(plan),
        other => Err(anyhow!("unexpected grounding response: {other:?}")),
    }
}

async fn execute_grounding_plan(
    capture: &mut RunningPlugin,
    capture_descriptor: &PluginDescriptor,
    control: &mut RunningPlugin,
    plan: &GroundingPlan,
    selected_source_id: &str,
    latest_frame: &VideoFrameDescriptor,
) -> Result<(ExecutionResult, VideoFrameDescriptor)> {
    if let Some(failure) = plan.failure {
        return Ok((
            ExecutionResult {
                applied: false,
                observed_change: false,
                phase: ExecutionPhase::Failed,
                summary: format!("grounding failed before execution: {}", failure.as_str()),
                attempts: 0,
                grounding_failure: Some(failure),
                failure_reason: Some(failure.as_str().to_string()),
            },
            latest_frame.clone(),
        ));
    }

    let mut recovery = RecoveryController::default();
    let mut attempts = 0u8;
    let mut previous_frame_index = latest_frame.frame_index;

    loop {
        attempts += 1;
        let summary = request_plan_execution(control, plan).await?;
        if matches!(
            summary.phase,
            ExecutionPhase::Pending | ExecutionPhase::Running
        ) {
            let summary_text = format_execution_summary(&summary, false, attempts - 1);
            return Ok((
                ExecutionResult {
                    applied: false,
                    observed_change: false,
                    phase: ExecutionPhase::Failed,
                    summary: summary_text,
                    attempts,
                    grounding_failure: None,
                    failure_reason: Some(summary.failure_reason.clone().unwrap_or_else(|| {
                        "async execution progress is not yet supported in the orchestrator".into()
                    })),
                },
                latest_frame.clone(),
            ));
        }

        if summary.phase == ExecutionPhase::Failed {
            let summary_text = format_execution_summary(&summary, false, attempts - 1);
            return Ok((
                ExecutionResult {
                    applied: false,
                    observed_change: false,
                    phase: ExecutionPhase::Failed,
                    summary: summary_text,
                    attempts,
                    grounding_failure: None,
                    failure_reason: summary.failure_reason.clone(),
                },
                latest_frame.clone(),
            ));
        }

        let frame = request_capture_frame(capture, capture_descriptor, selected_source_id).await?;
        match ExecutionMonitor::evaluate(previous_frame_index, frame.frame_index, &mut recovery) {
            ExecutionDecision::ObservedChange => {
                let summary_text = format_execution_summary(&summary, true, attempts - 1);
                return Ok((
                    ExecutionResult {
                        applied: false,
                        observed_change: true,
                        phase: ExecutionPhase::Succeeded,
                        summary: summary_text,
                        attempts,
                        grounding_failure: None,
                        failure_reason: Some(
                            "frame advanced after execution, but action effect is not yet confirmed"
                                .into(),
                        ),
                    },
                    frame,
                ));
            }
            ExecutionDecision::Retry => {
                previous_frame_index = frame.frame_index;
            }
            ExecutionDecision::Failed(failure) => {
                let summary_text = format_execution_summary(&summary, false, attempts - 1);
                return Ok((
                    ExecutionResult {
                        applied: false,
                        observed_change: false,
                        phase: ExecutionPhase::Failed,
                        summary: summary_text,
                        attempts,
                        grounding_failure: Some(failure),
                        failure_reason: summary.failure_reason.clone(),
                    },
                    frame,
                ));
            }
        }
    }
}

fn format_execution_summary(
    summary: &ExecutionSummary,
    screen_changed: bool,
    attempts: u8,
) -> String {
    let mut text = summary.summary.clone();
    if let Some(reason) = summary.failure_reason.as_deref() {
        text.push_str(&format!("; failure: {reason}"));
    }
    text.push_str(&format!(
        "; screen_changed={screen_changed}; attempts={}",
        attempts + 1
    ));
    text
}

async fn request_plan_execution(
    control: &mut RunningPlugin,
    plan: &GroundingPlan,
) -> Result<ExecutionSummary> {
    match request_plugin(control, &HostToPlugin::ExecutePlan { plan: plan.clone() }).await? {
        PluginToHost::ExecutionSummary { summary } => Ok(summary),
        other => Err(anyhow!("unexpected execution response: {other:?}")),
    }
}

async fn request_plugin(
    plugin: &mut RunningPlugin,
    message: &HostToPlugin,
) -> Result<PluginToHost> {
    plugin.send(message).await?;
    plugin.read().await
}
