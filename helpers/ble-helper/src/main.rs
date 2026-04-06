use anyhow::{anyhow, Result};
use ble_helper::{
    backend::HostCapability,
    probe_host_capability,
    state::{helper_state_from_capability, HelperState},
};
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let Some(command) = args.next() else {
        return Err(anyhow!("missing helper command"));
    };

    match command.as_str() {
        "probe" => {
            let capability = effective_host_capability();
            println!(
                "{}",
                serde_json::to_string(&json!({
                    "supported": capability.supported,
                    "reason": capability.reason,
                    "backend": capability.backend,
                    "supports_prepare": true,
                    "supports_execute": true,
                    "supports_status": true,
                    "supports_stop": true,
                    "supports_forget_bond": true
                }))?
            );
        }
        "prepare" => {
            let persisted = load_state()?;
            let state = derive_runtime_state(persisted, true);
            save_state(&state)?;
            print_state(&state)?;
        }
        "status" => {
            let state = derive_runtime_state(load_state()?, false);
            print_state(&state)?;
        }
        "execute" => {
            let state = derive_runtime_state(load_state()?, false);
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
            let mut state = derive_runtime_state(load_state()?, false);
            if state.phase == "Connected" {
                state.phase = if state.bonded {
                    "BondedIdle".into()
                } else {
                    "ReadyToAdvertise".into()
                };
            }
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

fn effective_host_capability() -> HostCapability {
    let capability = probe_host_capability();
    match std::env::var("IOS_CONTROL_BLE_HELPER_SUPPORTED").ok().as_deref() {
        Some("0") => HostCapability::unsupported(
            capability
                .reason
                .unwrap_or_else(|| "BLE helper disabled by environment".into()),
        ),
        _ => capability,
    }
}

fn derive_runtime_state(mut persisted: HelperState, allow_auto_pair: bool) -> HelperState {
    let capability = effective_host_capability();
    let mut state = helper_state_from_capability(&capability, persisted.bonded);
    state.paired_device_id = persisted.paired_device_id.take();
    state.paired_device_name = persisted.paired_device_name.take();

    if let Ok(phase) = std::env::var("IOS_CONTROL_BLE_HELPER_FORCE_PHASE") {
        state.phase = phase;
        return state;
    }

    if !capability.supported {
        return state;
    }

    if allow_auto_pair && std::env::var("IOS_CONTROL_BLE_HELPER_AUTO_PAIR").ok().as_deref() == Some("1")
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
        state.notes = vec!["Stored bond available".into()];
        return state;
    }

    if persisted.execute_ready && persisted.bonded {
        state.phase = "Connected".into();
        state.execute_ready = true;
        state.notes = vec!["Stored bond available".into()];
    }

    state
}

fn print_state(state: &HelperState) -> Result<()> {
    println!("{}", serde_json::to_string(state)?);
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
