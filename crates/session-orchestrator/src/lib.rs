use std::path::PathBuf;

use anyhow::{anyhow, Result};
use ios_control_capability_registry::CapabilityRegistry;
use ios_control_contracts::capture::{SourceKind, VideoFrameDescriptor, VideoSource};
use ios_control_contracts::control::{
    ControlCapability, ControlSessionPhase, ControlSetupChecklist,
};
use ios_control_contracts::grounding::{GroundingPlan, GroundingRequest, TargetInput};
use ios_control_contracts::plugin::{PluginDescriptor, PluginHealth};
use ios_control_contracts::session::{DeviceSessionSummary, SessionPhase};
use ios_control_device_registry::{DeviceRecord, DeviceRegistry};
use ios_control_plugin_protocol::{HostToPlugin, PluginToHost};
use ios_control_plugin_runtime::RunningPlugin;
use ios_control_telemetry_store::{TelemetryEvent, TelemetryStore};

#[derive(Debug, Clone)]
pub struct RequestedPlugins {
    pub capture: String,
    pub control: String,
    pub grounding: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PluginPaths {
    pub capture: PathBuf,
    pub control: PathBuf,
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
pub struct ActiveSessionState {
    pub summary: DeviceSessionSummary,
    pub selected_source_id: Option<String>,
    pub capture_sources: Vec<VideoSource>,
    pub latest_frame: Option<VideoFrameDescriptor>,
    pub control_checklist: ControlSetupChecklist,
    pub diagnostics: SessionDiagnostics,
}

#[derive(Debug, Default)]
pub struct SessionOrchestrator {
    pub capabilities: CapabilityRegistry,
    pub devices: DeviceRegistry,
    pub telemetry: TelemetryStore,
}

impl SessionOrchestrator {
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

        let mut capture = RunningPlugin::spawn(&request.plugin_paths.capture).await?;
        let capture_descriptor = capture.handshake().await?;
        self.capabilities
            .record(capture_descriptor.plugin_id.clone(), true, None);
        self.telemetry.push(TelemetryEvent {
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
        capture.stop().await?;
        self.telemetry.push(TelemetryEvent {
            session_id: session_id.clone(),
            message: format!("capture source selected: {selected_source_id}"),
        });

        let mut control = RunningPlugin::spawn(&request.plugin_paths.control).await?;
        let control_descriptor = control.handshake().await?;
        let control_capability = request_control_capability(&mut control).await?;
        self.capabilities.record(
            control_descriptor.plugin_id.clone(),
            control_capability.supported,
            control_capability.reason.clone(),
        );
        let (control_phase, control_checklist) = request_control_session(&mut control).await?;
        control.stop().await?;
        self.telemetry.push(TelemetryEvent {
            session_id: session_id.clone(),
            message: format!("control prepared: {control_phase:?}"),
        });

        let (grounding_plugin, grounding_summary) =
            if let Some(path) = request.plugin_paths.grounding.as_ref() {
                let mut grounding = RunningPlugin::spawn(path).await?;
                let grounding_descriptor = grounding.handshake().await?;
                let plan = request_grounding_plan(&mut grounding).await?;
                grounding.stop().await?;
                self.capabilities
                    .record(grounding_descriptor.plugin_id.clone(), true, None);
                self.telemetry.push(TelemetryEvent {
                    session_id: session_id.clone(),
                    message: format!("grounding planned: {}", plan.summary),
                });
                (Some(grounding_descriptor.plugin_id), Some(plan.summary))
            } else {
                (None, None)
            };

        let plugin_health = if control_capability.supported {
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
            grounding_plugin: grounding_plugin.clone(),
        };

        self.devices.upsert(DeviceRecord {
            device_id: request.device_id.clone(),
            device_name: request.device_name.clone(),
            preferred_capture_plugin: capture_descriptor.plugin_id.clone(),
            preferred_control_plugin: control_descriptor.plugin_id.clone(),
            preferred_grounding_plugin: grounding_plugin.clone(),
            last_source_id: Some(selected_source_id.clone()),
        });
        self.telemetry.push(TelemetryEvent {
            session_id,
            message: "session started".into(),
        });

        Ok(ActiveSessionState {
            summary,
            selected_source_id: Some(selected_source_id),
            capture_sources,
            latest_frame: Some(latest_frame),
            control_checklist,
            diagnostics: SessionDiagnostics {
                control_summary,
                grounding_summary,
            },
        })
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
    let response = match descriptor.plugin_id.as_str() {
        "capture.direct" => request_plugin(capture, &HostToPlugin::StartDirectCapture).await?,
        _ => {
            request_plugin(
                capture,
                &HostToPlugin::GetCaptureFrame {
                    source_id: selected_source_id.into(),
                },
            )
            .await?
        }
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

async fn request_plugin(
    plugin: &mut RunningPlugin,
    message: &HostToPlugin,
) -> Result<PluginToHost> {
    plugin.send(message).await?;
    plugin.read().await
}
