use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use windows::core::{GUID, HSTRING};
use windows::Devices::Bluetooth::BluetoothError;
use windows::Devices::Bluetooth::GenericAttributeProfile::{
    GattCharacteristicProperties, GattLocalCharacteristic, GattLocalCharacteristicParameters,
    GattLocalDescriptorParameters, GattLocalService, GattProtectionLevel, GattServiceProvider,
    GattServiceProviderAdvertisementStatus, GattServiceProviderAdvertisingParameters,
};
use windows::Foundation::TypedEventHandler;
use windows::Storage::Streams::{DataWriter, IBuffer};

const CREATE_NO_WINDOW: u32 = 0x08000000;
const HID_SERVICE_UUID: GUID = GUID::from_u128(0x00001812_0000_1000_8000_00805f9b34fb);
const BATTERY_SERVICE_UUID: GUID = GUID::from_u128(0x0000180f_0000_1000_8000_00805f9b34fb);
const BATTERY_LEVEL_UUID: GUID = GUID::from_u128(0x00002a19_0000_1000_8000_00805f9b34fb);
const HID_INFORMATION_UUID: GUID = GUID::from_u128(0x00002a4a_0000_1000_8000_00805f9b34fb);
const REPORT_MAP_UUID: GUID = GUID::from_u128(0x00002a4b_0000_1000_8000_00805f9b34fb);
const HID_CONTROL_POINT_UUID: GUID = GUID::from_u128(0x00002a4c_0000_1000_8000_00805f9b34fb);
const REPORT_UUID: GUID = GUID::from_u128(0x00002a4d_0000_1000_8000_00805f9b34fb);
const PROTOCOL_MODE_UUID: GUID = GUID::from_u128(0x00002a4e_0000_1000_8000_00805f9b34fb);
const REPORT_REFERENCE_DESCRIPTOR_UUID: GUID =
    GUID::from_u128(0x00002908_0000_1000_8000_00805f9b34fb);

const REPORT_TYPE_INPUT: u8 = 1;
const MOUSE_REPORT_ID: u8 = 1;
const KEYBOARD_REPORT_ID: u8 = 2;

const HID_INFORMATION: [u8; 4] = [0x11, 0x01, 0x00, 0x02];
const PROTOCOL_MODE_REPORT: [u8; 1] = [0x01];
const BATTERY_LEVEL: [u8; 1] = [100];
const EMPTY_MOUSE_REPORT: [u8; 4] = [0, 0, 0, 0];
const EMPTY_KEYBOARD_REPORT: [u8; 8] = [0, 0, 0, 0, 0, 0, 0, 0];
const COMMAND_POLL_INTERVAL: Duration = Duration::from_millis(10);

const HID_REPORT_MAP: &[u8] = &[
    0x05,
    0x01, // Usage Page (Generic Desktop)
    0x09,
    0x02, // Usage (Mouse)
    0xa1,
    0x01, // Collection (Application)
    0x85,
    MOUSE_REPORT_ID, //   Report ID
    0x09,
    0x01, //   Usage (Pointer)
    0xa1,
    0x00, //   Collection (Physical)
    0x05,
    0x09, //     Usage Page (Buttons)
    0x19,
    0x01, //     Usage Minimum (1)
    0x29,
    0x03, //     Usage Maximum (3)
    0x15,
    0x00, //     Logical Minimum (0)
    0x25,
    0x01, //     Logical Maximum (1)
    0x95,
    0x03, //     Report Count (3)
    0x75,
    0x01, //     Report Size (1)
    0x81,
    0x02, //     Input (Data, Variable, Absolute)
    0x95,
    0x01, //     Report Count (1)
    0x75,
    0x05, //     Report Size (5)
    0x81,
    0x03, //     Input (Constant)
    0x05,
    0x01, //     Usage Page (Generic Desktop)
    0x09,
    0x30, //     Usage (X)
    0x09,
    0x31, //     Usage (Y)
    0x09,
    0x38, //     Usage (Wheel)
    0x15,
    0x81, //     Logical Minimum (-127)
    0x25,
    0x7f, //     Logical Maximum (127)
    0x75,
    0x08, //     Report Size (8)
    0x95,
    0x03, //     Report Count (3)
    0x81,
    0x06, //     Input (Data, Variable, Relative)
    0xc0, //   End Collection
    0xc0, // End Collection
    0x05,
    0x01, // Usage Page (Generic Desktop)
    0x09,
    0x06, // Usage (Keyboard)
    0xa1,
    0x01, // Collection (Application)
    0x85,
    KEYBOARD_REPORT_ID, //   Report ID
    0x05,
    0x07, //   Usage Page (Keyboard/Keypad)
    0x19,
    0xe0, //   Usage Minimum (Left Control)
    0x29,
    0xe7, //   Usage Maximum (Right GUI)
    0x15,
    0x00, //   Logical Minimum (0)
    0x25,
    0x01, //   Logical Maximum (1)
    0x75,
    0x01, //   Report Size (1)
    0x95,
    0x08, //   Report Count (8)
    0x81,
    0x02, //   Input (Data, Variable, Absolute)
    0x95,
    0x01, //   Report Count (1)
    0x75,
    0x08, //   Report Size (8)
    0x81,
    0x03, //   Input (Constant)
    0x95,
    0x06, //   Report Count (6)
    0x75,
    0x08, //   Report Size (8)
    0x15,
    0x00, //   Logical Minimum (0)
    0x25,
    0x65, //   Logical Maximum (101)
    0x05,
    0x07, //   Usage Page (Keyboard/Keypad)
    0x19,
    0x00, //   Usage Minimum (Reserved)
    0x29,
    0x65, //   Usage Maximum (Keyboard Application)
    0x81,
    0x00, //   Input (Data, Array)
    0xc0, // End Collection
];

