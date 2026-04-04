use ios_control_contracts::control::{ExecutionPhase, ExecutionSummary};

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
