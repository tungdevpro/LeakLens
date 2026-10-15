use std::time::Duration;

use dioxus::prelude::*;
use leaklens_core::{
    AdbBinary, Device, DeviceStatus, MIN_API_LEVEL, auto_select, find_adb, list_devices,
    list_target_apps,
};

/// How often the Device list is refreshed, so plugging in or removing a Device shows up.
const DEVICE_POLL: Duration = Duration::from_secs(2);

const STYLE: &str = include_str!("style.css");

fn main() {
    let window = dioxus::desktop::WindowBuilder::new().with_title("LeakLens");
    dioxus::LaunchBuilder::desktop()
        .with_cfg(dioxus::desktop::Config::new().with_window(window))
        .launch(App);
}

#[component]
fn App() -> Element {
    let adb = use_hook(|| find_adb(|key| std::env::var_os(key)).map(AdbBinary::new));

    rsx! {
        style { {STYLE} }
        main {
            h1 { "LeakLens" }
            match adb {
                Ok(adb) => rsx! { Pickers { adb } },
                Err(error) => rsx! { p { class: "error", "{error}" } },
            }
        }
    }
}

#[component]
fn Pickers(adb: AdbBinary) -> Element {
    let mut devices = use_signal(Vec::<Device>::new);
    let mut device_error = use_signal(|| None::<String>);
    let mut selected_device = use_signal(|| None::<String>);
    let mut selected_app = use_signal(|| None::<String>);

    // Dioxus pauses its tasks while the window is covered and a redraw is pending,
    // so this list can lag until LeakLens is visible again. That's fine for picking a
    // Device, but anything that must keep running in the background needs `tokio::spawn`.
    let device_adb = adb.clone();
    use_future(move || {
        let adb = device_adb.clone();
        async move {
            loop {
                match list_devices(&adb).await {
                    Ok(found) => {
                        device_error.set(None);
                        let still_selectable =
                            selected_device.peek().as_ref().is_some_and(|serial| {
                                found
                                    .iter()
                                    .any(|d| &d.serial == serial && d.is_selectable())
                            });
                        if !still_selectable {
                            selected_device.set(auto_select(&found).map(str::to_owned));
                        }
                        devices.set(found);
                    }
                    Err(error) => device_error.set(Some(error.to_string())),
                }
                tokio::time::sleep(DEVICE_POLL).await;
            }
        }
    });

    let target_apps = use_resource(move || {
        let adb = adb.clone();
        let serial = selected_device();
        async move {
            match serial {
                Some(serial) => list_target_apps(&adb, &serial).await.map(Some),
                None => Ok(None),
            }
        }
    });

    // Pre-select the app on screen whenever a fresh list arrives.
    use_effect(move || {
        if let Some(Ok(choices)) = &*target_apps.read() {
            selected_app.set(choices.as_ref().and_then(|c| c.foreground.clone()));
        }
    });

    rsx! {
        section { class: "pickers",
            label {
                "Device"
                select {
                    onchange: move |e| selected_device.set(Some(e.value()).filter(|v| !v.is_empty())),
                    option { value: "", selected: selected_device().is_none(), "Choose a Device…" }
                    for device in devices() {
                        option {
                            value: "{device.serial}",
                            disabled: !device.is_selectable(),
                            selected: selected_device().as_deref() == Some(device.serial.as_str()),
                            {device_label(&device)}
                        }
                    }
                }
            }
            label {
                "Target App"
                select {
                    disabled: selected_device().is_none(),
                    onchange: move |e| selected_app.set(Some(e.value()).filter(|v| !v.is_empty())),
                    option { value: "", selected: selected_app().is_none(), "Choose a Target App…" }
                    if let Some(Ok(Some(choices))) = &*target_apps.read() {
                        for package in choices.packages.clone() {
                            option {
                                value: "{package}",
                                selected: selected_app().as_deref() == Some(package.as_str()),
                                "{package}"
                            }
                        }
                    }
                }
            }
        }
        if let Some(error) = device_error() {
            p { class: "error", "Could not list Devices: {error}" }
        } else if devices().is_empty() {
            p { class: "hint", "No Device connected. Plug in a phone with USB debugging on, or start an emulator." }
        }
        for device in devices().into_iter().filter(|d| matches!(d.status, DeviceStatus::Unsupported { .. })) {
            p { class: "error",
                "{device.serial} is unsupported: LeakLens needs Android 8 (API {MIN_API_LEVEL}) or later."
            }
        }
        if let Some(Err(error)) = &*target_apps.read() {
            p { class: "error", "Could not list Target Apps: {error}" }
        }
    }
}

fn device_label(device: &Device) -> String {
    match device.status {
        DeviceStatus::Supported { api_level } => format!("{} (API {api_level})", device.serial),
        DeviceStatus::Unsupported { api_level } => {
            format!("{} (API {api_level}, unsupported)", device.serial)
        }
        DeviceStatus::Unauthorized => format!(
            "{} (unauthorized: accept the prompt on the Device)",
            device.serial
        ),
        DeviceStatus::Offline => format!("{} (offline)", device.serial),
    }
}
