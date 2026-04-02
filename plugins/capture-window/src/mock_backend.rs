use async_trait::async_trait;
use ios_control_contracts::capture::VideoFrameDescriptor;

use crate::backend::{mock_frame, WindowCaptureBackend, WindowSource};

#[derive(Default)]
pub struct MockWindowBackend {
    frame_index: u64,
}

impl MockWindowBackend {
    pub async fn list_sources(&mut self) -> anyhow::Result<Vec<WindowSource>> {
        <Self as WindowCaptureBackend>::list_sources(self).await
    }

    pub async fn next_frame(&mut self, source_id: &str) -> anyhow::Result<VideoFrameDescriptor> {
        <Self as WindowCaptureBackend>::next_frame(self, source_id).await
    }
}

#[async_trait]
impl WindowCaptureBackend for MockWindowBackend {
    async fn list_sources(&mut self) -> anyhow::Result<Vec<WindowSource>> {
        Ok(vec![WindowSource {
            source_id: "window:mock".into(),
            display_name: "Mock Mirroring Window".into(),
        }])
    }

    async fn next_frame(&mut self, source_id: &str) -> anyhow::Result<VideoFrameDescriptor> {
        self.frame_index += 1;
        Ok(mock_frame(source_id, self.frame_index))
    }
}
