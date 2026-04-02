use ios_control_contracts::grounding::TargetInput;

pub fn prefers_semantic_target(input: &TargetInput) -> bool {
    input.semantic_label.is_some()
}
