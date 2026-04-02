use host_desktop::panels::diagnostics::GroundingDiagnosticsViewModel;

#[test]
fn grounding_diagnostics_formats_uncertainty_and_failure() {
    let view_model = GroundingDiagnosticsViewModel {
        pointer_uncertainty: 96.0,
        focus_confidence: 0.32,
        last_failure: Some("geometry_uncertain".into()),
    };

    assert!(view_model.summary().contains("96.0"));
    assert!(view_model.summary().contains("geometry_uncertain"));
}
