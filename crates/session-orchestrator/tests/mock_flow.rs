use ios_control_contracts::plugin::PluginHealth;
use ios_control_contracts::session::SessionPhase;
use ios_control_session_orchestrator::{PluginPaths, SessionOrchestrator, StartSessionRequest};

mod support;
use support::{build_plugins, plugin_path, workspace_root};

#[tokio::test]
async fn start_session_collects_mock_plugin_state() {
    let root = workspace_root();
    build_plugins(&root);

    let mut orchestrator = SessionOrchestrator::default();
    let state = orchestrator
        .start_session_with_plugins(StartSessionRequest {
            device_id: "device-1".into(),
            device_name: "Mock iPhone".into(),
            selected_source_id: Some("window-1".into()),
            plugin_paths: PluginPaths {
                capture: plugin_path(&root, "plugin-capture-window"),
                control: plugin_path(&root, "plugin-control-ble"),
                grounding: Some(plugin_path(&root, "plugin-grounding-core")),
            },
        })
        .await
        .unwrap();

    assert_eq!(state.summary.device_id, "device-1");
    assert_eq!(state.summary.device_name, "Mock iPhone");
    assert_eq!(state.summary.phase, SessionPhase::Streaming);
    assert_eq!(state.summary.plugin_health, PluginHealth::Healthy);
    assert_eq!(
        state.summary.capture_plugin.as_deref(),
        Some("capture.window")
    );
    assert_eq!(state.summary.control_plugin.as_deref(), Some("control.ble"));
    assert_eq!(
        state.summary.grounding_plugin.as_deref(),
        Some("grounding.core")
    );
    assert_eq!(state.selected_source_id.as_deref(), Some("window-1"));

    assert_eq!(state.capture_sources.len(), 1);
    assert_eq!(state.capture_sources[0].source_id, "window-1");
    assert_eq!(state.latest_frame.as_ref().unwrap().source_id, "window-1");
    assert_eq!(state.control_checklist.items.len(), 2);
    assert!(state
        .control_checklist
        .items
        .iter()
        .any(|item| item.contains("Enable Bluetooth")));
    assert!(state.diagnostics.control_summary.contains("supported"));
    assert!(state
        .diagnostics
        .grounding_summary
        .as_deref()
        .unwrap()
        .contains("selected"));
    assert!(state.execution_result.is_some());
    let execution = state.execution_result.as_ref().unwrap();
    assert!(!execution.applied);
    assert!(execution.summary.contains("execution payload"));

    let control_capability = orchestrator.capabilities.get("control.ble").unwrap();
    assert!(control_capability.supported);
    assert_eq!(control_capability.reason, None);

    let device = orchestrator.devices.get("device-1").unwrap();
    assert_eq!(device.device_name, "Mock iPhone");
    assert_eq!(device.preferred_capture_plugin, "capture.window");
    assert_eq!(device.preferred_control_plugin, "control.ble");
    assert_eq!(
        device.preferred_grounding_plugin.as_deref(),
        Some("grounding.core")
    );
    assert_eq!(device.last_source_id.as_deref(), Some("window-1"));

    let telemetry = orchestrator.telemetry.for_session("device-1");
    assert!(telemetry
        .iter()
        .any(|event| event.message == "session started"));
    assert!(telemetry
        .iter()
        .any(|event| event.message.contains("capture source")));
    assert!(telemetry
        .iter()
        .any(|event| event.message.contains("grounding planned")));

    state.shutdown().await.unwrap();
}

#[tokio::test]
async fn start_session_failure_does_not_persist_partial_state() {
    let root = workspace_root();
    build_plugins(&root);

    let mut orchestrator = SessionOrchestrator::default();
    let error = orchestrator
        .start_session_with_plugins(StartSessionRequest {
            device_id: "device-2".into(),
            device_name: "Broken Mock iPhone".into(),
            selected_source_id: Some("missing-source".into()),
            plugin_paths: PluginPaths {
                capture: plugin_path(&root, "plugin-capture-window"),
                control: plugin_path(&root, "plugin-control-ble"),
                grounding: Some(plugin_path(&root, "plugin-grounding-core")),
            },
        })
        .await
        .unwrap_err();

    assert!(error
        .to_string()
        .contains("requested capture source `missing-source` is unavailable"));
    assert!(orchestrator.capabilities.entries().is_empty());
    assert!(orchestrator.devices.entries().is_empty());
    assert!(orchestrator.telemetry.events().is_empty());
}
