use host_desktop::panels::device_detail::CaptureSourceOption;

#[test]
fn capture_source_option_labels_window_and_direct_sources() {
    let window = CaptureSourceOption::new("window:airdroid", "AirDroid Window");
    let direct = CaptureSourceOption::new("direct:receiver", "Direct Receiver");

    assert!(window.label().contains("Window"));
    assert!(direct.label().contains("Direct"));
}
