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
        #[cfg(target_os = "windows")]
        "serve" => {
            let state_file = parse_state_file(args)?;
            ble_helper::windows_hid::serve(state_file)?;
        }
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
            #[cfg(target_os = "windows")]
            if state.phase != "Unavailable" {
                let paths = ble_helper::windows_hid::HidPaths::new(state_path()?)?;
                if let Err(err) = ble_helper::windows_hid::start_server_if_needed(&paths) {
                    let mut error_state = state.clone();
                    error_state.phase = "Error".into();
                    error_state.execute_ready = false;
                    error_state.notes =
                        vec![format!("failed to start BLE HID mouse server: {err}")];
                    save_state(&error_state)?;
                    print_state(&error_state)?;
                    return Ok(());
                }

                std::thread::sleep(std::time::Duration::from_millis(300));
                let refreshed = derive_runtime_state(load_state()?, false);
                print_state(&refreshed)?;
                return Ok(());
            }
            print_state(&state)?;
        }
        "status" => {
            let state = derive_runtime_state(load_state()?, false);
            print_state(&state)?;
        }
        #[cfg(target_os = "windows")]
        "mouse" => {
            let mouse = parse_mouse_command(args)?;
            execute_mouse_command(mouse)?;
        }
        #[cfg(not(target_os = "windows"))]
        "mouse" => return Err(anyhow!("mouse command is supported only on Windows")),
        #[cfg(target_os = "windows")]
        "click" => {
            let mouse = parse_click_command(args)?;
            execute_mouse_command(mouse)?;
        }
        #[cfg(not(target_os = "windows"))]
        "click" => return Err(anyhow!("click command is supported only on Windows")),
        #[cfg(target_os = "windows")]
        "open-safari" => {
            execute_keyboard_command(open_safari_keyboard_command())?;
        }
        #[cfg(not(target_os = "windows"))]
        "open-safari" => return Err(anyhow!("open-safari command is supported only on Windows")),
        "execute" => {
            let plan_kind = parse_plan_kind(args)?;
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
            #[cfg(target_os = "windows")]
            {
                let paths = ble_helper::windows_hid::HidPaths::new(state_path()?)?;
                match ble_helper::windows_hid::execute_pointer(&paths, &plan_kind) {
                    Ok(ack) if ack.ok => {
                        println!(
                            "{}",
                            serde_json::to_string(&json!({
                                "phase": "Succeeded",
                                "summary": ack.message,
                                "observed_change": true
                            }))?
                        );
                    }
                    Ok(ack) => {
                        println!(
                            "{}",
                            serde_json::to_string(&json!({
                                "phase": "Failed",
                                "summary": "ble helper execution failed",
                                "failure_reason": ack.message,
                                "observed_change": false
                            }))?
                        );
                    }
                    Err(err) => {
                        println!(
                            "{}",
                            serde_json::to_string(&json!({
                                "phase": "Failed",
                                "summary": "ble helper execution failed",
                                "failure_reason": err.to_string(),
                                "observed_change": false
                            }))?
                        );
                    }
                }
                return Ok(());
            }
            #[cfg(not(target_os = "windows"))]
            {
                println!(
                    "{}",
                    serde_json::to_string(&json!({
                        "phase": "Succeeded",
                        "summary": "ble helper executed plan",
                        "observed_change": true
                    }))?
                );
            }
        }
        "stop" => {
            #[cfg(target_os = "windows")]
            {
                let paths = ble_helper::windows_hid::HidPaths::new(state_path()?)?;
                let _ = ble_helper::windows_hid::stop_server(&paths);
            }
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

fn parse_plan_kind(mut args: impl Iterator<Item = String>) -> Result<String> {
    let mut plan_kind = "pointer".to_string();
    while let Some(arg) = args.next() {
        if arg == "--plan-kind" {
            if let Some(value) = args.next() {
                plan_kind = value;
            }
        }
    }
    Ok(plan_kind)
}

#[cfg(target_os = "windows")]
fn parse_mouse_command(
    mut args: impl Iterator<Item = String>,
) -> Result<ble_helper::windows_hid::MouseCommand> {
    let mut dx = 0i8;
    let mut dy = 0i8;
    let mut wheel = 0i8;
    let mut buttons = 0u8;
    let mut repeat = 1u16;
    let mut delay_ms = 15u64;
    let mut release = true;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--dx" => dx = parse_i8_arg("--dx", args.next())?,
            "--dy" => dy = parse_i8_arg("--dy", args.next())?,
            "--wheel" => wheel = parse_i8_arg("--wheel", args.next())?,
            "--buttons" => buttons = parse_buttons_arg(args.next())?,
            "--repeat" => repeat = parse_repeat_arg(args.next())?,
            "--delay-ms" => delay_ms = parse_delay_arg(args.next())?,
            "--no-release" => release = false,
            _ => return Err(anyhow!("unsupported mouse option: {arg}")),
        }
    }

    if buttons == 0 && dx == 0 && dy == 0 && wheel == 0 {
        return Err(anyhow!(
            "mouse command needs at least one of --dx, --dy, --wheel, or --buttons"
        ));
    }

    let mut reports = vec![ble_helper::windows_hid::MouseReport {
        buttons,
        dx,
        dy,
        wheel,
        repeat,
    }];
    if release && buttons != 0 {
        reports.push(ble_helper::windows_hid::MouseReport {
            buttons: 0,
            dx: 0,
            dy: 0,
            wheel: 0,
            repeat: 1,
        });
    }

    Ok(ble_helper::windows_hid::MouseCommand { reports, delay_ms })
}

