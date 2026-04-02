use ios_control_contracts::grounding::{GroundingFailure, PlanKind, TargetInput};

#[test]
fn target_input_accepts_semantic_and_visual_data() {
    let target = TargetInput {
        semantic_label: Some("Settings".into()),
        visual_region: Some((10, 20, 30, 40)),
        confidence: 0.85,
    };

    assert_eq!(target.semantic_label.as_deref(), Some("Settings"));
    assert_eq!(target.visual_region, Some((10, 20, 30, 40)));
    assert_eq!(PlanKind::Hybrid.as_str(), "hybrid");
    assert_eq!(
        GroundingFailure::RecoveryExhausted.as_str(),
        "recovery_exhausted"
    );
}
