use ios_control_contracts::control::{
    ControlInputEvent, ExecutionPhase, ExecutionSummary, MouseInputReport,
};

#[test]
fn execution_summary_roundtrips_observed_change() {
    let summary = ExecutionSummary {
        summary: "pointer action applied".into(),
        phase: ExecutionPhase::Succeeded,
        observed_change: Some(true),
        failure_reason: None,
    };

    let json = serde_json::to_string(&summary).unwrap();
    let decoded: ExecutionSummary = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, summary);
}

#[test]
fn mouse_sequence_input_roundtrips() {
    let event = ControlInputEvent::MouseSequence(vec![
        MouseInputReport {
            buttons: 0,
            dx: 100,
            dy: -50,
            wheel: 0,
        },
        MouseInputReport {
            buttons: 1,
            dx: 0,
            dy: 0,
            wheel: 0,
        },
    ]);

    let json = serde_json::to_string(&event).unwrap();
    let decoded: ControlInputEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, event);
}
