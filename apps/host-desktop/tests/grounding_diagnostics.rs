use host_desktop::panels::diagnostics::GroundingDiagnosticsViewModel;

#[test]
fn grounding_diagnostics_formats_uncertainty_and_failure() {
    let view_model = GroundingDiagnosticsViewModel {
        pointer_uncertainty: 96.0,
        focus_confidence: 0.32,
        last_failure: Some("geometry_uncertain".into()),
    };

    assert_eq!(
        view_model.summary(),
        "pointer uncertainty 96.0, focus 0.32, last failure geometry_uncertain"
    );
}

#[test]
fn grounding_diagnostics_formats_none_when_no_failure() {
    let view_model = GroundingDiagnosticsViewModel {
        pointer_uncertainty: 12.3,
        focus_confidence: 0.5,
        last_failure: None,
    };

    assert_eq!(
        view_model.summary(),
        "pointer uncertainty 12.3, focus 0.50, last failure none"
    );
}
