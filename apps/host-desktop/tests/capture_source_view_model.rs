use host_desktop::panels::device_detail::CaptureSourceOption;

#[test]
fn capture_source_option_labels_window_and_direct_sources() {
    let window = CaptureSourceOption::new("window:airdroid", "AirDroid Window");
    let direct = CaptureSourceOption::new("direct:receiver", "Direct Receiver");
    let runtime_window = CaptureSourceOption::new("window-1", "Live Window");
    let runtime_direct = CaptureSourceOption::new("direct-1", "Live Direct");

    assert!(window.label().contains("Window"));
    assert!(direct.label().contains("Direct"));
    assert!(runtime_window.label().contains("Window"));
    assert!(runtime_direct.label().contains("Direct"));
}
