use host_desktop::panels::device_detail::ControlSetupChecklist;

#[test]
fn setup_checklist_marks_assistivetouch_required_in_pointer_mode() {
    let checklist = ControlSetupChecklist::for_pointer_mode();

    assert!(checklist
        .items
        .iter()
        .any(|item| item.contains("AssistiveTouch")));
    assert!(checklist
        .items
        .iter()
        .any(|item| item.contains("Full Keyboard Access")));
}
