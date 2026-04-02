use ios_control_contracts::grounding::PlanKind;
use plugin_grounding_core::action_selector::ActionSelector;
use plugin_grounding_core::focus_tracker::FocusTracker;

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
