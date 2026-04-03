use plugin_control_ble::windows_backend::WindowsProbeResult;

#[test]
fn windows_probe_marks_unsupported_without_peripheral_role() {
    let probe = WindowsProbeResult::from_peripheral_role(false);

    assert!(!probe.supported);
    assert_eq!(
        probe.reason.as_deref(),
        Some("bluetooth peripheral role not supported")
    );
}

#[test]
fn windows_probe_reports_reason_when_radio_missing() {
    let probe = WindowsProbeResult::from_runtime_checks(false, false);

    assert!(!probe.supported);
    assert_eq!(
        probe.reason.as_deref(),
        Some("bluetooth radio not detected")
    );
}
