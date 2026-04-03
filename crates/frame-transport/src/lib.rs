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
        if bytes.len() > self.byte_len {
            bail!("frame larger than slot");
        }
        self.mmap[..bytes.len()].copy_from_slice(bytes);
        Ok(())
    }
}
