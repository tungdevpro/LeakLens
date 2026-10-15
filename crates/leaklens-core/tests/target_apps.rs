use leaklens_core::fake::FakeAdb;
use leaklens_core::{TargetAppChoices, list_target_apps};

fn fixture(name: &str) -> String {
    std::fs::read_to_string(format!(
        "{}/tests/fixtures/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

fn device_with_screen(activity_dump: &str) -> FakeAdb {
    FakeAdb::new()
        .respond(
            "-s emulator-5554 shell pm list packages -3",
            fixture("pm_list_packages_3.txt"),
        )
        .respond(
            "-s emulator-5554 shell dumpsys activity activities",
            fixture(activity_dump),
        )
}

#[tokio::test]
async fn lists_third_party_apps_with_the_app_on_screen_preselected() {
    let adb = device_with_screen("dumpsys_activity_api34.txt");

    let choices = list_target_apps(&adb, "emulator-5554").await.unwrap();

    assert_eq!(
        choices,
        TargetAppChoices {
            packages: vec![
                "com.example.notes".into(),
                "com.example.shop".into(),
                "com.whatsapp".into()
            ],
            foreground: Some("com.example.shop".into()),
        }
    );
}

#[tokio::test]
async fn finds_the_app_on_screen_on_android_9() {
    let adb = device_with_screen("dumpsys_activity_api28.txt");

    let choices = list_target_apps(&adb, "emulator-5554").await.unwrap();

    assert_eq!(choices.foreground.as_deref(), Some("com.example.notes"));
}

#[tokio::test]
async fn preselects_nothing_when_the_app_on_screen_is_not_a_third_party_app() {
    let adb = device_with_screen("dumpsys_activity_launcher.txt");

    let choices = list_target_apps(&adb, "emulator-5554").await.unwrap();

    assert_eq!(choices.foreground, None);
}

#[tokio::test]
async fn reads_real_output_captured_from_an_android_17_emulator() {
    let adb = FakeAdb::new()
        .respond(
            "-s emulator-5554 shell pm list packages -3",
            fixture("pm_list_packages_3_api37_real.txt"),
        )
        .respond(
            "-s emulator-5554 shell dumpsys activity activities",
            fixture("dumpsys_activity_api37_real.txt"),
        );

    let choices = list_target_apps(&adb, "emulator-5554").await.unwrap();

    assert_eq!(
        choices,
        TargetAppChoices {
            packages: vec![
                "com.andrewpack.fata".into(),
                "com.tokaiopt.app.tokaiapp".into(),
                "dev.mobile.maestro".into(),
                "dev.mobile.maestro.test".into(),
            ],
            foreground: Some("com.andrewpack.fata".into()),
        }
    );
}
