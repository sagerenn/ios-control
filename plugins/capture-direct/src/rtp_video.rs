use anyhow::{anyhow, Result};
use image::imageops::FilterType;
use std::error::Error as StdError;
use std::path::{Path, PathBuf};

use crate::backend::{DIRECT_HEIGHT, DIRECT_WIDTH};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedFrame {
    pub frame_index: u64,
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

pub fn receiver_args(port: u16, frame_pattern: &Path) -> Vec<String> {
    vec![
        "-q".into(),
        "udpsrc".into(),
        format!("port={port}"),
        "caps=application/x-rtp,media=video,clock-rate=90000,encoding-name=H264,payload=96".into(),
        "!".into(),
        "rtph264depay".into(),
        "!".into(),
        "h264parse".into(),
        "!".into(),
        "decodebin".into(),
        "!".into(),
        "videoconvert".into(),
        "!".into(),
        "pngenc".into(),
        "!".into(),
        format!("multifilesink location={}", frame_pattern.display()),
    ]
}

pub fn next_frame(frame_dir: &Path, last_index: &mut u64) -> Result<Option<DecodedFrame>> {
    for (frame_index, path) in candidate_frames(frame_dir, *last_index)? {
        let frame = match decode_frame(&path, frame_index) {
            Ok(frame) => frame,
            Err(err) if is_incomplete_frame_error(&err) => continue,
            Err(err) => return Err(err.into()),
        };
        *last_index = frame_index;
        let _ = std::fs::remove_file(&path);
        return Ok(Some(frame));
    }

    Ok(None)
}

fn candidate_frames(frame_dir: &Path, last_index: u64) -> Result<Vec<(u64, PathBuf)>> {
    let mut frames = Vec::new();
    for entry in std::fs::read_dir(frame_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("png") {
            continue;
        }
        let index = parse_frame_index(&path)?;
        if index <= last_index {
            continue;
        }
        frames.push((index, path));
    }
    frames.sort_unstable_by(|left, right| right.0.cmp(&left.0));
    Ok(frames)
}

fn decode_frame(path: &Path, frame_index: u64) -> image::ImageResult<DecodedFrame> {
    let image = image::open(path)?.to_rgba8();
    let resized =
        image::imageops::resize(&image, DIRECT_WIDTH, DIRECT_HEIGHT, FilterType::Triangle);
    Ok(DecodedFrame {
        frame_index,
        width: DIRECT_WIDTH,
        height: DIRECT_HEIGHT,
        rgba: resized.into_raw(),
    })
}

fn is_incomplete_frame_error(err: &image::ImageError) -> bool {
    let mut current: Option<&(dyn StdError + 'static)> = Some(err);
    while let Some(err) = current {
        if let Some(io_err) = err.downcast_ref::<std::io::Error>() {
            if io_err.kind() == std::io::ErrorKind::UnexpectedEof {
                return true;
            }
        }
        if err.to_string().contains("unexpected end of file") {
            return true;
        }
        current = err.source();
    }
    false
}

fn parse_frame_index(path: &Path) -> Result<u64> {
    let stem = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .ok_or_else(|| anyhow!("missing frame stem for {}", path.display()))?;
    let suffix = stem
        .rsplit_once('-')
        .map(|(_, suffix)| suffix)
        .ok_or_else(|| anyhow!("invalid frame file name {}", path.display()))?;
    suffix
        .parse::<u64>()
        .map_err(|err| anyhow!("invalid frame index in {}: {err}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::next_frame;
    use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};
    use std::io::Cursor;

    fn sample_png_bytes() -> Vec<u8> {
        let image = RgbaImage::from_pixel(1, 1, Rgba([0, 255, 0, 255]));
        let mut cursor = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(image)
            .write_to(&mut cursor, ImageFormat::Png)
            .unwrap();
        cursor.into_inner()
    }

    #[test]
    fn next_frame_waits_for_incomplete_png_to_finish_writing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("frame-000000001.png");
        let png = sample_png_bytes();
        std::fs::write(&path, &png[..png.len() / 2]).unwrap();

        let mut last_index = 0;
        assert!(next_frame(dir.path(), &mut last_index).unwrap().is_none());
        assert_eq!(last_index, 0);

        std::fs::write(&path, png).unwrap();
        let frame = next_frame(dir.path(), &mut last_index)
            .unwrap()
            .expect("completed frame should decode after retry");
        assert_eq!(frame.frame_index, 1);
        assert_eq!(last_index, 1);
        assert!(!path.exists());
    }
}
