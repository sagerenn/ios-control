use ios_control_contracts::grounding::{GroundingPlan, PlanKind};
use plugin_control_window_bridge::backend::command_for_plan;
use plugin_control_window_bridge::helper_launcher::helper_available;

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
