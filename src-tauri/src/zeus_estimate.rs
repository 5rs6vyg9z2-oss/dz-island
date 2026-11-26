use serde::{Deserialize, Serialize};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use crate::zeus_estimate_math::{counted_delta, remaining, used_from_percent, BluetoothObservation};

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Estimate {
    initialized: bool,
    hours: u32,
    used_seconds: u64,
    manual_pause: bool,
    last_bluetooth_percent: Option<u8>,
    last_bluetooth_at: Option<u64>,
    level_increased: bool,
    #[serde(skip_deserializing)]
    source: String,
    #[serde(skip_deserializing)]
    charging: bool,
    #[serde(skip_deserializing)]
    running: bool,
    #[serde(skip_deserializing)]
    percent: u8,
    #[serde(skip_deserializing)]
    remaining_seconds: u64,
}

impl Default for Estimate {
    fn default() -> Self {
        Self {
            initialized: false, hours: 18, used_seconds: 0, manual_pause: false,
            last_bluetooth_percent: None, last_bluetooth_at: None, level_increased: false,
            source: "checking".into(), charging: false, running: false,
            percent: 100, remaining_seconds: 64800,
        }
    }
}

struct Clock { value: Estimate, tick: Instant, saved: Instant }

fn state() -> &'static Mutex<Clock> {
    static STATE: OnceLock<Mutex<Clock>> = OnceLock::new();
    STATE.get_or_init(|| {
        let mut value: Estimate = crate::utils::portable_data_dir().ok()
            .and_then(|dir| std::fs::read(dir.join("zeus-estimate.json")).ok())
            .and_then(|bytes| serde_json::from_slice(&bytes).ok()).unwrap_or_default();
        value.hours = value.hours.clamp(1, 100);
        value.running = false;
        value.source = "checking".into();
        Mutex::new(Clock { value, tick: Instant::now(), saved: Instant::now() })
    })
}

impl Clock {
    fn advance(&mut self) {
        let seconds = self.tick.elapsed().as_secs();
        if seconds > 0 {
            if self.value.running {
                self.value.used_seconds = self.value.used_seconds.saturating_add(counted_delta(seconds));
            }
            self.tick += Duration::from_secs(seconds);
        }
    }
    fn synchronize(&mut self, reading: BluetoothObservation) {
        self.advance();
        self.value.charging = false;
        match reading {
            BluetoothObservation::Connected { percent: Some(percent @ 0..=100), charging } => {
                if self.value.last_bluetooth_percent != Some(percent) {
                    self.value.level_increased = self.value.last_bluetooth_percent.is_some_and(|previous| percent > previous);
                }
                self.value.initialized = true;
                self.value.last_bluetooth_percent = Some(percent);
                self.value.last_bluetooth_at = SystemTime::now().duration_since(UNIX_EPOCH).ok().map(|d| d.as_secs());
                self.value.used_seconds = used_from_percent(percent, self.value.hours);
                self.value.source = "bluetooth".into();
                self.value.charging = charging;
                self.value.running = false;
            }
            BluetoothObservation::Connected { .. } => {
                self.value.source = "bluetooth-unavailable".into();
                self.value.running = false;
            }
            BluetoothObservation::Disconnected => {
                self.value.source = "estimate".into();
                self.value.running = self.value.initialized && !self.value.manual_pause;
            }
            BluetoothObservation::Unavailable => {
                self.value.source = "checking".into();
                self.value.running = false;
            }
        }
        self.tick = Instant::now();
    }
    fn view(&self) -> Estimate {
        let mut value = self.value.clone();
        (value.percent, value.remaining_seconds) = remaining(value.used_seconds, value.hours);
        if value.source == "bluetooth" {
            if let Some(percent) = value.last_bluetooth_percent { value.percent = percent; }
        }
        value
    }
    fn save(&mut self) -> Result<(), String> {
        let dir = crate::utils::portable_data_dir().map_err(|e| e.to_string())?;
        let tmp = dir.join("zeus-estimate.tmp");
        let bytes = serde_json::to_vec_pretty(&self.view()).map_err(|e| e.to_string())?;
        std::fs::write(&tmp, bytes).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, dir.join("zeus-estimate.json")).map_err(|e| e.to_string())?;
        self.saved = Instant::now();
        Ok(())
    }
}

