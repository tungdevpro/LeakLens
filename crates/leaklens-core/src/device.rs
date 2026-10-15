use crate::adb::{Adb, AdbError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Device {
    pub serial: String,
    pub status: DeviceStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeviceStatus {
    Supported { api_level: u32 },
    Unsupported { api_level: u32 },
    Unauthorized,
    Offline,
}

/// Android 8 (API 26) is the oldest version whose output LeakLens can parse.
pub const MIN_API_LEVEL: u32 = 26;

/// Lists every Device adb can see, checking the Android version of those that are ready.
pub async fn list_devices(adb: &impl Adb) -> Result<Vec<Device>, AdbError> {
    let output = adb.run(&["devices"]).await?;
    let mut devices = Vec::new();
    // The first line is the "List of devices attached" header.
    for line in output.lines().skip(1) {
        let Some((serial, state)) = line.split_once('\t') else {
            continue;
        };
        let status = match state.trim() {
            "device" => {
                let api_level = api_level(adb, serial).await?;
                if api_level >= MIN_API_LEVEL {
                    DeviceStatus::Supported { api_level }
                } else {
                    DeviceStatus::Unsupported { api_level }
                }
            }
            "unauthorized" => DeviceStatus::Unauthorized,
            _ => DeviceStatus::Offline,
        };
        devices.push(Device {
            serial: serial.to_owned(),
            status,
        });
    }
    Ok(devices)
}

async fn api_level(adb: &impl Adb, serial: &str) -> Result<u32, AdbError> {
    let output = adb
        .run(&["-s", serial, "shell", "getprop", "ro.build.version.sdk"])
        .await?;
    output.trim().parse().map_err(|_| {
        AdbError::Failed(format!(
            "unexpected API level `{}` from {serial}",
            output.trim()
        ))
    })
}

impl Device {
    /// Whether this Device can be chosen for a Session.
    pub fn is_selectable(&self) -> bool {
        matches!(self.status, DeviceStatus::Supported { .. })
    }
}

/// The serial of the only selectable Device, if there is exactly one.
pub fn auto_select(devices: &[Device]) -> Option<&str> {
    let mut selectable = devices.iter().filter(|d| d.is_selectable());
    match (selectable.next(), selectable.next()) {
        (Some(only), None) => Some(&only.serial),
        _ => None,
    }
}
