use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use windows::Devices::Bluetooth::{BluetoothConnectionStatus, BluetoothDevice, BluetoothLEDevice};
use windows::Devices::Enumeration::DeviceInformation;
use windows::core::HSTRING;
use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED};

const CONNECTION_INTERVAL: Duration = Duration::from_secs(5);


#[derive(Clone, Serialize)]
pub struct DockBluetoothDevice {
    id: String,
    name: String,
    percent: Option<u8>,
    connected: bool,
    charging: bool,
    kind: String,
    detail: String,
    #[serde(skip)]
    order: usize,
}

#[derive(Deserialize)]
struct BatteryHubDevice {
    key: String,
    name: String,
    via: String,
    percent: i32,
    connected: bool,
    kind: String,
    detail: String,
    charging: bool,
    #[serde(default)]
    source: String,
    #[serde(default)]
    note: String,
    #[serde(default)]
    hidden: bool,
    #[serde(default, rename = "displayName")]
    display_name: String,
    #[serde(default)]
    asleep: bool,
}

#[derive(Default)]
struct Snapshot {
    devices: Vec<DockBluetoothDevice>,
    connections_at: Option<Instant>,
}

fn snapshot() -> &'static Mutex<Snapshot> {
    static STATE: OnceLock<Mutex<Snapshot>> = OnceLock::new();
    STATE.get_or_init(|| Mutex::new(Snapshot::default()))
}

fn address_from_id(id: &str) -> Option<String> {
    let tail = id.rsplit(['-', '_']).next()?;
    let address: String = tail.chars().filter(|&c| c != ':').collect();
    (address.len() == 12 && address.bytes().all(|b| b.is_ascii_hexdigit()))
        .then(|| address.to_ascii_lowercase())
}

struct ComApartment(bool);

impl Drop for ComApartment {
    fn drop(&mut self) {
        if self.0 { unsafe { CoUninitialize() }; }
    }
}

fn connected_windows_devices() -> windows::core::Result<Vec<DockBluetoothDevice>> {
    let _com = ComApartment(unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.is_ok());
    let selectors = [
        BluetoothDevice::GetDeviceSelectorFromConnectionStatus(BluetoothConnectionStatus::Connected),
        BluetoothLEDevice::GetDeviceSelectorFromConnectionStatus(BluetoothConnectionStatus::Connected),
    ];
    let mut devices = Vec::new();
    let mut last_error = None;
    let mut queried = false;
    for selector in selectors {
        let collection = match selector.and_then(|s: HSTRING| DeviceInformation::FindAllAsyncAqsFilter(&s))
            .and_then(|op| op.join())
        {
            Ok(collection) => collection,
            Err(error) => {
                last_error = Some(error);
                continue;
            }
        };
        queried = true;
        for i in 0..collection.Size().unwrap_or(0) {
            let Ok(device) = collection.GetAt(i) else { continue };
            let (Ok(id), Ok(name)) = (device.Id(), device.Name()) else { continue };
            let id = id.to_string();
            let id = address_from_id(&id).unwrap_or(id);
            if !devices.iter().any(|other: &DockBluetoothDevice| other.id == id) {
                devices.push(DockBluetoothDevice {
                    id, name: name.to_string(), percent: None, connected: true,
                    charging: false, kind: "bluetooth".into(), detail: String::new(), order: usize::MAX,
                });
            }
        }
    }
    if !queried {
        if let Some(error) = last_error { return Err(error); }
    }
    Ok(devices)
}

fn batteryhub_path() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    Some(exe.parent()?.join("Battery").join("BatteryEngine.exe"))
}

pub fn start_battery_engine() {
    let Some(exe) = batteryhub_path().filter(|path| path.is_file()) else {
        crate::diagnostics::log("battery: integrated engine missing");
        return;
    };
    let Ok(data) = crate::utils::portable_data_dir() else { return };
    let _ = std::fs::remove_file(data.join("battery-snapshot.json"));
    let argument = format!("--dz-island-parent={}", std::process::id());
    std::thread::spawn(move || {
        // Retry briefly if the old engine is still shutting down during a DZ Island restart.
        for _ in 0..3 {
            let started = Instant::now();
            let status = std::process::Command::new(&exe)
                .arg(&argument)
                .creation_flags(0x0800_0000)
                .status();
            if started.elapsed() >= Duration::from_secs(15) { break; }
            crate::diagnostics::log(&format!("battery: engine exited: {status:?}"));
            std::thread::sleep(Duration::from_secs(2));
        }
    });
}

fn read_batteryhub() -> Vec<BatteryHubDevice> {
    crate::utils::portable_data_dir().ok()
        .and_then(|dir| std::fs::read(dir.join("battery-snapshot.json")).ok())
        .and_then(|json| serde_json::from_slice(&json).ok())
        .unwrap_or_default()
}

