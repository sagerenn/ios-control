use ios_control_contracts::grounding::{GroundingFailure, PlanKind};
use plugin_grounding_core::action_selector::ActionSelector;
use plugin_grounding_core::execution_monitor::ExecutionMonitor;
use plugin_grounding_core::focus_tracker::FocusTracker;
use plugin_grounding_core::recovery_controller::RecoveryController;

#[test]
fn keyboard_plan_wins_when_focus_confidence_is_high() {
    let selector = ActionSelector::default();
    let focus = FocusTracker {
        focus_confidence: 0.9,
        keyboard_friendly: true,
    };

    let plan = selector.choose_plan(true, &focus, 120.0).unwrap();

    assert_eq!(plan.kind, PlanKind::Keyboard);
}

#[test]
fn pointer_plan_wins_when_keyboard_not_preferred_and_pointer_is_precise() {
    let selector = ActionSelector::default();
    let focus = FocusTracker {
        focus_confidence: 0.2,
        keyboard_friendly: false,
    };

    let plan = selector.choose_plan(true, &focus, 20.0).unwrap();

    assert_eq!(plan.kind, PlanKind::Pointer);
}

#[test]
fn choose_plan_fails_when_keyboard_not_preferred_and_pointer_not_viable() {
    let selector = ActionSelector::default();
    let focus = FocusTracker {
        focus_confidence: 0.3,
        keyboard_friendly: false,
    };

    let err = selector.choose_plan(false, &focus, 120.0).unwrap_err();

    assert_eq!(err, GroundingFailure::GeometryUncertain);
}

#[test]
fn screen_changed_reports_difference() {
    assert!(!ExecutionMonitor::screen_changed(1234, 1234));
    assert!(ExecutionMonitor::screen_changed(1234, 5678));
}

#[test]
fn recovery_controller_allows_one_obvious_retry_then_exhausts() {
    let mut controller = RecoveryController::default();

    let first = controller.next_action(true).unwrap();
    assert!(first);

    let second = controller.next_action(true).unwrap_err();
    assert_eq!(second, GroundingFailure::RecoveryExhausted);
}
