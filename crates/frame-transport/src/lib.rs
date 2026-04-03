use anyhow::{bail, Context, Result};
use memmap2::MmapMut;
use std::fs::{remove_file, File, OpenOptions};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct FrameSlot {
    file: File,
    path: PathBuf,
    mmap: MmapMut,
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
            file,
            path,
            mmap,
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
        self.mmap[..bytes.len()].copy_from_slice(bytes);
        Ok(())
    }
}

impl Drop for FrameSlot {
    fn drop(&mut self) {
        let _ = self.mmap.flush();
        let _ = self.file.sync_all();
        let _ = remove_file(&self.path);
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
