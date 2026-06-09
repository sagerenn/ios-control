use ios_control_contracts::capture::{
    CaptureStreamDescriptor, FrameHealth, SourceKind, VideoFrameDescriptor,
};

use host_desktop::preview::color_image_from_slot;

mod support;

#[test]
fn color_image_from_slot_reads_rgba_frame() {
    let descriptor = CaptureStreamDescriptor {
        source_id: "window-helper-1".into(),
        source_kind: SourceKind::Window,
        width: 100,
        height: 100,
        rotation_degrees: 0,
        slot_bytes: 8,
        slot_path: support::write_slot_bytes(&[255, 0, 0, 255, 0, 255, 0, 255]),
    };
    let frame = VideoFrameDescriptor {
        source_id: "window-helper-1".into(),
        source_kind: SourceKind::Window,
        width: 2,
        height: 1,
        rotation_degrees: 0,
        frame_index: 1,
        health: FrameHealth::Healthy,
    };

    let image = color_image_from_slot(&descriptor, &frame).unwrap();
    assert_eq!(image.size, [2, 1]);
    assert_eq!(image.pixels.len(), 2);
}
