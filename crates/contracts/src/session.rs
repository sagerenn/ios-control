use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::capture::CaptureStreamPhase;
use crate::control::ExecutionPhase;
use crate::plugin::PluginHealth;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionPhase {
    Disconnected,
    Connecting,
    Streaming,
    Degraded,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceSessionSummary {
    pub device_id: String,
    pub device_name: String,
    pub phase: SessionPhase,
    pub plugin_health: PluginHealth,
    pub capture_plugin: Option<String>,
    pub control_plugin: Option<String>,
    pub grounding_plugin: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LiveSessionStatus {
    pub capture_phase: CaptureStreamPhase,
    pub execution_phase: Option<ExecutionPhase>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionSubstate {
    Discovering,
    StartingCapture,
    Streaming,
    StartingControl,
    ControlReady,
    DegradedCapture,
    DegradedControl,
    Recovering,
    OperatorActionRequired,
    Stopped,
}

impl SessionSubstate {
    pub fn summary_phase(self) -> SessionPhase {
        match self {
            Self::Discovering
            | Self::StartingCapture
            | Self::StartingControl
            | Self::Recovering => SessionPhase::Connecting,
            Self::Streaming | Self::ControlReady => SessionPhase::Streaming,
            Self::DegradedCapture | Self::DegradedControl | Self::OperatorActionRequired => {
                SessionPhase::Degraded
            }
            Self::Stopped => SessionPhase::Disconnected,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackendSelection {
    pub capture_backend: String,
    pub control_backend: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceSessionStatus {
    summary: DeviceSessionSummary,
    substate: SessionSubstate,
    backends: BackendSelection,
    operator_action: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct DeviceSessionStatusWire {
    summary: DeviceSessionSummary,
    substate: SessionSubstate,
    backends: BackendSelection,
    operator_action: Option<String>,
}

impl DeviceSessionStatus {
    pub fn new(
        summary: DeviceSessionSummary,
        substate: SessionSubstate,
        backends: BackendSelection,
        operator_action: Option<String>,
    ) -> Result<Self, String> {
        let expected_phase = substate.summary_phase();
        if summary.phase != expected_phase {
            return Err(format!(
                "session summary phase {:?} does not match substate {:?}",
                summary.phase, substate
            ));
        }

        match (&substate, operator_action.as_deref()) {
            (SessionSubstate::OperatorActionRequired, None) => {
                return Err(
                    "operator action required state must include an operator action message".into(),
                )
            }
            (SessionSubstate::OperatorActionRequired, Some(_)) => {}
            (_, Some(_)) => {
                return Err(
                    "operator action message is only valid for operator action required state"
                        .into(),
                )
            }
            (_, None) => {}
        }

        Ok(Self {
            summary,
            substate,
            backends,
            operator_action,
        })
    }

    pub fn summary(&self) -> &DeviceSessionSummary {
        &self.summary
    }

    pub fn substate(&self) -> SessionSubstate {
        self.substate
    }

    pub fn backends(&self) -> &BackendSelection {
        &self.backends
    }

    pub fn operator_action(&self) -> Option<&str> {
        self.operator_action.as_deref()
    }
}

impl From<&DeviceSessionStatus> for DeviceSessionStatusWire {
    fn from(value: &DeviceSessionStatus) -> Self {
        Self {
            summary: value.summary.clone(),
            substate: value.substate,
            backends: value.backends.clone(),
            operator_action: value.operator_action.clone(),
        }
    }
}

impl TryFrom<DeviceSessionStatusWire> for DeviceSessionStatus {
    type Error = String;

    fn try_from(value: DeviceSessionStatusWire) -> Result<Self, Self::Error> {
        Self::new(
            value.summary,
            value.substate,
            value.backends,
            value.operator_action,
        )
    }
}

impl Serialize for DeviceSessionStatus {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        DeviceSessionStatusWire::from(self).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for DeviceSessionStatus {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = DeviceSessionStatusWire::deserialize(deserializer)?;
        Self::try_from(wire).map_err(serde::de::Error::custom)
    }
}

impl DeviceSessionSummary {
    pub fn new(device_id: String, device_name: String) -> Self {
        Self {
            device_id,
            device_name,
            phase: SessionPhase::Disconnected,
            plugin_health: PluginHealth::Unknown,
            capture_plugin: None,
            control_plugin: None,
            grounding_plugin: None,
        }
    }
}
