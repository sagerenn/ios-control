use anyhow::{bail, Result};
use memmap2::MmapMut;

pub struct FrameSlot {
    mmap: MmapMut,
    byte_len: usize,
}

impl FrameSlot {
    pub fn new(byte_len: usize) -> Result<Self> {
        Ok(Self {
            mmap: MmapMut::map_anon(byte_len)?,
            byte_len,
        })
    }

    pub fn write(&mut self, bytes: &[u8]) -> Result<()> {
        if bytes.len() != self.byte_len {
            bail!(
                "frame size mismatch: expected {}, got {}",
                self.byte_len,
                bytes.len()
            );
        }
        self.mmap[..bytes.len()].copy_from_slice(bytes);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::FrameSlot;

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
}
