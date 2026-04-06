use ios_control_contracts::capture::{
    AudioRoute, AudioStreamPhase, AudioStreamStatus, CaptureStatus, CaptureStreamPhase,
    FrameHealth,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectCaptureStatus {
    pub video_phase: CaptureStreamPhase,
    pub video_health: FrameHealth,
    pub audio_phase: AudioStreamPhase,
    pub audio_route: AudioRoute,
    pub audio_active: bool,
    pub detail: Option<String>,
}

impl Default for DirectCaptureStatus {
    fn default() -> Self {
        Self {
            video_phase: CaptureStreamPhase::Opening,
            video_health: FrameHealth::Healthy,
            audio_phase: AudioStreamPhase::Idle,
            audio_route: AudioRoute::None,
            audio_active: false,
            detail: None,
        }
    }
}

impl DirectCaptureStatus {
    pub fn to_capture_status(&self) -> CaptureStatus {
        CaptureStatus {
            video_phase: self.video_phase,
            video_health: self.video_health,
            audio: AudioStreamStatus {
                phase: self.audio_phase,
                route: self.audio_route,
                active: self.audio_active,
                detail: self.detail.clone(),
            },
            detail: self.detail.clone(),
        }
    }
}
