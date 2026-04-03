use ios_control_contracts::grounding::{GroundingPlan, PlanKind};
use plugin_control_window_bridge::backend::command_for_plan;
use plugin_control_window_bridge::helper_launcher::{helper_available, launch_helper};
use std::env;
use std::path::PathBuf;
use std::sync::Mutex;
#[cfg(unix)]
use std::{fs, io::ErrorKind, os::unix::fs::PermissionsExt, time::{SystemTime, UNIX_EPOCH}};

static ENV_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn window_bridge_formats_pointer_execution_for_helper() {
    let plan = GroundingPlan {
        kind: PlanKind::Pointer,
        failure: None,
        summary: "selected pointer plan".into(),
    };

    let command = command_for_plan("window-helper-1", &plan).unwrap();
    assert_eq!(
        command.args,
        vec!["--source", "window-helper-1", "--pointer-plan"]
    );
}

#[test]
fn window_bridge_helper_requires_existing_executable() {
    assert!(!helper_available(None));
}

#[cfg(unix)]
fn write_test_helper_script(name: &str, body: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = env::temp_dir().join(format!(
        "ios-control-{name}-{}-{nanos}.sh",
        std::process::id()
    ));
    fs::write(&path, body).unwrap();
    let mut perms = fs::metadata(&path).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&path, perms).unwrap();
    path
}

#[cfg(unix)]
#[test]
fn window_bridge_launch_times_out_for_hung_helper() {
    let _guard = ENV_LOCK.lock().unwrap();
    let old_timeout = env::var_os("IOS_CONTROL_WINDOW_INPUT_HELPER_TIMEOUT_MS");
    env::set_var("IOS_CONTROL_WINDOW_INPUT_HELPER_TIMEOUT_MS", "50");
    let helper = write_test_helper_script("window-timeout", "#!/bin/sh\nsleep 1\nexit 0\n");

    let err = launch_helper(helper.clone(), &[]).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::TimedOut);

    let _ = fs::remove_file(helper);
    match old_timeout {
        Some(value) => env::set_var("IOS_CONTROL_WINDOW_INPUT_HELPER_TIMEOUT_MS", value),
        None => env::remove_var("IOS_CONTROL_WINDOW_INPUT_HELPER_TIMEOUT_MS"),
    }
}
