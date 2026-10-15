use crate::adb::{Adb, AdbError};

/// The Target Apps a user can pick from on one Device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetAppChoices {
    pub packages: Vec<String>,
    pub foreground: Option<String>,
}

/// Lists the Device's third-party packages, sorted, and which of them is on screen.
pub async fn list_target_apps(adb: &impl Adb, serial: &str) -> Result<TargetAppChoices, AdbError> {
    let listing = adb
        .run(&["-s", serial, "shell", "pm", "list", "packages", "-3"])
        .await?;
    let mut packages: Vec<String> = listing
        .lines()
        .filter_map(|line| line.trim().strip_prefix("package:"))
        .map(str::to_owned)
        .collect();
    packages.sort();

    let activities = adb
        .run(&["-s", serial, "shell", "dumpsys", "activity", "activities"])
        .await?;
    let foreground = resumed_activity(&activities)
        .map(|(package, _)| package)
        .filter(|package| packages.iter().any(|p| p == package))
        .map(str::to_owned);

    Ok(TargetAppChoices {
        packages,
        foreground,
    })
}

/// The package and Activity name of the resumed Activity in a
/// `dumpsys activity activities` dump.
///
/// Android 8-9 report it as `mResumedActivity:`, Android 10+ as `ResumedActivity:`.
pub(crate) fn resumed_activity(dump: &str) -> Option<(&str, &str)> {
    let line = dump.lines().map(str::trim).find(|line| {
        line.starts_with("mResumedActivity:") || line.starts_with("ResumedActivity:")
    })?;
    // ActivityRecord{<hash> u<user> <package>/<activity> t<task>}
    let record = line.split_once("ActivityRecord{")?.1;
    let component = record.split_whitespace().nth(2)?;
    component.split_once('/')
}
