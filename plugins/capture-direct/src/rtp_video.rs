use anyhow::{anyhow, Result};
use image::imageops::FilterType;
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
    let Some(path) = newest_frame(frame_dir, *last_index)? else {
        return Ok(None);
    };
    let frame_index = parse_frame_index(&path)?;
    let image = image::open(&path)?.to_rgba8();
    let resized = image::imageops::resize(&image, DIRECT_WIDTH, DIRECT_HEIGHT, FilterType::Triangle);
    *last_index = frame_index;
    let _ = std::fs::remove_file(&path);
    Ok(Some(DecodedFrame {
        frame_index,
        width: DIRECT_WIDTH,
        height: DIRECT_HEIGHT,
        rgba: resized.into_raw(),
    }))
}

fn newest_frame(frame_dir: &Path, last_index: u64) -> Result<Option<PathBuf>> {
    let mut selected: Option<(u64, PathBuf)> = None;
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
        match selected.as_ref() {
            Some((current, _)) if *current >= index => {}
            _ => selected = Some((index, path)),
        }
    }
    Ok(selected.map(|(_, path)| path))
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