#[cfg(target_os = "windows")]
fn parse_click_command(
    mut args: impl Iterator<Item = String>,
) -> Result<ble_helper::windows_hid::MouseCommand> {
    let mut button = 1u8;
    let mut repeat = 1u16;
    let mut delay_ms = 40u64;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--button" => button = parse_button_name(args.next())?,
            "--buttons" => button = parse_buttons_arg(args.next())?,
            "--repeat" => repeat = parse_repeat_arg(args.next())?,
            "--delay-ms" => delay_ms = parse_delay_arg(args.next())?,
            _ => return Err(anyhow!("unsupported click option: {arg}")),
        }
    }
    if button == 0 {
        return Err(anyhow!("click button mask must not be zero"));
    }

    let mut reports = Vec::new();
    for _ in 0..repeat {
        reports.push(ble_helper::windows_hid::MouseReport {
            buttons: button,
            dx: 0,
            dy: 0,
            wheel: 0,
            repeat: 1,
        });
        reports.push(ble_helper::windows_hid::MouseReport {
            buttons: 0,
            dx: 0,
            dy: 0,
            wheel: 0,
            repeat: 1,
        });
    }

    Ok(ble_helper::windows_hid::MouseCommand { reports, delay_ms })
}

#[cfg(target_os = "windows")]
fn execute_mouse_command(mouse: ble_helper::windows_hid::MouseCommand) -> Result<()> {
    let state = derive_runtime_state(load_state()?, false);
    if !state.execute_ready {
        println!(
            "{}",
            serde_json::to_string(&json!({
                "phase": "Failed",
                "summary": "ble helper not connected",
                "failure_reason": "BLE helper mouse command requested while device is not connected",
                "observed_change": false
            }))?
        );
        return Ok(());
    }

    let paths = ble_helper::windows_hid::HidPaths::new(state_path()?)?;
    match ble_helper::windows_hid::execute_mouse(&paths, mouse) {
        Ok(ack) if ack.ok => {
            println!(
                "{}",
                serde_json::to_string(&json!({
                    "phase": "Succeeded",
                    "summary": ack.message,
                    "observed_change": true
                }))?
            );
        }
        Ok(ack) => {
            println!(
                "{}",
                serde_json::to_string(&json!({
                    "phase": "Failed",
                    "summary": "ble helper mouse command failed",
                    "failure_reason": ack.message,
                    "observed_change": false
                }))?
            );
        }
        Err(err) => {
            println!(
                "{}",
                serde_json::to_string(&json!({
                    "phase": "Failed",
                    "summary": "ble helper mouse command failed",
                    "failure_reason": err.to_string(),
                    "observed_change": false
                }))?
            );
        }
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn execute_keyboard_command(keyboard: ble_helper::windows_hid::KeyboardCommand) -> Result<()> {
    let state = derive_runtime_state(load_state()?, false);
    if !state.execute_ready {
        println!(
            "{}",
            serde_json::to_string(&json!({
                "phase": "Failed",
                "summary": "ble helper not connected",
                "failure_reason": "BLE helper keyboard command requested while device is not connected",
                "observed_change": false
            }))?
        );
        return Ok(());
    }

    let paths = ble_helper::windows_hid::HidPaths::new(state_path()?)?;
    match ble_helper::windows_hid::execute_keyboard(&paths, keyboard) {
        Ok(ack) if ack.ok => {
            println!(
                "{}",
                serde_json::to_string(&json!({
                    "phase": "Succeeded",
                    "summary": ack.message,
                    "observed_change": true
                }))?
            );
        }
        Ok(ack) => {
            println!(
                "{}",
                serde_json::to_string(&json!({
                    "phase": "Failed",
                    "summary": "ble helper keyboard command failed",
                    "failure_reason": ack.message,
                    "observed_change": false
                }))?
            );
        }
        Err(err) => {
            println!(
                "{}",
                serde_json::to_string(&json!({
                    "phase": "Failed",
                    "summary": "ble helper keyboard command failed",
                    "failure_reason": err.to_string(),
                    "observed_change": false
                }))?
            );
        }
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn open_safari_keyboard_command() -> ble_helper::windows_hid::KeyboardCommand {
    const MOD_CMD: u8 = 0x08;
    const KEY_A: u8 = 0x04;
    const KEY_F: u8 = 0x09;
    const KEY_H: u8 = 0x0b;
    const KEY_I: u8 = 0x0c;
    const KEY_R: u8 = 0x15;
    const KEY_S: u8 = 0x16;
    const KEY_ENTER: u8 = 0x28;
    const KEY_SPACE: u8 = 0x2c;

    let mut reports = Vec::new();
    push_key(&mut reports, MOD_CMD, KEY_H);
    push_pause(&mut reports, 4);
    push_key(&mut reports, MOD_CMD, KEY_SPACE);
    push_pause(&mut reports, 8);
    for key in [KEY_S, KEY_A, KEY_F, KEY_A, KEY_R, KEY_I] {
        push_key(&mut reports, 0, key);
    }
    push_pause(&mut reports, 3);
    push_key(&mut reports, 0, KEY_ENTER);

    ble_helper::windows_hid::KeyboardCommand {
        reports,
        delay_ms: 80,
    }
}

#[cfg(target_os = "windows")]
fn push_key(reports: &mut Vec<ble_helper::windows_hid::KeyboardReport>, modifiers: u8, key: u8) {
    reports.push(ble_helper::windows_hid::KeyboardReport {
        modifiers,
        keys: vec![key],
        repeat: 1,
    });
    push_pause(reports, 1);
}

#[cfg(target_os = "windows")]
fn push_pause(reports: &mut Vec<ble_helper::windows_hid::KeyboardReport>, repeat: u16) {
    reports.push(ble_helper::windows_hid::KeyboardReport {
        modifiers: 0,
        keys: Vec::new(),
        repeat,
    });
}

#[cfg(target_os = "windows")]
fn parse_i8_arg(name: &str, value: Option<String>) -> Result<i8> {
    let value = value.ok_or_else(|| anyhow!("{name} requires a value"))?;
    value
        .parse::<i8>()
        .map_err(|_| anyhow!("{name} must be an integer from -128 to 127"))
}

#[cfg(target_os = "windows")]
fn parse_buttons_arg(value: Option<String>) -> Result<u8> {
    let value = value.ok_or_else(|| anyhow!("--buttons requires a value"))?;
    let buttons = value
        .parse::<u8>()
        .map_err(|_| anyhow!("--buttons must be an integer from 0 to 7"))?;
    if buttons > 7 {
        return Err(anyhow!("--buttons must be an integer from 0 to 7"));
    }
    Ok(buttons)
}

#[cfg(target_os = "windows")]
fn parse_button_name(value: Option<String>) -> Result<u8> {
    let value = value.ok_or_else(|| anyhow!("--button requires a value"))?;
    match value.as_str() {
        "left" => Ok(1),
        "right" => Ok(2),
        "middle" => Ok(4),
        _ => parse_buttons_arg(Some(value)),
    }
}

#[cfg(target_os = "windows")]
fn parse_repeat_arg(value: Option<String>) -> Result<u16> {
    let value = value.ok_or_else(|| anyhow!("--repeat requires a value"))?;
    let repeat = value
        .parse::<u16>()
        .map_err(|_| anyhow!("--repeat must be an integer from 1 to 1000"))?;
    if repeat == 0 || repeat > 1000 {
        return Err(anyhow!("--repeat must be an integer from 1 to 1000"));
    }
    Ok(repeat)
}

#[cfg(target_os = "windows")]
fn parse_delay_arg(value: Option<String>) -> Result<u64> {
    let value = value.ok_or_else(|| anyhow!("--delay-ms requires a value"))?;
    let delay_ms = value
        .parse::<u64>()
        .map_err(|_| anyhow!("--delay-ms must be an integer from 0 to 1000"))?;
    if delay_ms > 1000 {
        return Err(anyhow!("--delay-ms must be an integer from 0 to 1000"));
    }
    Ok(delay_ms)
}

#[cfg(target_os = "windows")]
fn parse_state_file(mut args: impl Iterator<Item = String>) -> Result<PathBuf> {
    while let Some(arg) = args.next() {
        if arg == "--state-file" {
            if let Some(value) = args.next() {
                return Ok(PathBuf::from(value));
            }
        }
    }
    state_path()
}

fn effective_host_capability() -> HostCapability {
    let capability = probe_host_capability();
    match std::env::var("IOS_CONTROL_BLE_HELPER_SUPPORTED")
        .ok()
        .as_deref()
    {
        Some("0") => HostCapability::unsupported(
            capability
                .reason
                .unwrap_or_else(|| "BLE helper disabled by environment".into()),
        ),
        _ => capability,
    }
}

fn derive_runtime_state(mut persisted: HelperState, allow_auto_pair: bool) -> HelperState {
    let persisted_snapshot = persisted.clone();
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

    if matches!(
        persisted_snapshot.phase.as_str(),
        "Advertising" | "Pairing" | "Connected" | "Error"
    ) {
        return persisted_snapshot;
    }

    if allow_auto_pair
        && std::env::var("IOS_CONTROL_BLE_HELPER_AUTO_PAIR")
            .ok()
            .as_deref()
            == Some("1")
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
