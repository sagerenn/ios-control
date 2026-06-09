use anyhow::{anyhow, Context, Result};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};

const RUNTIME_ROOT_ENV: &str = "IOS_CONTROL_DIRECT_RUNTIME_ROOT";
const BLE_PATH_ENV: &str = "IOS_CONTROL_DIRECT_BLE_PATH";

#[derive(Debug, Serialize)]
struct ProbeResponse {
    supported: bool,
    script_path: String,
    python_command: String,
}

fn main() -> Result<()> {
    match std::env::args().nth(1).as_deref() {
        Some("probe") => {
            let bundle = BeaconBundle::resolve()?;
            let payload = ProbeResponse {
                supported: true,
                script_path: bundle.script_path.display().to_string(),
                python_command: bundle.python_command.clone(),
            };
            println!("{}", serde_json::to_string(&payload)?);
            Ok(())
        }
        Some("serve") => {
            let bundle = BeaconBundle::resolve()?;
            let ble_path = std::env::var_os(BLE_PATH_ENV)
                .map(PathBuf::from)
                .ok_or_else(|| anyhow!("{BLE_PATH_ENV} not configured"))?;
            let status = bundle.run(&ble_path)?;
            if status.success() {
                Ok(())
            } else {
                Err(anyhow!("direct beacon exited with status {status}"))
            }
        }
        Some(other) => Err(anyhow!("unsupported direct-beacon command: {other}")),
        None => Err(anyhow!("missing direct-beacon command")),
    }
}

#[derive(Debug, Clone)]
struct BeaconBundle {
    runtime_root: PathBuf,
    script_path: PathBuf,
    python_command: String,
}

impl BeaconBundle {
    fn resolve() -> Result<Self> {
        let runtime_root = std::env::var_os(RUNTIME_ROOT_ENV)
            .map(PathBuf::from)
            .ok_or_else(|| anyhow!("{RUNTIME_ROOT_ENV} not configured"))?;
        let script_path = runtime_root
            .join("Bluetooth_LE_beacon")
            .join("uxplay-beacon.py");
        if !script_path.is_file() {
            return Err(anyhow!(
                "direct beacon script missing: {}",
                script_path.display()
            ));
        }

        let python_command = resolve_python_command()
            .ok_or_else(|| anyhow!("python interpreter unavailable for direct-beacon"))?;

        Ok(Self {
            runtime_root,
            script_path,
            python_command,
        })
    }

    fn run(&self, ble_path: &Path) -> Result<ExitStatus> {
        let mut command = python_command(&self.python_command);
        hide_child_console(&mut command);
        command
            .arg(&self.script_path)
            .arg("--path")
            .arg(ble_path)
            .current_dir(&self.runtime_root)
            .stdin(std::process::Stdio::null());
        command.status().context("failed to launch beacon script")
    }
}

fn resolve_python_command() -> Option<String> {
    for candidate in python_candidates() {
        let mut command = python_command(candidate);
        hide_child_console(&mut command);
        if command
            .arg("--version")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
        {
            return Some(candidate.to_string());
        }
    }
    None
}

fn python_candidates() -> &'static [&'static str] {
    if cfg!(target_os = "windows") {
        &["py", "python", "python3"]
    } else {
        &["python3", "python"]
    }
}

fn python_command(command: &str) -> Command {
    if cfg!(target_os = "windows") && command == "py" {
        let mut cmd = Command::new("py");
        cmd.arg("-3");
        return cmd;
    }
    Command::new(command)
}

fn hide_child_console(command: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
}
