use plugin_capture_window::helper_config::WindowHelperConfig;
use plugin_capture_window::mock_backend::MockWindowBackend;
use plugin_capture_window::linux_backend::probe_linux_capture;
use plugin_capture_window::windows_backend::probe_windows_capture;
use std::env;
use std::sync::Mutex;

static ENV_LOCK: Mutex<()> = Mutex::new(());

#[tokio::test]
async fn window_capture_lists_mock_source_then_streams_one_frame() {
    let mut backend = MockWindowBackend::default();
    let sources = backend.list_sources().await.unwrap();

    assert_eq!(sources.len(), 1);
    assert_eq!(sources[0].source_id, "window:mock");

    let frame = backend.next_frame("window:mock").await.unwrap();
    assert_eq!(frame.frame_index, 1);
    assert_eq!(frame.width, 1280);
}

#[test]
fn window_capture_probe_reports_helper_backed_bridge_support() {
    let helper = tempfile::NamedTempFile::new().unwrap();
    let config = WindowHelperConfig::from_parts(
        Some(helper.path().to_path_buf()),
        Some("Operator Mirror".into()),
    )
    .unwrap();

    let capability = config.capture_capability();
    assert!(capability.available);
    assert_eq!(capability.backend_id, "capture.window.helper");
    assert!(capability.supports_input_bridge);
}

#[test]
fn linux_capture_probe_requires_helper_configuration() {
    let _guard = ENV_LOCK.lock().unwrap();
    let old_helper = env::var_os("IOS_CONTROL_WINDOW_CAPTURE_HELPER");
    env::remove_var("IOS_CONTROL_WINDOW_CAPTURE_HELPER");
    let helper = tempfile::NamedTempFile::new().unwrap();

    if cfg!(target_os = "linux") {
        assert!(
            !probe_linux_capture(),
            "default test environment should not claim real capture support"
        );
        env::set_var("IOS_CONTROL_WINDOW_CAPTURE_HELPER", helper.path());
        assert!(probe_linux_capture());
    } else {
        assert!(!probe_linux_capture());
    }

    match old_helper {
        Some(value) => env::set_var("IOS_CONTROL_WINDOW_CAPTURE_HELPER", value),
        None => env::remove_var("IOS_CONTROL_WINDOW_CAPTURE_HELPER"),
    }
}

#[test]
fn windows_capture_probe_requires_helper_configuration() {
    let _guard = ENV_LOCK.lock().unwrap();
    let old_helper = env::var_os("IOS_CONTROL_WINDOW_CAPTURE_HELPER");
    env::remove_var("IOS_CONTROL_WINDOW_CAPTURE_HELPER");
    let helper = tempfile::NamedTempFile::new().unwrap();

    if cfg!(target_os = "windows") {
        assert!(
            !probe_windows_capture(),
            "default test environment should not claim real capture support"
        );
        env::set_var("IOS_CONTROL_WINDOW_CAPTURE_HELPER", helper.path());
        assert!(probe_windows_capture());
    } else {
        assert!(!probe_windows_capture());
    }

    match old_helper {
        Some(value) => env::set_var("IOS_CONTROL_WINDOW_CAPTURE_HELPER", value),
        None => env::remove_var("IOS_CONTROL_WINDOW_CAPTURE_HELPER"),
    }
}
