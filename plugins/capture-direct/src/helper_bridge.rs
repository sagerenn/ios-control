use anyhow::{anyhow, Result};
use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct HelperProbe {
    pub available: bool,
    pub supports_input_bridge: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct HelperFrameEvent {
    pub frame_index: u64,
    pub width: u32,
    pub height: u32,
    #[serde(default)]
    pub fill_byte: u8,
    #[serde(default)]
    pub rgba_base64: String,
}

impl HelperFrameEvent {
    pub fn decode_rgba(&self) -> Result<Vec<u8>> {
        decode_base64(&self.rgba_base64)
    }
}

fn decode_base64(input: &str) -> Result<Vec<u8>> {
    let mut output = Vec::with_capacity(input.len() * 3 / 4);
    let mut chunk = [0_u8; 4];
    let mut len = 0_usize;

    for byte in input.bytes().filter(|byte| !byte.is_ascii_whitespace()) {
        chunk[len] = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' => 64,
            _ => return Err(anyhow!("invalid base64 byte")),
        };
        len += 1;

        if len == 4 {
            if chunk[0] == 64 || chunk[1] == 64 {
                return Err(anyhow!("invalid base64 padding"));
            }

            output.push((chunk[0] << 2) | (chunk[1] >> 4));
            if chunk[2] != 64 {
                output.push((chunk[1] << 4) | (chunk[2] >> 2));
                if chunk[3] != 64 {
                    output.push((chunk[2] << 6) | chunk[3]);
                }
            } else if chunk[3] != 64 {
                return Err(anyhow!("invalid base64 padding"));
            }

            len = 0;
        }
    }

    if len != 0 {
        return Err(anyhow!("invalid base64 length"));
    }

    Ok(output)
}
