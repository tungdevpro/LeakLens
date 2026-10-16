//! Everything LeakLens knows about Devices, Target Apps and Sessions, with no UI.

mod adb;
mod cpu;
mod device;
pub mod fake;
mod find_adb;
mod memory;
mod session;
mod target_app;

pub use adb::{Adb, AdbBinary, AdbError};
pub use device::{Device, DeviceStatus, MIN_API_LEVEL, auto_select, list_devices};
pub use find_adb::{AdbNotFound, find_adb};
pub use memory::Memory;
pub use session::{Sample, SamplingIntervals, Session, SessionState};
pub use target_app::{TargetAppChoices, list_target_apps};
