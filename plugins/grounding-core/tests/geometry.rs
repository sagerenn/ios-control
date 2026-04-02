use plugin_grounding_core::coordinate_mapper::CoordinateMapper;

#[test]
fn pointer_plan_is_rejected_when_uncertainty_exceeds_target_size() {
    let mapper = CoordinateMapper::new((1179, 2556), (400.0, 400.0), 120.0);

    assert!(!mapper.can_confidently_hit((350, 350, 40, 40)));
    assert!(mapper.can_confidently_hit((350, 350, 320, 320)));
}
