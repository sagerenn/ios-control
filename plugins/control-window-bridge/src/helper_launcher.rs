use std::env;
use std::io;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

pub fn find_helper() -> Option<PathBuf> {
    std::env::var_os("IOS_CONTROL_WINDOW_INPUT_HELPER")
        .map(PathBuf::from)
        .filter(|path| path.is_file())
}

pub fn helper_available(path: Option<PathBuf>) -> bool {
    path.as_ref().is_some_and(|path| helper_is_executable(path))
}

pub fn helper_is_executable(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(path)
            .map(|metadata| metadata.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        true
    }
}

const DEFAULT_TIMEOUT_MS: u64 = 2_000;
const POLL_INTERVAL_MS: u64 = 10;

fn helper_timeout() -> Duration {
    let from_env = env::var("IOS_CONTROL_WINDOW_INPUT_HELPER_TIMEOUT_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_TIMEOUT_MS);
    Duration::from_millis(from_env)
}

fn wait_for_completion(child: &mut Child, timeout: Duration) -> io::Result<ExitStatus> {
    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(status);
        }
        if start.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                format!(
                    "window input helper timed out after {}ms",
                    timeout.as_millis()
                ),
            ));
        }
        thread::sleep(Duration::from_millis(POLL_INTERVAL_MS));
    }
}

pub fn launch_helper(helper: PathBuf, args: &[String]) -> std::io::Result<ExitStatus> {
    let mut child = Command::new(helper)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    wait_for_completion(&mut child, helper_timeout())
}

pub fn should_run_embedded_helper_mode(args: &[String]) -> bool {
    !args.is_empty() && args.iter().any(|arg| arg == "--source")
}

pub fn run_embedded_helper_mode(args: &[String]) -> io::Result<()> {
    let mut source = None::<String>;
    let mut action = None::<&'static str>;
    let mut index = 0usize;

    while index < args.len() {
        match args[index].as_str() {
            "--source" => {
                let value = args.get(index + 1).ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidInput, "missing value for --source")
                })?;
                source = Some(value.clone());
                index += 2;
            }
            "--pointer-plan" => {
                action = Some("pointer");
                index += 1;
            }
            "--keyboard-plan" => {
                action = Some("keyboard");
                index += 1;
            }
            "--hybrid-plan" => {
                action = Some("hybrid");
                index += 1;
            }
            _ => {
                index += 1;
            }
        }
    }

    let source = source.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "embedded helper mode requires --source <id>",
        )
    })?;
    let action = action.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "embedded helper mode requires one plan action flag",
        )
    })?;

    if let Some(path) = env::var_os("IOS_CONTROL_WINDOW_INPUT_HELPER_ACTION_LOG") {
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;
        writeln!(file, "source={source} action={action}")?;
    }

    Ok(())
}
