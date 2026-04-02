use plugin_grounding_core::coordinate_mapper::CoordinateMapper;

#[test]
fn pointer_plan_is_rejected_when_uncertainty_exceeds_target_size() {
    let mapper = CoordinateMapper::new((1179, 2556), (400.0, 400.0), 120.0);

    assert!(!mapper.can_confidently_hit((350, 350, 40, 40)));
    assert!(mapper.can_confidently_hit((350, 350, 320, 320)));
}

#[test]
fn pointer_plan_is_rejected_when_pointer_estimate_is_outside_target_region() {
    let mapper = CoordinateMapper::new((1179, 2556), (100.0, 100.0), 20.0);

    assert!(!mapper.can_confidently_hit((350, 350, 320, 320)));
}
