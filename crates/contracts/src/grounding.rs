use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TargetInput {
    pub semantic_label: Option<String>,
    pub visual_region: Option<(u32, u32, u32, u32)>,
    pub confidence: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GroundingRequest {
    pub target: TargetInput,
    pub device_size: (u32, u32),
    pub pointer_estimate: (f32, f32),
    pub uncertainty_radius: f32,
    pub focus_confidence: f32,
    pub keyboard_preferred: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GroundingPlan {
    pub kind: PlanKind,
    pub failure: Option<GroundingFailure>,
    pub summary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlanKind {
    Pointer,
    Keyboard,
    Hybrid,
}

impl PlanKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pointer => "pointer",
            Self::Keyboard => "keyboard",
            Self::Hybrid => "hybrid",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GroundingFailure {
    TargetAmbiguous,
    GeometryUncertain,
    FocusUncertain,
    ExecutionMismatch,
    RecoveryExhausted,
}

impl GroundingFailure {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::TargetAmbiguous => "target_ambiguous",
            Self::GeometryUncertain => "geometry_uncertain",
            Self::FocusUncertain => "focus_uncertain",
            Self::ExecutionMismatch => "execution_mismatch",
            Self::RecoveryExhausted => "recovery_exhausted",
        }
    }
}
