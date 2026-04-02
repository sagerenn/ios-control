use anyhow::Result;
use memmap2::MmapMut;

pub struct FrameSlot {
    mmap: MmapMut,
}

impl FrameSlot {
    pub fn new(byte_len: usize) -> Result<Self> {
        Ok(Self {
            mmap: MmapMut::map_anon(byte_len)?,
        })
    }

    pub fn write(&mut self, bytes: &[u8]) {
        self.mmap[..bytes.len()].copy_from_slice(bytes);
    }
}
