use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct HelperState {
    phase: String,
    paired_device_id: Option<String>,
    paired_device_name: Option<String>,
    bonded: bool,
    execute_ready: bool,
}

impl Default for HelperState {
    fn default() -> Self {
        Self {
            phase: "Advertising".into(),
            paired_device_id: None,
            paired_device_name: None,
            bonded: false,
            execute_ready: false,
        }
    }
}

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let Some(command) = args.next() else {
        return Err(anyhow!("missing helper command"));
    };

    match command.as_str() {
        "probe" => {
            println!(
                "{}",
                serde_json::to_string(&json!({
                    "supported": helper_supported(),
                    "supports_prepare": true,
                    "supports_execute": true,
                    "supports_status": true,
                    "supports_stop": true,
                    "supports_forget_bond": true
                }))?
            );
        }
        "prepare" => {
            let mut state = load_state()?;
            if let Ok(phase) = std::env::var("IOS_CONTROL_BLE_HELPER_FORCE_PHASE") {
                state.phase = phase;
            } else if std::env::var("IOS_CONTROL_BLE_HELPER_AUTO_PAIR").ok().as_deref() == Some("1")
            {
                state.phase = "Connected".into();
                state.bonded = true;
                state.execute_ready = true;
                if state.paired_device_id.is_none() {
                    state.paired_device_id = Some("device-ble-1".into());
                }
                if state.paired_device_name.is_none() {
                    state.paired_device_name = Some("Paired iPhone".into());
                }
            } else if state.bonded {
                state.phase = "Connected".into();
                state.execute_ready = true;
            } else {
                state.phase = "Advertising".into();
                state.execute_ready = false;
            }
            save_state(&state)?;
            print_state(&state)?;
        }
        "status" => {
            let state = load_state()?;
            print_state(&state)?;
        }
        "execute" => {
            let state = load_state()?;
            if !state.execute_ready {
                println!(
                    "{}",
                    serde_json::to_string(&json!({
                        "phase": "Failed",
                        "summary": "ble helper not connected",
                        "failure_reason": "BLE helper execute requested while device is not connected",
                        "observed_change": false
                    }))?
                );
                return Ok(());
            }
            println!(
                "{}",
                serde_json::to_string(&json!({
                    "phase": "Succeeded",
                    "summary": "ble helper executed plan",
                    "observed_change": true
                }))?
            );
        }
        "stop" => {
            let mut state = load_state()?;
            state.phase = if state.bonded {
                "BondedIdle".into()
            } else {
                "ReadyToAdvertise".into()
            };
            state.execute_ready = false;
            save_state(&state)?;
            println!(
                "{}",
                serde_json::to_string(&json!({
                    "ok": true,
                    "message": "helper stopped"
                }))?
            );
        }
        "forget-bond" => {
            let _ = args.next();
            let _ = args.next();
            let state = HelperState::default();
            save_state(&state)?;
            println!(
                "{}",
                serde_json::to_string(&json!({
                    "ok": true,
                    "message": "bond forgotten"
                }))?
            );
        }
        _ => return Err(anyhow!("unsupported helper command: {command}")),
    }

    Ok(())
}

fn helper_supported() -> bool {
    std::env::var("IOS_CONTROL_BLE_HELPER_SUPPORTED")
        .ok()
        .map(|value| value != "0")
        .unwrap_or(cfg!(any(target_os = "linux", target_os = "windows")))
}

fn print_state(state: &HelperState) -> Result<()> {
    println!(
        "{}",
        serde_json::to_string(&json!({
            "phase": state.phase,
            "checklist": if state.bonded {
                vec!["Reconnect the paired device"]
            } else {
                vec!["Enable Bluetooth", "Pair the device when it appears"]
            },
            "notes": if state.bonded {
                vec!["Stored bond available"]
            } else {
                vec!["Waiting for first-time pairing"]
            },
            "paired_device_id": state.paired_device_id,
            "paired_device_name": state.paired_device_name,
            "bonded": state.bonded,
            "execute_ready": state.execute_ready
        }))?
    );
    Ok(())
}

fn load_state() -> Result<HelperState> {
    let path = state_path()?;
    match fs::read_to_string(&path) {
        Ok(text) => Ok(serde_json::from_str(&text)?),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(HelperState::default()),
        Err(err) => Err(err.into()),
    }
}

fn save_state(state: &HelperState) -> Result<()> {
    let path = state_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_string_pretty(state)?)?;
    Ok(())
}

fn state_path() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("IOS_CONTROL_BLE_HELPER_STATE_DIR") {
        return Ok(PathBuf::from(path).join("ble-helper-state.json"));
    }
    if let Some(appdata) = std::env::var_os("APPDATA") {
        return Ok(PathBuf::from(appdata)
            .join("ios-control")
            .join("ble-helper-state.json"));
    }
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
        return Ok(PathBuf::from(xdg)
            .join("ios-control")
            .join("ble-helper-state.json"));
    }
    if let Some(home) = std::env::var_os("HOME") {
        return Ok(Path::new(&home)
            .join(".config")
            .join("ios-control")
            .join("ble-helper-state.json"));
    }
    Ok(std::env::temp_dir().join("ble-helper-state.json"))
}
