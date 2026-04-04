use std::path::Path;

use anyhow::Result;
use ios_control_contracts::capture::CaptureStreamDescriptor;
use ios_control_frame_transport::FrameSlotReader;

pub fn color_image_from_slot(stream: &CaptureStreamDescriptor) -> Result<egui::ColorImage> {
    let reader = FrameSlotReader::open(Path::new(&stream.slot_path), stream.slot_bytes as usize)?;
    Ok(egui::ColorImage::from_rgba_unmultiplied(
        [stream.width as usize, stream.height as usize],
        reader.read(),
    ))
}
