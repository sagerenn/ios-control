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
    pub start_enabled: bool,
}

impl SessionViewModel {
    pub fn idle() -> Self {
        Self {
            ui_state: SessionUiState::Idle,
            selected_source: None,
            latest_frame: None,
            start_enabled: false,
        }
    }

    pub fn idle_startable(selected_source: Option<CaptureSourceOption>) -> Self {
        Self {
            ui_state: SessionUiState::Idle,
            selected_source,
            latest_frame: None,
            start_enabled: true,
        }
    }

    pub fn starting() -> Self {
        Self {
            ui_state: SessionUiState::Starting,
            selected_source: None,
            latest_frame: None,
            start_enabled: false,
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
            start_enabled: false,
        }
    }

    pub fn streaming_without_frame(selected_source: CaptureSourceOption) -> Self {
        Self {
            ui_state: SessionUiState::Streaming,
            selected_source: Some(selected_source),
            latest_frame: None,
            start_enabled: false,
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            ui_state: SessionUiState::Error(message.into()),
            selected_source: None,
            latest_frame: None,
            start_enabled: true,
        }
    }

    pub fn blocked(message: impl Into<String>, selected_source: Option<CaptureSourceOption>) -> Self {
        let start_enabled = selected_source.is_some();
        Self {
            ui_state: SessionUiState::Error(message.into()),
            selected_source,
            latest_frame: None,
            start_enabled,
        }
    }

    pub fn can_start(&self) -> bool {
        self.start_enabled && matches!(self.ui_state, SessionUiState::Idle | SessionUiState::Error(_))
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
