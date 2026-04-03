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
fn linux_capture_probe_requires_runtime_support() {
    let _guard = ENV_LOCK.lock().unwrap();
    let old_wayland = env::var_os("WAYLAND_DISPLAY");
    let old_display = env::var_os("DISPLAY");
    env::remove_var("WAYLAND_DISPLAY");
    env::remove_var("DISPLAY");

    if cfg!(target_os = "linux") {
        assert!(
            !probe_linux_capture(),
            "default test environment should not claim real capture support"
        );
        env::set_var("WAYLAND_DISPLAY", "wayland-0");
        assert!(probe_linux_capture());
    } else {
        assert!(!probe_linux_capture());
    }

    match old_wayland {
        Some(value) => env::set_var("WAYLAND_DISPLAY", value),
        None => env::remove_var("WAYLAND_DISPLAY"),
    }
    match old_display {
        Some(value) => env::set_var("DISPLAY", value),
        None => env::remove_var("DISPLAY"),
    }
}

#[test]
fn windows_capture_probe_requires_runtime_support() {
    let _guard = ENV_LOCK.lock().unwrap();
    let old_session = env::var_os("SESSIONNAME");
    env::remove_var("SESSIONNAME");

    if cfg!(target_os = "windows") {
        assert!(
            !probe_windows_capture(),
            "default test environment should not claim real capture support"
        );
        env::set_var("SESSIONNAME", "Console");
        assert!(probe_windows_capture());
    } else {
        assert!(!probe_windows_capture());
    }

    match old_session {
        Some(value) => env::set_var("SESSIONNAME", value),
        None => env::remove_var("SESSIONNAME"),
    }
}
