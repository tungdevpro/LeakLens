use std::ffi::OsString;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error(
    "adb was not found. Install the Android SDK Platform-Tools, then either set ANDROID_HOME \
     (or ANDROID_SDK_ROOT) to your Android SDK folder or add its platform-tools folder to your PATH."
)]
pub struct AdbNotFound;

/// Finds the `adb` program: first in the Android SDK named by `ANDROID_HOME`
/// or `ANDROID_SDK_ROOT`, then on `PATH`. `env` reads an environment variable.
pub fn find_adb(env: impl Fn(&str) -> Option<OsString>) -> Result<PathBuf, AdbNotFound> {
    let adb = format!("adb{}", std::env::consts::EXE_SUFFIX);

    let sdk_dirs = ["ANDROID_HOME", "ANDROID_SDK_ROOT"]
        .into_iter()
        .filter_map(&env)
        .map(|sdk| PathBuf::from(sdk).join("platform-tools"));
    let path_dirs = env("PATH")
        .map(|path| std::env::split_paths(&path).collect::<Vec<_>>())
        .unwrap_or_default();

    sdk_dirs
        .chain(path_dirs)
        .map(|dir| dir.join(&adb))
        .find(|candidate| candidate.is_file())
        .ok_or(AdbNotFound)
}
