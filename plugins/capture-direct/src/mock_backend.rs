use async_trait::async_trait;
use ios_control_contracts::capture::VideoFrameDescriptor;

use crate::backend::{first_frame, DirectReceiverBackend};

pub struct MockDirectReceiverBackend {
    error: Option<String>,
}

impl MockDirectReceiverBackend {
    pub fn unavailable(message: &str) -> Self {
        Self {
            error: Some(message.into()),
        }
    }

    pub async fn start_session(&self) -> anyhow::Result<VideoFrameDescriptor> {
        <Self as DirectReceiverBackend>::start_session(self).await
    }
}

#[async_trait]
impl DirectReceiverBackend for MockDirectReceiverBackend {
    async fn start_session(&self) -> anyhow::Result<VideoFrameDescriptor> {
        if let Some(message) = &self.error {
            anyhow::bail!(message.clone());
        }

        Ok(first_frame("direct:mock"))
    }
}