pub fn start_clock() {
    if let Some(percent) = std::env::args()
        .find_map(|arg| arg.strip_prefix("--zeus-estimate-start=").and_then(|value| value.parse::<u8>().ok()))
        .filter(|value| *value <= 100)
    {
        let mut clock = state().lock().unwrap_or_else(|e| e.into_inner());
        clock.value.initialized = true;
        clock.value.used_seconds = used_from_percent(percent, clock.value.hours);
        clock.value.manual_pause = false;
        clock.tick = Instant::now();
    }
    std::thread::spawn(|| loop {
        // Device I/O stays outside the state lock and outside WebView IPC threads.
        let observation = crate::dock_bluetooth::zeus_bluetooth_observation();
        {
            let mut clock = state().lock().unwrap_or_else(|e| e.into_inner());
            let before = (clock.value.source.clone(), clock.value.last_bluetooth_percent, clock.value.running, clock.value.charging);
            clock.synchronize(observation);
            let after = (clock.value.source.clone(), clock.value.last_bluetooth_percent, clock.value.running, clock.value.charging);
            if before != after || clock.saved.elapsed() >= Duration::from_secs(60) {
                if let Err(error) = clock.save() { crate::diagnostics::log(&format!("zeus sync: {error}")); }
            }
        }
        std::thread::sleep(Duration::from_secs(15));
    });
}

#[tauri::command]
pub fn zeus_estimate_status() -> Estimate {
    let mut clock = state().lock().unwrap_or_else(|e| e.into_inner());
    clock.advance();
    clock.view()
}

#[tauri::command]
pub fn zeus_estimate_update(action: String, hours: Option<u32>) -> Result<Estimate, String> {
    let mut clock = state().lock().unwrap_or_else(|e| e.into_inner());
    clock.advance();
    match action.as_str() {
        "full" if clock.value.source != "bluetooth" => {
            clock.value.initialized = true; clock.value.used_seconds = 0;
            clock.value.manual_pause = true; clock.value.running = false;
        }
        "start" if clock.value.initialized => {
            clock.value.manual_pause = false;
            clock.value.running = clock.value.source == "estimate";
        }
        "pause" => { clock.value.manual_pause = true; clock.value.running = false; }
        "hours" => {
            let hours = hours.filter(|h| (1..=100).contains(h)).ok_or("Use uma autonomia entre 1 e 100 horas.")?;
            clock.value.used_seconds = used_from_percent(clock.view().percent, hours);
            clock.value.hours = hours;
        }
        _ => return Err("Use a leitura Bluetooth ou confirme uma carga completa para calibrar.".into()),
    }
    clock.save()?;
    Ok(clock.view())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn clock() -> Clock {
        Clock { value: Estimate::default(), tick: Instant::now(), saved: Instant::now() }
    }
    #[test]
    fn bluetooth_is_exact_and_usb_counts_from_that_anchor() {
        let mut c = clock();
        c.synchronize(BluetoothObservation::Connected { percent: Some(70), charging: false });
        assert_eq!(c.view().percent, 70);
        c.tick -= Duration::from_secs(30);
        c.advance();
        assert_eq!(c.view().percent, 70);
        c.synchronize(BluetoothObservation::Disconnected);
        assert!(c.value.running);
        c.tick -= Duration::from_secs(30);
        c.advance();
        assert_eq!(c.value.used_seconds, 19470);
    }
    #[test]
    fn reconnection_recalibrates_and_increase_does_not_invent_charging() {
        let mut c = clock();
        c.synchronize(BluetoothObservation::Connected { percent: Some(70), charging: false });
        c.synchronize(BluetoothObservation::Disconnected);
        c.synchronize(BluetoothObservation::Connected { percent: Some(90), charging: false });
        assert_eq!(c.view().percent, 90);
        assert!(c.value.level_increased);
        assert!(!c.value.charging);
    }
    #[test]
    fn missing_reading_never_becomes_a_real_percentage() {
        let mut c = clock();
        c.synchronize(BluetoothObservation::Connected { percent: Some(90), charging: false });
        c.synchronize(BluetoothObservation::Connected { percent: None, charging: false });
        assert_eq!(c.value.source, "bluetooth-unavailable");
        assert!(!c.value.running);
        c.synchronize(BluetoothObservation::Unavailable);
        assert_eq!(c.value.source, "checking");
        assert!(!c.value.running);
    }
    #[test]
    fn manual_pause_survives_reconnection_and_restart() {
        let mut c = clock();
        c.value.manual_pause = true;
        c.synchronize(BluetoothObservation::Connected { percent: Some(90), charging: false });
        c.synchronize(BluetoothObservation::Disconnected);
        assert!(!c.value.running);
        let restored: Estimate = serde_json::from_value(serde_json::to_value(&c.value).unwrap()).unwrap();
        assert!(restored.manual_pause);
        assert!(!restored.running);
    }
}
