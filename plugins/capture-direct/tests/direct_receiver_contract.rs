use plugin_capture_direct::helper_bridge::HelperFrameEvent;
use plugin_capture_direct::helper_launcher::capture_capability;

#[test]
fn direct_receiver_probe_requires_existing_executable() {
    let capability = capture_capability(None);
    assert!(!capability.available);
    assert_eq!(
        capability.reason.as_deref(),
        Some("IOS_CONTROL_DIRECT_RECEIVER_HELPER not configured")
    );
}

#[test]
fn direct_helper_frame_event_requires_slot_fill_metadata() {
    let event: HelperFrameEvent =
        serde_json::from_str(r#"{"frame_index":3,"width":1179,"height":2556,"fill_byte":64}"#)
            .unwrap();

    assert_eq!(event.frame_index, 3);
    assert_eq!(event.fill_byte, 64);
}
