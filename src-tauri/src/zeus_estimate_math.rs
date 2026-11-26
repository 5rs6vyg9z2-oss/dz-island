// Linear session estimate inspired by Zeus Pro Suite; not hardware telemetry.
#[derive(Clone, Copy)]
pub enum BluetoothObservation {
    Disconnected,
    Unavailable,
    Connected { percent: Option<u8>, charging: bool },
}

pub fn remaining(used: u64, hours: u32) -> (u8, u64) {
    let total = u64::from(hours.clamp(1, 100)) * 3600;
    let left = total.saturating_sub(used);
    (((left as f64 / total as f64 * 100.0).round() as u8).min(100), left)
}

pub fn counted_delta(seconds: u64) -> u64 {
    // Long gaps mean suspension or a stalled process, not confirmed headset use.
    if seconds <= 45 { seconds } else { 0 }
}

pub fn used_from_percent(percent: u8, hours: u32) -> u64 {
    u64::from(hours.clamp(1, 100)) * 36 * u64::from(100 - percent.min(100))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn bluetooth_anchor_preserves_partial_charge() {
        assert_eq!(remaining(used_from_percent(70, 18), 18).0, 70);
        assert_eq!(used_from_percent(100, 18), 0);
        assert_eq!(remaining(used_from_percent(0, 18), 18).0, 0);
    }
    #[test] fn full_half_and_empty() {
        assert_eq!(remaining(0, 18), (100, 64800));
        assert_eq!(remaining(32400, 18), (50, 32400));
        assert_eq!(remaining(90000, 18), (0, 0));
    }
    #[test] fn suspend_gap_is_not_usage() {
        assert_eq!(counted_delta(15), 15);
        assert_eq!(counted_delta(3600), 0);
    }
    #[test] fn malformed_capacity_cannot_divide_by_zero() {
        assert_eq!(remaining(1800, 0), (50, 1800));
        assert_eq!(remaining(u64::MAX, 18), (0, 0));
    }
}
