use crate::panels::device_detail::CaptureSourceOption;
use ios_control_contracts::capture::VideoFrameDescriptor;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionUiState {
    Idle,
    Starting,
    Streaming,
    Error(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionViewModel {
    pub ui_state: SessionUiState,
    pub selected_source: Option<CaptureSourceOption>,
    pub latest_frame: Option<VideoFrameDescriptor>,
}

impl SessionViewModel {
    pub fn idle() -> Self {
        Self {
            ui_state: SessionUiState::Idle,
            selected_source: None,
            latest_frame: None,
        }
    }

    pub fn starting() -> Self {
        Self {
            ui_state: SessionUiState::Starting,
            selected_source: None,
            latest_frame: None,
        }
    }

    pub fn streaming(
        selected_source: CaptureSourceOption,
        latest_frame: VideoFrameDescriptor,
    ) -> Self {
        Self {
            ui_state: SessionUiState::Streaming,
            selected_source: Some(selected_source),
            latest_frame: Some(latest_frame),
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            ui_state: SessionUiState::Error(message.into()),
            selected_source: None,
            latest_frame: None,
        }
    }

    pub fn can_start(&self) -> bool {
        matches!(self.ui_state, SessionUiState::Idle | SessionUiState::Error(_))
    }

    pub fn can_stop(&self) -> bool {
        matches!(self.ui_state, SessionUiState::Streaming)
    }

    pub fn status_line(&self) -> &str {
        match &self.ui_state {
            SessionUiState::Idle => "No active session",
            SessionUiState::Starting => "Starting session",
            SessionUiState::Streaming => "Streaming session",
            SessionUiState::Error(message) => message.as_str(),
        }
    }
}