#[derive(Debug, Clone, Serialize, Deserialize)]
struct HidCommand {
    id: String,
    kind: String,
    #[serde(default)]
    mouse: Option<MouseCommand>,
    #[serde(default)]
    keyboard: Option<KeyboardCommand>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MouseCommand {
    pub reports: Vec<MouseReport>,
    pub delay_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MouseReport {
    pub buttons: u8,
    pub dx: i8,
    pub dy: i8,
    pub wheel: i8,
    pub repeat: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyboardCommand {
    pub reports: Vec<KeyboardReport>,
    pub delay_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyboardReport {
    pub modifiers: u8,
    pub keys: Vec<u8>,
    pub repeat: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HidAck {
    pub ok: bool,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct HidPaths {
    pub root: PathBuf,
    pub state_file: PathBuf,
    pub pid_file: PathBuf,
    pub command_dir: PathBuf,
    pub ack_dir: PathBuf,
    pub stdout_log: PathBuf,
    pub stderr_log: PathBuf,
}

impl HidPaths {
    pub fn new(state_file: PathBuf) -> Result<Self> {
        let root = state_file
            .parent()
            .ok_or_else(|| anyhow!("BLE helper state path has no parent"))?
            .to_path_buf();
        Ok(Self {
            state_file,
            pid_file: root.join("ble-helper-hid-server.pid"),
            command_dir: root.join("ble-helper-hid-commands"),
            ack_dir: root.join("ble-helper-hid-acks"),
            stdout_log: root.join("ble-helper-hid-server.stdout.log"),
            stderr_log: root.join("ble-helper-hid-server.stderr.log"),
            root,
        })
    }

    pub fn ensure_dirs(&self) -> Result<()> {
        fs::create_dir_all(&self.root)?;
        fs::create_dir_all(&self.command_dir)?;
        fs::create_dir_all(&self.ack_dir)?;
        Ok(())
    }
}

pub fn start_server_if_needed(paths: &HidPaths) -> Result<()> {
    paths.ensure_dirs()?;
    if let Some(pid) = fs::read_to_string(&paths.pid_file)
        .ok()
        .and_then(|text| text.trim().parse::<u32>().ok())
    {
        if process_exists(pid) {
            return Ok(());
        }
    }

    clear_pending_commands(paths)?;

    let exe = std::env::current_exe().context("failed to resolve current ble-helper executable")?;
    let stdout = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&paths.stdout_log)?;
    let stderr = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&paths.stderr_log)?;

    let mut command = std::process::Command::new(exe);
    command
        .arg("serve")
        .arg("--state-file")
        .arg(&paths.state_file)
        .stdin(std::process::Stdio::null())
        .stdout(stdout)
        .stderr(stderr);
    command.creation_flags(CREATE_NO_WINDOW);
    let child = command.spawn().context("failed to spawn BLE HID server")?;
    fs::write(&paths.pid_file, child.id().to_string())?;
    Ok(())
}

fn clear_pending_commands(paths: &HidPaths) -> Result<()> {
    for entry in fs::read_dir(&paths.command_dir)? {
        let path = entry?.path();
        if path.extension().and_then(|ext| ext.to_str()) == Some("json") {
            let _ = fs::remove_file(path);
        }
    }
    Ok(())
}

pub fn stop_server(paths: &HidPaths) -> Result<()> {
    enqueue_command(paths, "stop", Duration::from_secs(3)).map(|_| ())
}

pub fn execute_pointer(paths: &HidPaths, kind: &str) -> Result<HidAck> {
    enqueue_command(paths, kind, Duration::from_secs(5))
}

pub fn execute_mouse(paths: &HidPaths, mouse: MouseCommand) -> Result<HidAck> {
    enqueue_hid_command(
        paths,
        HidCommand {
            id: String::new(),
            kind: "mouse".into(),
            mouse: Some(mouse),
            keyboard: None,
        },
        Duration::from_secs(5),
    )
}

pub fn execute_keyboard(paths: &HidPaths, keyboard: KeyboardCommand) -> Result<HidAck> {
    enqueue_hid_command(
        paths,
        HidCommand {
            id: String::new(),
            kind: "keyboard".into(),
            mouse: None,
            keyboard: Some(keyboard),
        },
        Duration::from_secs(5),
    )
}

pub fn serve(state_file: PathBuf) -> Result<()> {
    let paths = HidPaths::new(state_file)?;
    paths.ensure_dirs()?;
    fs::write(&paths.pid_file, std::process::id().to_string())?;

    let server = HidMouseServer::start(&paths)?;
    let mut next_status_write = Instant::now();
    loop {
        if next_status_write <= Instant::now() {
            server.write_state(&paths)?;
            next_status_write = Instant::now() + Duration::from_millis(500);
        }

        for command_path in pending_commands(&paths.command_dir)? {
            let command_text = fs::read_to_string(&command_path);
            let _ = fs::remove_file(&command_path);
            let command = match command_text
                .ok()
                .and_then(|text| serde_json::from_str::<HidCommand>(&text).ok())
            {
                Some(command) => command,
                None => continue,
            };
            let ack = match command.kind.as_str() {
                "stop" => HidAck {
                    ok: true,
                    message: "BLE HID server stopping".into(),
                },
                "mouse" => match command.mouse {
                    Some(mouse) => match server.execute_mouse(&mouse) {
                        Ok(()) => HidAck {
                            ok: true,
                            message: "BLE HID mouse reports sent".into(),
                        },
                        Err(err) => HidAck {
                            ok: false,
                            message: err.to_string(),
                        },
                    },
                    None => HidAck {
                        ok: false,
                        message: "missing mouse command payload".into(),
                    },
                },
                "keyboard" => match command.keyboard {
                    Some(keyboard) => match server.execute_keyboard(&keyboard) {
                        Ok(()) => HidAck {
                            ok: true,
                            message: "BLE HID keyboard reports sent".into(),
                        },
                        Err(err) => HidAck {
                            ok: false,
                            message: err.to_string(),
                        },
                    },
                    None => HidAck {
                        ok: false,
                        message: "missing keyboard command payload".into(),
                    },
                },
                _ => match server.execute_pointer_demo() {
                    Ok(()) => HidAck {
                        ok: true,
                        message: "BLE HID mouse report sequence sent".into(),
                    },
                    Err(err) => HidAck {
                        ok: false,
                        message: err.to_string(),
                    },
                },
            };
            write_ack(&paths, &command.id, &ack)?;
            if command.kind == "stop" {
                server.stop();
                let _ = fs::remove_file(&paths.pid_file);
                return Ok(());
            }
        }
        thread::sleep(COMMAND_POLL_INTERVAL);
    }
}

struct HidMouseServer {
    hid_provider: GattServiceProvider,
    battery_provider: GattServiceProvider,
    mouse_report: GattLocalCharacteristic,
    keyboard_report: GattLocalCharacteristic,
}

impl HidMouseServer {
    fn start(paths: &HidPaths) -> Result<Self> {
        let hid_provider = create_provider(HID_SERVICE_UUID, "HID")?;
        let hid_service = hid_provider.Service()?;

        create_static_characteristic(
            &hid_service,
            HID_INFORMATION_UUID,
            GattCharacteristicProperties::Read,
            &HID_INFORMATION,
            "HID Information",
        )?;
        create_static_characteristic(
            &hid_service,
            REPORT_MAP_UUID,
            GattCharacteristicProperties::Read,
            HID_REPORT_MAP,
            "Report Map",
        )?;
        create_ack_write_characteristic(
            &hid_service,
            HID_CONTROL_POINT_UUID,
            GattCharacteristicProperties::WriteWithoutResponse,
            "HID Control Point",
        )?;
        create_ack_write_characteristic(
            &hid_service,
            PROTOCOL_MODE_UUID,
            GattCharacteristicProperties::Read | GattCharacteristicProperties::WriteWithoutResponse,
            "Protocol Mode",
        )?;
        let mouse_report = create_static_characteristic(
            &hid_service,
            REPORT_UUID,
            GattCharacteristicProperties::Read | GattCharacteristicProperties::Notify,
            &EMPTY_MOUSE_REPORT,
            "Mouse Input Report",
        )?;
        create_static_descriptor(
            &mouse_report,
            REPORT_REFERENCE_DESCRIPTOR_UUID,
            &[MOUSE_REPORT_ID, REPORT_TYPE_INPUT],
        )?;
        let keyboard_report = create_static_characteristic(
            &hid_service,
            REPORT_UUID,
            GattCharacteristicProperties::Read | GattCharacteristicProperties::Notify,
            &EMPTY_KEYBOARD_REPORT,
            "Keyboard Input Report",
        )?;
        create_static_descriptor(
            &keyboard_report,
            REPORT_REFERENCE_DESCRIPTOR_UUID,
            &[KEYBOARD_REPORT_ID, REPORT_TYPE_INPUT],
        )?;

        let battery_provider = create_provider(BATTERY_SERVICE_UUID, "Battery")?;
        let battery_service = battery_provider.Service()?;
        create_static_characteristic(
            &battery_service,
            BATTERY_LEVEL_UUID,
            GattCharacteristicProperties::Read | GattCharacteristicProperties::Notify,
            &BATTERY_LEVEL,
            "Battery Level",
        )?;

        let params = GattServiceProviderAdvertisingParameters::new()?;
        params.SetIsDiscoverable(true)?;
        params.SetIsConnectable(true)?;

        hid_provider.StartAdvertisingWithParameters(&params)?;
        battery_provider.StartAdvertisingWithParameters(&params)?;

        let server = Self {
            hid_provider,
            battery_provider,
            mouse_report,
            keyboard_report,
        };
        server.write_state(paths)?;
        Ok(server)
    }

    fn connected(&self) -> bool {
        self.characteristic_connected(&self.mouse_report)
            || self.characteristic_connected(&self.keyboard_report)
    }

    fn characteristic_connected(&self, characteristic: &GattLocalCharacteristic) -> bool {
        characteristic
            .SubscribedClients()
            .and_then(|clients| clients.Size())
            .map(|size| size > 0)
            .unwrap_or(false)
    }

    fn write_state(&self, paths: &HidPaths) -> Result<()> {
        let hid_status = self.hid_provider.AdvertisementStatus()?;
        let connected = self.connected();
        let mut notes = Vec::new();
        if self.characteristic_connected(&self.mouse_report) {
            notes.push("BLE HID mouse client subscribed".to_string());
        }
        if self.characteristic_connected(&self.keyboard_report) {
            notes.push("BLE HID keyboard client subscribed".to_string());
        }
        let (phase, execute_ready, notes) = if connected {
            ("Connected", true, notes)
        } else if hid_status == GattServiceProviderAdvertisementStatus::Started
            || hid_status
                == GattServiceProviderAdvertisementStatus::StartedWithoutAllAdvertisementData
        {
            (
                "Advertising",
                false,
                vec!["BLE HID mouse advertising from Windows".to_string()],
            )
        } else {
            (
                "Error",
                false,
                vec![format!("BLE HID advertisement status: {:?}", hid_status)],
            )
        };

        let payload = serde_json::json!({
            "phase": phase,
            "checklist": [
                "Enable Bluetooth",
                "Pair iPhone with the Windows BLE HID mouse",
                "Enable AssistiveTouch"
            ],
            "notes": notes,
            "paired_device_id": if connected { Some("ble-hid-client") } else { None::<&str> },
            "paired_device_name": if connected { Some("BLE HID client") } else { None::<&str> },
            "bonded": connected,
            "execute_ready": execute_ready
        });
        fs::write(&paths.state_file, serde_json::to_string_pretty(&payload)?)?;
        Ok(())
    }

    fn execute_pointer_demo(&self) -> Result<()> {
        if !self.connected() {
            return Err(anyhow!(
                "BLE HID mouse has no subscribed client; pair and connect the iPhone first"
            ));
        }

        for report in pointer_demo_reports() {
            let value = buffer(&report)?;
            self.mouse_report.NotifyValueAsync(&value)?.join()?;
            thread::sleep(Duration::from_millis(25));
        }
        Ok(())
    }

    fn execute_mouse(&self, mouse: &MouseCommand) -> Result<()> {
        if !self.connected() {
            return Err(anyhow!(
                "BLE HID mouse has no subscribed client; pair and connect the iPhone first"
            ));
        }
        if mouse.reports.is_empty() {
            return Err(anyhow!("mouse command has no reports"));
        }

        let delay = Duration::from_millis(mouse.delay_ms);
        for report in &mouse.reports {
            let report_bytes = [
                report.buttons & 0x07,
                report.dx as u8,
                report.dy as u8,
                report.wheel as u8,
            ];
            for _ in 0..report.repeat.max(1) {
                let value = buffer(&report_bytes)?;
                self.mouse_report.NotifyValueAsync(&value)?.join()?;
                thread::sleep(delay);
            }
        }
        Ok(())
    }

    fn execute_keyboard(&self, keyboard: &KeyboardCommand) -> Result<()> {
        if !self.characteristic_connected(&self.keyboard_report) {
            return Err(anyhow!(
                "BLE HID keyboard has no subscribed client; pair and connect the iPhone first"
            ));
        }
        if keyboard.reports.is_empty() {
            return Err(anyhow!("keyboard command has no reports"));
        }

        let delay = Duration::from_millis(keyboard.delay_ms);
        for report in &keyboard.reports {
            let mut report_bytes = [0u8; 8];
            report_bytes[0] = report.modifiers;
            for (index, key) in report.keys.iter().take(6).enumerate() {
                report_bytes[index + 2] = *key;
            }
            for _ in 0..report.repeat.max(1) {
                let value = buffer(&report_bytes)?;
                self.keyboard_report.NotifyValueAsync(&value)?.join()?;
                thread::sleep(delay);
            }
        }
        Ok(())
    }

    fn stop(&self) {
        let _ = self.hid_provider.StopAdvertising();
        let _ = self.battery_provider.StopAdvertising();
    }
}

fn create_provider(uuid: GUID, label: &str) -> Result<GattServiceProvider> {
    let result = GattServiceProvider::CreateAsync(uuid)?.join()?;
    let error = result.Error()?;
    if error != BluetoothError::Success {
        return Err(anyhow!(
            "failed to create {label} GATT service provider: {:?}",
            error
        ));
    }
    result.ServiceProvider().map_err(Into::into)
}

fn create_static_characteristic(
    service: &GattLocalService,
    uuid: GUID,
    properties: GattCharacteristicProperties,
    value: &[u8],
    description: &str,
) -> Result<GattLocalCharacteristic> {
    let params = GattLocalCharacteristicParameters::new()?;
    params.SetCharacteristicProperties(properties)?;
    let static_value = buffer(value)?;
    params.SetStaticValue(&static_value)?;
    params.SetReadProtectionLevel(GattProtectionLevel::Plain)?;
    params.SetWriteProtectionLevel(GattProtectionLevel::Plain)?;
    params.SetUserDescription(&HSTRING::from(description))?;
    let result = service.CreateCharacteristicAsync(uuid, &params)?.join()?;
    let error = result.Error()?;
    if error != BluetoothError::Success {
        return Err(anyhow!(
            "failed to create {description} characteristic: {:?}",
            error
        ));
    }
    result.Characteristic().map_err(Into::into)
}

fn create_ack_write_characteristic(
    service: &GattLocalService,
    uuid: GUID,
    properties: GattCharacteristicProperties,
    description: &str,
) -> Result<GattLocalCharacteristic> {
    let characteristic = create_static_characteristic(
        service,
        uuid,
        properties,
        &PROTOCOL_MODE_REPORT,
        description,
    )?;
    let handler = TypedEventHandler::new(
        move |_sender: windows::core::Ref<GattLocalCharacteristic>,
              args: windows::core::Ref<
            windows::Devices::Bluetooth::GenericAttributeProfile::GattWriteRequestedEventArgs,
        >| {
            if let Some(args) = args.as_ref() {
                let deferral = args.GetDeferral()?;
                let request = args.GetRequestAsync()?.join()?;
                let _ = request.Value();
                request.Respond()?;
                deferral.Complete()?;
            }
            Ok(())
        },
    );
    let _ = characteristic.WriteRequested(&handler)?;
    Ok(characteristic)
}

fn create_static_descriptor(
    characteristic: &GattLocalCharacteristic,
    uuid: GUID,
    value: &[u8],
) -> Result<()> {
    let params = GattLocalDescriptorParameters::new()?;
    let static_value = buffer(value)?;
    params.SetStaticValue(&static_value)?;
    params.SetReadProtectionLevel(GattProtectionLevel::Plain)?;
    params.SetWriteProtectionLevel(GattProtectionLevel::Plain)?;
    let result = characteristic
        .CreateDescriptorAsync(uuid, &params)?
        .join()?;
    let error = result.Error()?;
    if error != BluetoothError::Success {
        return Err(anyhow!(
            "failed to create descriptor {:?}: {:?}",
            uuid,
            error
        ));
    }
    Ok(())
}

fn buffer(bytes: &[u8]) -> windows::core::Result<IBuffer> {
    let writer = DataWriter::new()?;
    writer.WriteBytes(bytes)?;
    writer.DetachBuffer()
}

fn pointer_demo_reports() -> Vec<[u8; 4]> {
    let mut reports = Vec::new();
    for _ in 0..8 {
        reports.push([0, 18, 0, 0]);
    }
    for _ in 0..8 {
        reports.push([0, 0, 18, 0]);
    }
    reports.push([1, 0, 0, 0]);
    reports.push([0, 0, 0, 0]);
    reports
}

fn enqueue_command(paths: &HidPaths, kind: &str, timeout: Duration) -> Result<HidAck> {
    enqueue_hid_command(
        paths,
        HidCommand {
            id: String::new(),
            kind: kind.into(),
            mouse: None,
            keyboard: None,
        },
        timeout,
    )
}

fn enqueue_hid_command(
    paths: &HidPaths,
    mut command: HidCommand,
    timeout: Duration,
) -> Result<HidAck> {
    paths.ensure_dirs()?;
    let id = format!(
        "{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    );
    command.id = id.clone();
    let command_path = paths.command_dir.join(format!("{id}.json"));
    let temp_path = paths.command_dir.join(format!("{id}.tmp"));
    fs::write(&temp_path, serde_json::to_string(&command)?)?;
    fs::rename(temp_path, command_path)?;

    let ack_path = paths.ack_dir.join(format!("{id}.json"));
    let start = Instant::now();
    while start.elapsed() < timeout {
        if let Ok(text) = fs::read_to_string(&ack_path) {
            let _ = fs::remove_file(&ack_path);
            return Ok(serde_json::from_str(&text)?);
        }
        thread::sleep(Duration::from_millis(50));
    }

    Err(anyhow!("timed out waiting for BLE HID server response"))
}

fn write_ack(paths: &HidPaths, id: &str, ack: &HidAck) -> Result<()> {
    let ack_path = paths.ack_dir.join(format!("{id}.json"));
    let temp_path = paths.ack_dir.join(format!("{id}.tmp"));
    fs::write(&temp_path, serde_json::to_string(ack)?)?;
    fs::rename(temp_path, ack_path)?;
    Ok(())
}

fn pending_commands(command_dir: &Path) -> Result<Vec<PathBuf>> {
    let mut paths = fs::read_dir(command_dir)?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("json"))
        .collect::<Vec<_>>();
    paths.sort();
    Ok(paths)
}

fn process_exists(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    std::process::Command::new("cmd")
        .args([
            "/C",
            &format!("tasklist /FI \"PID eq {pid}\" | findstr /R \"^[^ ]* *{pid} \""),
        ])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .status()
        .is_ok_and(|status| status.success())
}
