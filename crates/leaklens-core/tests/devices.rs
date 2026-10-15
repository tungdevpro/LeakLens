use leaklens_core::fake::FakeAdb;
use leaklens_core::{Device, DeviceStatus, auto_select, list_devices};

fn fixture(name: &str) -> String {
    std::fs::read_to_string(format!(
        "{}/tests/fixtures/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

#[tokio::test]
async fn lists_every_connected_device_with_its_status() {
    let adb = FakeAdb::new()
        .respond("devices", fixture("adb_devices_mixed.txt"))
        .respond(
            "-s emulator-5554 shell getprop ro.build.version.sdk",
            fixture("getprop_sdk_34.txt"),
        )
        .respond(
            "-s ZY22OLDPHONE shell getprop ro.build.version.sdk",
            fixture("getprop_sdk_25.txt"),
        );

    let devices = list_devices(&adb).await.unwrap();

    assert_eq!(
        devices,
        vec![
            Device {
                serial: "emulator-5554".into(),
                status: DeviceStatus::Supported { api_level: 34 }
            },
            Device {
                serial: "R5CT1234ABC".into(),
                status: DeviceStatus::Unauthorized
            },
            Device {
                serial: "0123456789ABCDEF".into(),
                status: DeviceStatus::Offline
            },
            Device {
                serial: "ZY22OLDPHONE".into(),
                status: DeviceStatus::Unsupported { api_level: 25 }
            },
        ]
    );
}

fn device(serial: &str, status: DeviceStatus) -> Device {
    Device {
        serial: serial.into(),
        status,
    }
}

#[test]
fn selects_the_only_supported_device_automatically() {
    let devices = [
        device("emulator-5554", DeviceStatus::Supported { api_level: 34 }),
        device("R5CT1234ABC", DeviceStatus::Unauthorized),
    ];

    assert_eq!(auto_select(&devices), Some("emulator-5554"));
}

#[test]
fn leaves_the_choice_to_the_user_when_several_devices_are_supported() {
    let devices = [
        device("emulator-5554", DeviceStatus::Supported { api_level: 34 }),
        device("emulator-5556", DeviceStatus::Supported { api_level: 30 }),
    ];

    assert_eq!(auto_select(&devices), None);
}

#[test]
fn never_selects_a_device_older_than_android_8() {
    let devices = [device(
        "ZY22OLDPHONE",
        DeviceStatus::Unsupported { api_level: 25 },
    )];

    assert_eq!(auto_select(&devices), None);
}
