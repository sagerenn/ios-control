use anyhow::{bail, Context, Result};
use memmap2::{Mmap, MmapMut};
use std::fs::{remove_file, File, OpenOptions};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct FrameSlot {
    file: Option<File>,
    path: PathBuf,
    mmap: Option<MmapMut>,
    byte_len: usize,
}

impl FrameSlot {
    pub fn new(byte_len: usize) -> Result<Self> {
        let mut path = std::env::temp_dir();
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .context("system clock before unix epoch")?
            .as_nanos();
        path.push(format!("ios-control-frame-slot-{}-{}.bin", std::process::id(), nonce));

        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&path)
            .with_context(|| format!("failed to create frame slot at {}", path.display()))?;
        file.set_len(byte_len as u64)
            .with_context(|| format!("failed to size frame slot at {}", path.display()))?;
        let mmap = unsafe { MmapMut::map_mut(&file)? };
        Ok(Self {
            file: Some(file),
            path,
            mmap: Some(mmap),
            byte_len,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn byte_len(&self) -> usize {
        self.byte_len
    }

    pub fn write(&mut self, bytes: &[u8]) -> Result<()> {
        if bytes.len() != self.byte_len {
            bail!(
                "frame size mismatch: expected {}, got {}",
                self.byte_len,
                bytes.len()
            );
        }
        let mmap = self
            .mmap
            .as_mut()
            .expect("frame slot mapping unavailable during write");
        mmap[..bytes.len()].copy_from_slice(bytes);
        Ok(())
    }
}

pub struct FrameSlotReader {
    file: File,
    mmap: Mmap,
    byte_len: usize,
}

impl FrameSlotReader {
    pub fn open(path: &Path, byte_len: usize) -> Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .open(path)
            .with_context(|| format!("failed to open frame slot at {}", path.display()))?;
        let mmap = unsafe { Mmap::map(&file)? };
        if mmap.len() < byte_len {
            bail!(
                "frame slot too small: expected at least {}, got {}",
                byte_len,
                mmap.len()
            );
        }
        Ok(Self {
            file,
            mmap,
            byte_len,
        })
    }

    pub fn read(&self) -> &[u8] {
        let _ = &self.file;
        &self.mmap[..self.byte_len]
    }
}

pub fn decode_base64_bytes(input: &str) -> Result<Vec<u8>> {
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
            _ => bail!("invalid base64 byte"),
        };
        len += 1;

        if len == 4 {
            if chunk[0] == 64 || chunk[1] == 64 {
                bail!("invalid base64 padding");
            }

            output.push((chunk[0] << 2) | (chunk[1] >> 4));
            if chunk[2] != 64 {
                output.push((chunk[1] << 4) | (chunk[2] >> 2));
                if chunk[3] != 64 {
                    output.push((chunk[2] << 6) | chunk[3]);
                }
            } else if chunk[3] != 64 {
                bail!("invalid base64 padding");
            }

            len = 0;
        }
    }

    if len != 0 {
        bail!("invalid base64 length");
    }

    Ok(output)
}

impl Drop for FrameSlot {
    fn drop(&mut self) {
        if let Some(mmap) = self.mmap.take() {
            let _ = mmap.flush();
        }
        if let Some(file) = self.file.take() {
            let _ = file.sync_all();
        }
        let _ = remove_file(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::{decode_base64_bytes, FrameSlot, FrameSlotReader};

    #[test]
    fn write_accepts_exact_size() {
        let mut slot = FrameSlot::new(4).unwrap();
        slot.write(&[1, 2, 3, 4]).unwrap();
    }

    #[test]
    fn write_rejects_wrong_size() {
        let mut slot = FrameSlot::new(4).unwrap();
        assert!(slot.write(&[1, 2, 3]).is_err());
        assert!(slot.write(&[1, 2, 3, 4, 5]).is_err());
    }

    #[test]
    fn frame_slot_reader_reads_exact_rgba_bytes() {
        let mut slot = FrameSlot::new(8).unwrap();
        slot.write(&[255, 0, 0, 255, 0, 255, 0, 255]).unwrap();

        let reader = FrameSlotReader::open(slot.path(), 8).unwrap();
        assert_eq!(reader.read(), &[255, 0, 0, 255, 0, 255, 0, 255]);
    }

    #[test]
    fn decode_base64_bytes_decodes_rgba_payload() {
        assert_eq!(decode_base64_bytes("AP8A/w==").unwrap(), vec![0, 255, 0, 255]);
    }
}
