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
