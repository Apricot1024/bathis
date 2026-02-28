use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

/// Battery charging state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BatteryStatus {
    Charging,
    Discharging,
    NotCharging,
    Full,
    Unknown,
}

impl fmt::Display for BatteryStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BatteryStatus::Charging => write!(f, "Charging"),
            BatteryStatus::Discharging => write!(f, "Discharging"),
            BatteryStatus::NotCharging => write!(f, "Not charging"),
            BatteryStatus::Full => write!(f, "Full"),
            BatteryStatus::Unknown => write!(f, "Unknown"),
        }
    }
}

/// A single battery data sample
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatterySample {
    pub timestamp: DateTime<Local>,
    pub capacity: f64,       // percent 0-100
    pub power_watts: f64,    // watts (positive = charging, negative = discharging)
    pub status: BatteryStatus,
    pub energy_now_wh: f64,  // watt-hours
    pub energy_full_wh: f64, // watt-hours
    pub voltage_now_v: f64,  // volts
}

/// Reader for Linux sysfs battery interface
pub struct BatteryReader {
    base_path: PathBuf,
}

impl BatteryReader {
    pub fn new() -> Option<Self> {
        // Try to find a battery in /sys/class/power_supply/
        let ps_path = Path::new("/sys/class/power_supply");
        if !ps_path.exists() {
            return None;
        }

        for entry in fs::read_dir(ps_path).ok()? {
            let entry = entry.ok()?;
            let type_path = entry.path().join("type");
            if let Ok(ptype) = fs::read_to_string(&type_path) {
                if ptype.trim() == "Battery" {
                    return Some(BatteryReader {
                        base_path: entry.path(),
                    });
                }
            }
        }
        None
    }

    fn read_sysfs_string(&self, filename: &str) -> Option<String> {
        let path = self.base_path.join(filename);
        fs::read_to_string(path).ok().map(|s| s.trim().to_string())
    }

    fn read_sysfs_i64(&self, filename: &str) -> Option<i64> {
        self.read_sysfs_string(filename)?.parse::<i64>().ok()
    }

    pub fn sample(&self) -> Option<BatterySample> {
        let capacity = self.read_sysfs_i64("capacity")? as f64;
        let status = match self.read_sysfs_string("status")?.as_str() {
            "Charging" => BatteryStatus::Charging,
            "Discharging" => BatteryStatus::Discharging,
            "Not charging" => BatteryStatus::NotCharging,
            "Full" => BatteryStatus::Full,
            _ => BatteryStatus::Unknown,
        };

        // power_now is in microwatts
        let power_uw = self.read_sysfs_i64("power_now").unwrap_or(0);
        let power_watts = power_uw as f64 / 1_000_000.0;

        // Sign convention: positive = charging, negative = discharging
        let signed_power = match status {
            BatteryStatus::Charging => power_watts,
            BatteryStatus::Discharging => -power_watts,
            _ => 0.0,
        };

        let energy_now_uh = self.read_sysfs_i64("energy_now").unwrap_or(0);
        let energy_full_uh = self.read_sysfs_i64("energy_full").unwrap_or(0);
        let voltage_uv = self.read_sysfs_i64("voltage_now").unwrap_or(0);

        Some(BatterySample {
            timestamp: Local::now(),
            capacity,
            power_watts: signed_power,
            status,
            energy_now_wh: energy_now_uh as f64 / 1_000_000.0,
            energy_full_wh: energy_full_uh as f64 / 1_000_000.0,
            voltage_now_v: voltage_uv as f64 / 1_000_000.0,
        })
    }

    pub fn battery_name(&self) -> String {
        let model = self.read_sysfs_string("model_name").unwrap_or_default();
        let mfr = self.read_sysfs_string("manufacturer").unwrap_or_default();
        if model.is_empty() && mfr.is_empty() {
            self.base_path
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "Battery".to_string())
        } else {
            format!("{mfr} {model}").trim().to_string()
        }
    }
}
