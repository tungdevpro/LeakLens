use std::ffi::OsString;
use std::path::{Path, PathBuf};

use leaklens_core::find_adb;

fn adb_name() -> String {
    format!("adb{}", std::env::consts::EXE_SUFFIX)
}

/// Creates an empty `adb` executable in `dir` and returns its path.
fn place_adb(dir: &Path) -> PathBuf {
    std::fs::create_dir_all(dir).unwrap();
    let adb = dir.join(adb_name());
    std::fs::write(&adb, "").unwrap();
    adb
}

fn env(vars: &[(&str, OsString)]) -> impl Fn(&str) -> Option<OsString> {
    let vars: Vec<(String, OsString)> = vars
        .iter()
        .map(|(k, v)| (k.to_string(), v.clone()))
        .collect();
    move |key| vars.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone())
}

#[test]
fn finds_adb_in_the_android_sdk_named_by_android_home() {
    let sdk = tempfile::tempdir().unwrap();
    let adb = place_adb(&sdk.path().join("platform-tools"));

    let found = find_adb(env(&[("ANDROID_HOME", sdk.path().into())]));

    assert_eq!(found.unwrap(), adb);
}

#[test]
fn finds_adb_in_the_android_sdk_named_by_android_sdk_root() {
    let sdk = tempfile::tempdir().unwrap();
    let adb = place_adb(&sdk.path().join("platform-tools"));

    let found = find_adb(env(&[("ANDROID_SDK_ROOT", sdk.path().into())]));

    assert_eq!(found.unwrap(), adb);
}

#[test]
fn falls_back_to_adb_on_the_path() {
    let empty_sdk = tempfile::tempdir().unwrap();
    let bin = tempfile::tempdir().unwrap();
    let adb = place_adb(bin.path());
    let path =
        std::env::join_paths([PathBuf::from("/nonexistent"), bin.path().to_owned()]).unwrap();

    let found = find_adb(env(&[
        ("ANDROID_HOME", empty_sdk.path().into()),
        ("PATH", path),
    ]));

    assert_eq!(found.unwrap(), adb);
}

#[test]
fn explains_how_to_fix_the_setup_when_adb_cannot_be_found() {
    let empty = tempfile::tempdir().unwrap();

    let error = find_adb(env(&[("PATH", empty.path().into())])).unwrap_err();

    let message = error.to_string();
    assert!(message.contains("ANDROID_HOME"), "{message}");
    assert!(message.contains("PATH"), "{message}");
}
