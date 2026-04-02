use ios_control_contracts::capture::{FrameHealth, SourceKind, VideoFrameDescriptor};

#[test]
fn frame_descriptor_roundtrips_orientation_and_health() {
    let descriptor = VideoFrameDescriptor {
        source_id: "window:airdroid".into(),
        source_kind: SourceKind::Window,
        width: 1179,
        height: 2556,
        rotation_degrees: 90,
        frame_index: 7,
        health: FrameHealth::Occluded,
    };

    let encoded = serde_json::to_string(&descriptor).unwrap();
    let decoded: VideoFrameDescriptor = serde_json::from_str(&encoded).unwrap();

    assert_eq!(decoded.rotation_degrees, 90);
    assert_eq!(decoded.health, FrameHealth::Occluded);
}
