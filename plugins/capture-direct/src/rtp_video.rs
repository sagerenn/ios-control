use anyhow::{anyhow, Result};
use std::error::Error as StdError;
use std::io::{ErrorKind, Read};
use std::path::{Path, PathBuf};
use std::process::ChildStdout;
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

const DIAGNOSTIC_FRAME_LIMIT: usize = 500;
const RTP_JITTERBUFFER_LATENCY_MS: u16 = 35;
const DEFAULT_LIVE_FRAME_WIDTH: u32 = 720;
const DEFAULT_LIVE_FRAME_HEIGHT: u32 = 1280;
const DEFAULT_LIVE_FRAME_MAX_FPS: u32 = 20;
const MAX_LIVE_FRAME_FPS: u32 = 60;
const VIDEO_RTP_CAPS: &str =
    "application/x-rtp,media=video,clock-rate=90000,encoding-name=H264,payload=96";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedFrame {
    pub frame_index: u64,
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LiveFrameConfig {
    pub width: u32,
    pub height: u32,
    pub max_fps: u32,
}

impl LiveFrameConfig {
    pub fn from_env() -> Self {
        Self {
            width: env_u32("IOS_CONTROL_DIRECT_PREVIEW_WIDTH", DEFAULT_LIVE_FRAME_WIDTH),
            height: env_u32(
                "IOS_CONTROL_DIRECT_PREVIEW_HEIGHT",
                DEFAULT_LIVE_FRAME_HEIGHT,
            ),
            max_fps: env_u32("IOS_CONTROL_DIRECT_PREVIEW_FPS", DEFAULT_LIVE_FRAME_MAX_FPS)
                .clamp(1, MAX_LIVE_FRAME_FPS),
        }
    }

    pub fn byte_len(self) -> Result<usize> {
        (self.width as usize)
            .checked_mul(self.height as usize)
            .and_then(|pixels| pixels.checked_mul(4))
            .ok_or_else(|| anyhow!("live frame dimensions overflow RGBA byte length"))
    }
}

#[derive(Debug, Default)]
struct RawFrameState {
    latest: Option<DecodedFrame>,
    error: Option<String>,
}

pub struct RawFrameReader {
    state: Arc<Mutex<RawFrameState>>,
    join: Option<JoinHandle<()>>,
}

impl RawFrameReader {
    pub fn start(mut stdout: ChildStdout, config: LiveFrameConfig) -> Result<Self> {
        let byte_len = config.byte_len()?;
        let min_publish_interval = Duration::from_secs_f64(1.0 / f64::from(config.max_fps));
        let state = Arc::new(Mutex::new(RawFrameState::default()));
        let thread_state = Arc::clone(&state);
        let join = thread::spawn(move || {
            let mut frame_index = 0_u64;
            let mut published_index = 0_u64;
            let mut last_publish_at = None::<Instant>;
            let mut rgba = vec![0_u8; byte_len];
            loop {
                match stdout.read_exact(&mut rgba) {
                    Ok(()) => {
                        frame_index = frame_index.saturating_add(1);
                        let now = Instant::now();
                        let should_publish = last_publish_at
                            .is_none_or(|last| now.duration_since(last) >= min_publish_interval);
                        if !should_publish {
                            continue;
                        }
                        published_index = published_index.saturating_add(1);
                        last_publish_at = Some(now);
                        if published_index == 1 || published_index % 30 == 0 {
                            append_direct_debug_line(&format!(
                                "raw video reader published frame {published_index} from raw frame {frame_index}"
                            ));
                        }
                        let frame = DecodedFrame {
                            frame_index: published_index,
                            width: config.width,
                            height: config.height,
                            rgba: rgba.clone(),
                        };
                        if let Ok(mut state) = thread_state.lock() {
                            state.latest = Some(frame);
                        } else {
                            break;
                        }
                    }
                    Err(err)
                        if matches!(
                            err.kind(),
                            ErrorKind::UnexpectedEof
                                | ErrorKind::BrokenPipe
                                | ErrorKind::ConnectionReset
                        ) =>
                    {
                        append_direct_debug_line("raw video pipe closed");
                        if let Ok(mut state) = thread_state.lock() {
                            state.error = Some("raw video pipe closed".into());
                        }
                        break;
                    }
                    Err(err) => {
                        append_direct_debug_line(&format!("raw video pipe read failed: {err}"));
                        if let Ok(mut state) = thread_state.lock() {
                            state.error = Some(format!("raw video pipe read failed: {err}"));
                        }
                        break;
                    }
                }
            }
        });

        Ok(Self {
            state,
            join: Some(join),
        })
    }

    pub fn take_latest_after(&self, last_frame_index: u64) -> Result<Option<DecodedFrame>> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| anyhow!("raw video reader state poisoned"))?;
        if state
            .latest
            .as_ref()
            .is_some_and(|frame| frame.frame_index > last_frame_index)
        {
            return Ok(state.latest.take());
        }
        if let Some(error) = state.error.as_ref() {
            return Err(anyhow!(error.clone()));
        }
        Ok(None)
    }

    pub fn join(&mut self) {
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

pub fn receiver_args(port: u16, config: LiveFrameConfig) -> Vec<String> {
    let raw_caps = format!(
        "video/x-raw,format=RGBA,width={},height={},pixel-aspect-ratio=1/1",
        config.width, config.height
    );
    vec![
        "-q".into(),
        "udpsrc".into(),
        format!("port={port}"),
        format!("caps={VIDEO_RTP_CAPS}"),
        "!".into(),
        "rtpjitterbuffer".into(),
        format!("latency={RTP_JITTERBUFFER_LATENCY_MS}"),
        "drop-on-latency=true".into(),
        "do-lost=true".into(),
        "mode=none".into(),
        "!".into(),
        "rtph264depay".into(),
        "!".into(),
        "h264parse".into(),
        "!".into(),
        "decodebin".into(),
        "!".into(),
        "queue".into(),
        "leaky=downstream".into(),
        "max-size-buffers=1".into(),
        "max-size-bytes=0".into(),
        "max-size-time=0".into(),
        "!".into(),
        "videoconvert".into(),
        "!".into(),
        "videoscale".into(),
        "add-borders=true".into(),
        "!".into(),
        raw_caps,
        "!".into(),
        "queue".into(),
        "leaky=downstream".into(),
        "max-size-buffers=1".into(),
        "max-size-bytes=0".into(),
        "max-size-time=0".into(),
        "!".into(),
        "fdsink".into(),
        "fd=1".into(),
        "sync=false".into(),
        "async=false".into(),
    ]
}

fn env_u32(name: &str, default: u32) -> u32 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse::<u32>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(default)
}

