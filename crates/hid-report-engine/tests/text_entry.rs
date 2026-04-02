use ios_control_hid_report_engine::expand_text_entry;

#[test]
fn text_entry_expands_to_key_press_sequence() {
    let sequence = expand_text_entry("Ab");

    assert_eq!(sequence.len(), 2);
    assert_eq!(sequence[0].modifiers.shift, true);
    assert_eq!(sequence[0].usage_id, 0x04);
    assert_eq!(sequence[1].modifiers.shift, false);
    assert_eq!(sequence[1].usage_id, 0x05);
}
