use ios_control_contracts::control::ControlSessionPhase;
use ios_control_contracts::grounding::{GroundingPlan, PlanKind};
use ios_control_plugin_protocol::{HostToPlugin, PluginToHost};
use plugin_control_window_bridge::backend::command_for_plan;
use plugin_control_window_bridge::helper_launcher::{
    helper_available, launch_helper, launch_helper_json,
};
use std::env;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::Mutex;
#[cfg(unix)]
use std::{
    fs,
    io::ErrorKind,
    os::unix::fs::PermissionsExt,
    time::{SystemTime, UNIX_EPOCH},
};

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
#[test]
fn window_bridge_helper_rejects_non_executable_file() {
    let helper = write_test_helper_script("window-nonexec", "#!/bin/sh\nexit 0\n");
    let mut perms = fs::metadata(&helper).unwrap().permissions();
    perms.set_mode(0o644);
    fs::set_permissions(&helper, perms).unwrap();

    assert!(!helper_available(Some(helper.clone())));
    let _ = fs::remove_file(helper);
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
fn resolve_plugin_binary() -> PathBuf {
    if let Some(path) = env::var_os("CARGO_BIN_EXE_plugin-control-window-bridge") {
        return PathBuf::from(path);
    }

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("workspace root should be discoverable from plugin manifest")
        .to_path_buf();

    let mut target_dir = match env::var_os("CARGO_TARGET_DIR") {
        Some(path) => {
            let path = PathBuf::from(path);
            if path.is_absolute() {
                path
            } else {
                workspace_root.join(path)
            }
        }
        None => workspace_root.join("target"),
    };

    if let Some(target) = env::var_os("CARGO_BUILD_TARGET") {
        target_dir.push(target);
    }

    target_dir.join(format!(
        "debug/plugin-control-window-bridge{}",
        std::env::consts::EXE_SUFFIX
    ))
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

#[cfg(unix)]
#[test]
fn window_bridge_binary_helper_mode_runs_action_path() {
    let _guard = ENV_LOCK.lock().unwrap();
    let helper = resolve_plugin_binary();
    assert!(
        helper.is_file(),
        "expected plugin binary at {}",
        helper.display()
    );
    let log_path = env::temp_dir().join(format!(
        "ios-control-window-helper-action-{}-{}.log",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let old_log = env::var_os("IOS_CONTROL_WINDOW_INPUT_HELPER_ACTION_LOG");
    env::set_var("IOS_CONTROL_WINDOW_INPUT_HELPER_ACTION_LOG", &log_path);

    let plan = GroundingPlan {
        kind: PlanKind::Pointer,
        failure: None,
        summary: "selected pointer plan".into(),
    };
    let command = command_for_plan("window-helper-1", &plan).unwrap();
    let status = launch_helper(helper, &command.args).unwrap();
    assert!(status.success());

    let log = fs::read_to_string(&log_path).unwrap();
    assert!(log.contains("source=window-helper-1 action=pointer"));

    let _ = fs::remove_file(log_path);
    match old_log {
        Some(value) => env::set_var("IOS_CONTROL_WINDOW_INPUT_HELPER_ACTION_LOG", value),
        None => env::remove_var("IOS_CONTROL_WINDOW_INPUT_HELPER_ACTION_LOG"),
    }
}

#[cfg(unix)]
#[test]
fn window_bridge_helper_returns_structured_execution_summary() {
    let helper = write_test_helper_script(
        "window-json-exec",
        r#"#!/bin/sh
printf '%s\n' '{"phase":"Succeeded","summary":"window click applied","observed_change":true}'
"#,
    );

    let execution = launch_helper_json(
        helper.clone(),
        &[
            "--source".into(),
            "window-helper-1".into(),
            "--pointer-plan".into(),
        ],
    )
    .unwrap();
    assert_eq!(execution.summary, "window click applied");
    assert_eq!(execution.observed_change, Some(true));

    let _ = fs::remove_file(helper);
}

#[cfg(unix)]
#[test]
fn window_bridge_protocol_reports_unavailable_for_non_executable_helper() {
    let _guard = ENV_LOCK.lock().unwrap();
    let helper = write_test_helper_script("window-nonexec-protocol", "#!/bin/sh\nexit 0\n");
    let mut perms = fs::metadata(&helper).unwrap().permissions();
    perms.set_mode(0o644);
    fs::set_permissions(&helper, perms).unwrap();

    let plugin = resolve_plugin_binary();
    let mut child = Command::new(plugin)
        .env("IOS_CONTROL_WINDOW_INPUT_HELPER", &helper)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let requests = vec![
        HostToPlugin::Handshake {
            protocol_version: 3,
        },
        HostToPlugin::ProbeControl,
        HostToPlugin::PrepareControl,
        HostToPlugin::Stop,
    ];
    for request in requests {
        let line = serde_json::to_string(&request).unwrap();
        writeln!(stdin, "{line}").unwrap();
    }
    drop(stdin);

    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let lines = String::from_utf8(output.stdout).unwrap();
    let responses = lines
        .lines()
        .map(|line| serde_json::from_str::<PluginToHost>(line).unwrap())
        .collect::<Vec<_>>();
    assert!(matches!(
        responses.first(),
        Some(PluginToHost::HandshakeAck { .. })
    ));
    match responses.get(1) {
        Some(PluginToHost::ControlCapability { capability }) => {
            assert!(!capability.supported);
            assert_eq!(
                capability.reason.as_deref(),
                Some("IOS_CONTROL_WINDOW_INPUT_HELPER is not executable")
            );
        }
        other => panic!("unexpected ProbeControl response: {other:?}"),
    }
    match responses.get(2) {
        Some(PluginToHost::ControlSession { phase, checklist }) => {
            assert_eq!(*phase, ControlSessionPhase::Unavailable);
            assert!(checklist
                .items
                .iter()
                .any(|item| item.contains("not executable")));
        }
        other => panic!("unexpected PrepareControl response: {other:?}"),
    }

    let _ = fs::remove_file(helper);
}