fn append_direct_debug_line(line: &str) {
    if std::env::var_os("IOS_CONTROL_DIRECT_DEBUG_LOG").is_none() {
        return;
    }
    let path = std::env::temp_dir().join(format!(
        "ios-control-capture-direct-{}.log",
        std::process::id()
    ));
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        use std::io::Write;
        let _ = writeln!(file, "{line}");
    }
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
        frames.push((index, path));
    }
    frames.sort_unstable_by(|left, right| right.0.cmp(&left.0));
    for (_, path) in frames.iter().skip(DIAGNOSTIC_FRAME_LIMIT) {
        let _ = std::fs::remove_file(path);
    }
    frames.truncate(DIAGNOSTIC_FRAME_LIMIT);
    Ok(frames
        .into_iter()
        .filter(|(index, _)| *index > last_index)
        .collect())
}

fn decode_frame(path: &Path, frame_index: u64) -> image::ImageResult<DecodedFrame> {
    let image = image::open(path)?.to_rgba8();
    let width = image.width();
    let height = image.height();
    Ok(DecodedFrame {
        frame_index,
        width,
        height,
        rgba: image.into_raw(),
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
    use super::LiveFrameConfig;
    use super::{next_frame, receiver_args};
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
        assert_eq!(frame.width, 1);
        assert_eq!(frame.height, 1);
        assert_eq!(last_index, 1);
        assert!(!path.exists());
    }

    #[test]
    fn next_frame_preserves_source_png_dimensions() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("frame-000000001.png");
        let image = RgbaImage::from_pixel(608, 1080, Rgba([0, 0, 255, 255]));
        image.save(&path).unwrap();

        let mut last_index = 0;
        let frame = next_frame(dir.path(), &mut last_index)
            .unwrap()
            .expect("completed frame should decode");

        assert_eq!(frame.width, 608);
        assert_eq!(frame.height, 1080);
        assert_eq!(frame.rgba.len(), 608 * 1080 * 4);
    }

    #[test]
    fn receiver_args_stream_raw_rgba_to_stdout_without_disk_frames() {
        let args = receiver_args(
            12345,
            LiveFrameConfig {
                width: 1080,
                height: 1920,
                max_fps: 20,
            },
        );

        assert!(args.iter().any(|arg| arg == "fdsink"));
        assert!(args.iter().any(|arg| arg == "fd=1"));
        assert!(args.iter().any(|arg| arg == "sync=false"));
        assert!(args.iter().any(|arg| arg.contains("video/x-raw")));
        assert!(args.iter().any(|arg| arg.contains("format=RGBA")));
        assert!(args.iter().any(|arg| arg.contains("width=1080")));
        assert!(args.iter().any(|arg| arg.contains("height=1920")));
        assert!(!args.iter().any(|arg| arg == "videorate"));
        assert!(!args.iter().any(|arg| arg == "drop-only=true"));
        assert!(!args.iter().any(|arg| arg.starts_with("max-rate=")));
        assert!(!args.iter().any(|arg| arg.contains("framerate=")));
        assert!(!args.iter().any(|arg| arg == "autovideosink"));
        assert!(!args.iter().any(|arg| arg == "pngenc"));
        assert!(!args.iter().any(|arg| arg == "multifilesink"));
        assert!(!args.iter().any(|arg| arg.starts_with("location=")));
        assert!(args.iter().any(|arg| arg == "rtpjitterbuffer"));
        assert!(args.iter().any(|arg| arg == "latency=35"));
        assert!(args.iter().any(|arg| arg == "drop-on-latency=true"));
        assert!(args.iter().any(|arg| arg == "do-lost=true"));
        assert!(args.iter().any(|arg| arg == "leaky=downstream"));
        assert!(args.iter().any(|arg| arg == "max-size-buffers=1"));
        assert!(args.iter().any(|arg| arg == "rtph264depay"));
        assert!(!args.iter().any(|arg| arg == "request-keyframe=true"));
        assert!(!args.iter().any(|arg| arg == "wait-for-keyframe=true"));
        assert!(args.iter().any(|arg| arg == "decodebin"));
        assert!(args.iter().any(|arg| arg.contains("encoding-name=H264")));
        let jitter_index = args
            .iter()
            .position(|arg| arg == "rtpjitterbuffer")
            .expect("video receiver should buffer RTP before depayloading");
        let depay_index = args
            .iter()
            .position(|arg| arg == "rtph264depay")
            .expect("video receiver should depayload H.264 RTP");
        let decode_index = args
            .iter()
            .position(|arg| arg == "decodebin")
            .expect("video receiver should decode RTP to raw video");
        assert!(jitter_index < depay_index);
        assert!(jitter_index < decode_index);
    }

    #[test]
    fn next_frame_prunes_diagnostic_frames_to_latest_limit() {
        let dir = tempfile::tempdir().unwrap();
        let png = sample_png_bytes();
        for index in 1..=505 {
            std::fs::write(dir.path().join(format!("frame-{index:09}.png")), &png).unwrap();
        }

        let mut last_index = 0;
        let frame = next_frame(dir.path(), &mut last_index)
            .unwrap()
            .expect("newest frame should decode");

        assert_eq!(frame.frame_index, 505);
        assert!(!dir.path().join("frame-000000001.png").exists());
        let retained = std::fs::read_dir(dir.path()).unwrap().count();
        assert_eq!(retained, 499);
    }
}
