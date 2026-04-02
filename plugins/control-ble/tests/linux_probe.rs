use plugin_control_ble::linux_backend::LinuxProbeResult;

#[test]
fn linux_probe_marks_unsupported_when_bluez_service_missing() {
    let probe = LinuxProbeResult::from_service_name(None);

    assert!(!probe.supported);
    assert_eq!(probe.reason.as_deref(), Some("org.bluez not available"));
}