pub fn zeus_bluetooth_observation() -> crate::zeus_estimate_math::BluetoothObservation {
    use crate::zeus_estimate_math::BluetoothObservation;
    let is_zeus = |name: &str| {
        let name = name.to_lowercase();
        name.contains("h510-pro") || name.contains("zeus pro")
    };
    // PnP presence and a stored percentage alone do not prove a live connection.
    let Ok(connected) = connected_windows_devices() else { return BluetoothObservation::Unavailable };
    let count = connected.iter().filter(|device| is_zeus(&device.name)).count();
    if count == 0 { return BluetoothObservation::Disconnected; }
    if count != 1 { return BluetoothObservation::Unavailable; }
    let fresh = crate::utils::portable_data_dir().ok()
        .and_then(|dir| std::fs::metadata(dir.join("battery-snapshot.json")).ok())
        .and_then(|meta| meta.modified().ok())
        .and_then(|time| time.elapsed().ok())
        .is_some_and(|age| age <= Duration::from_secs(120));
    if !fresh { return BluetoothObservation::Connected { percent: None, charging: false }; }
    let readings: Vec<_> = read_batteryhub().into_iter().filter(|device| {
        is_zeus(&device.name) && device.via.eq_ignore_ascii_case("Bluetooth")
            && device.connected && !device.asleep && (0..=100).contains(&device.percent)
    }).collect();
    if readings.len() != 1 { return BluetoothObservation::Connected { percent: None, charging: false }; }
    BluetoothObservation::Connected { percent: Some(readings[0].percent as u8), charging: readings[0].charging }
}

#[tauri::command]
pub fn open_battery_menu() -> Result<(), String> {
    use windows::core::{w, PCWSTR};
    use windows::Win32::Foundation::{LPARAM, WPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{AllowSetForegroundWindow, FindWindowW, GetWindowThreadProcessId, PostMessageW, WM_APP};
    let result = unsafe {
        FindWindowW(w!("DzIslandBatteryEngineWnd"), PCWSTR::null())
            .and_then(|window| {
                let mut pid = 0;
                GetWindowThreadProcessId(window, Some(&mut pid));
                if pid != 0 { let _ = AllowSetForegroundWindow(pid); }
                PostMessageW(Some(window), WM_APP + 42, WPARAM(0), LPARAM(0))
            })
    };
    result.map_err(|_| "O motor de bateria não está disponível.".into())
}

fn merge_devices(mut devices: Vec<DockBluetoothDevice>, batteries: &[BatteryHubDevice]) -> Vec<DockBluetoothDevice> {
    let mut names = HashMap::new();
    for device in &devices {
        *names.entry(device.name.trim().to_lowercase()).or_insert(0) += 1;
    }
    let mut battery_names = HashMap::new();
    for battery in batteries {
        *battery_names.entry(battery.name.trim().to_lowercase()).or_insert(0) += 1;
    }
    for (order, battery) in batteries.iter().enumerate() {
        let name = battery.name.trim().to_lowercase();
        let bluetooth = battery.key.starts_with("bt:") || battery.via.to_ascii_lowercase().contains("bluetooth");
        let display_name = if battery.display_name.is_empty() { &battery.name } else { &battery.display_name };
        let address = battery.key.strip_prefix("bt:").and_then(address_from_id);
        let percent = (!battery.asleep).then(|| u8::try_from(battery.percent).ok().filter(|&n| n <= 100)).flatten();
        let detail = [&battery.detail, &battery.note, &battery.source].into_iter()
            .filter(|value| !value.is_empty()).cloned().collect::<Vec<_>>().join(" · ");
        let matched = devices.iter().position(|device| {
            address.as_ref() == Some(&device.id)
                || (bluetooth && names.get(&name) == Some(&1) && battery_names.get(&name) == Some(&1)
                    && device.name.trim().to_lowercase() == name)
        });
        if let Some(index) = matched {
            if battery.hidden || !battery.connected {
                devices.remove(index);
                continue;
            }
            let device = &mut devices[index];
            device.name = display_name.clone();
            device.order = order;
            device.percent = percent.or(device.percent);
            device.charging = battery.charging;
            device.kind = battery.kind.clone();
            device.detail = detail.clone();
        } else if !battery.hidden && (!bluetooth || !names.contains_key(&name)) {
            devices.push(DockBluetoothDevice {
                id: battery.key.clone(), name: display_name.clone(), percent,
                connected: battery.connected, charging: battery.charging,
                kind: battery.kind.clone(), detail, order,
            });
        }
    }
    devices.sort_by_cached_key(|device| (device.order, device.name.to_lowercase()));
    devices
}

fn collect_dock_devices() -> Vec<DockBluetoothDevice> {
    let mut state = snapshot().lock().unwrap_or_else(|err| err.into_inner());
    if state.connections_at.is_none_or(|at| at.elapsed() >= CONNECTION_INTERVAL) {
        if let Ok(devices) = connected_windows_devices() {
            state.devices = devices;
        }
        state.connections_at = Some(Instant::now());
    }
    let devices = state.devices.clone();
    drop(state);
    merge_devices(devices, &read_batteryhub())
}

#[tauri::command]
pub async fn dock_bluetooth_devices() -> Vec<DockBluetoothDevice> {
    tauri::async_runtime::spawn_blocking(collect_dock_devices).await.unwrap_or_default()
}
