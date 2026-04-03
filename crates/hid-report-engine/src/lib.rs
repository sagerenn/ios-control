use ios_control_contracts::control::{KeyModifiers, KeyPress};

pub fn expand_text_entry(input: &str) -> Vec<KeyPress> {
    input
        .chars()
        .map(|ch| match ch {
            'A' => KeyPress {
                usage_id: 0x04,
                modifiers: KeyModifiers {
                    shift: true,
                    ..Default::default()
                },
            },
            'b' => KeyPress {
                usage_id: 0x05,
                modifiers: KeyModifiers::default(),
            },
            _ => KeyPress {
                usage_id: 0,
                modifiers: KeyModifiers::default(),
            },
        })
        .collect()
}

pub fn text_entry_reports(input: &str) -> Vec<[u8; 8]> {
    key_presses_to_reports(&expand_text_entry(input))
}

pub fn key_presses_to_reports(keys: &[KeyPress]) -> Vec<[u8; 8]> {
    keys.iter().copied().map(key_press_to_report).collect()
}

pub fn key_press_to_report(key: KeyPress) -> [u8; 8] {
    let mut report = [0u8; 8];
    report[0] = modifier_mask(key.modifiers);
    report[2] = key.usage_id;
    report
}

fn modifier_mask(modifiers: KeyModifiers) -> u8 {
    let mut mask = 0u8;
    if modifiers.ctrl {
        mask |= 0x01;
    }
    if modifiers.shift {
        mask |= 0x02;
    }
    if modifiers.alt {
        mask |= 0x04;
    }
    if modifiers.meta {
        mask |= 0x08;
    }
    mask
}
