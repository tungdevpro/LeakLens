mod chart;

use std::time::Duration;

use dioxus::prelude::*;
use leaklens_core::{
    AdbBinary, Device, DeviceStatus, MIN_API_LEVEL, Memory, Sample, SamplingIntervals, Session,
    SessionState, auto_select, find_adb, list_devices, list_target_apps,
};

use chart::{LineChart, Point, Series};

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
                Ok(adb) => rsx! { Monitor { adb } },
                Err(error) => rsx! { p { class: "error", "{error}" } },
            }
        }
    }
}

/// The Device and Target App pickers, Start/Stop, and the live charts.
#[component]
fn Monitor(adb: AdbBinary) -> Element {
    let selected_device = use_signal(|| None::<String>);
    let selected_app = use_signal(|| None::<String>);
    let mut session = use_signal(|| None::<Session>);
    let mut samples = use_signal(Vec::<Sample>::new);

    let running = session
        .read()
        .as_ref()
        .is_some_and(|s| s.state() == SessionState::Running);
    let can_start = selected_device().is_some() && selected_app().is_some();

    let start_adb = adb.clone();
    let start = move |_| {
        let (Some(device), Some(app)) = (selected_device(), selected_app()) else {
            return;
        };
        let started = Session::start(
            start_adb.clone(),
            &device,
            &app,
            SamplingIntervals::default(),
        );
        let mut updates = started.subscribe();
        samples.set(Vec::new());
        session.set(Some(started));
        // Copies new Samples into the UI. Sampling itself runs on its own tokio
        // task, so it carries on even while Dioxus pauses this one.
        spawn(async move {
            while updates.changed().await.is_ok() {
                if let Some(current) = &*session.peek() {
                    samples.set(current.samples());
                }
            }
        });
    };
    let stop = move |_| {
        if let Some(current) = session.write().as_mut() {
            current.stop();
            samples.set(current.samples());
        }
    };

    let intervals = SamplingIntervals::default();
    let samples = samples.read();
    let cpu = series(&samples, "CPU Usage", |s| s.cpu_usage);
    let memory = |name: &str, kb: fn(&Memory) -> Option<u64>| {
        series(&samples, name, |s| {
            s.memory.as_ref().and_then(kb).map(|kb| kb as f64 / 1024.0)
        })
    };
    let memory_series = vec![
        memory("Total PSS", Memory::total_pss_kb),
        memory("Java Heap", Memory::java_heap_kb),
        memory("Native Heap", Memory::native_heap_kb),
        memory("Graphics", Memory::graphics_kb),
    ];
    let count = |name: &str, read: fn(&Memory) -> Option<u64>| {
        series(&samples, name, |s| {
            s.memory.as_ref().and_then(read).map(|n| n as f64)
        })
    };
    let object_counts = [
        count("Activities", Memory::activities),
        count("Views", Memory::views),
        count("AppContexts", Memory::app_contexts),
    ];

    rsx! {
        div { class: "toolbar",
            Pickers { adb, selected_device, selected_app, locked: running }
            if running {
                button { class: "stop", onclick: stop, "Stop" }
            } else {
                button { class: "start", disabled: !can_start, onclick: start, "Start" }
            }
        }
        if session.read().is_some() {
            LineChart {
                title: "CPU Usage",
                unit: "%",
                decimals: 1,
                y_floor: 100.0,
                max_gap: max_gap(intervals.cpu),
                series: vec![cpu],
            }
            LineChart {
                title: "Memory",
                unit: " MB",
                decimals: 1,
                y_floor: 10.0,
                max_gap: max_gap(intervals.memory),
                series: memory_series,
            }
            section { class: "small-multiples",
                h2 { "Object Counts" }
                div { class: "small-multiples-row",
                    for s in object_counts {
                        LineChart {
                            title: s.name.clone(),
                            unit: "",
                            y_floor: 4.0,
                            max_gap: max_gap(intervals.memory),
                            width: 300.0,
                            height: 150.0,
                            series: vec![s],
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn Pickers(
    adb: AdbBinary,
    selected_device: Signal<Option<String>>,
    selected_app: Signal<Option<String>>,
    locked: bool,
) -> Element {
    let mut devices = use_signal(Vec::<Device>::new);
    let mut device_error = use_signal(|| None::<String>);

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
                    disabled: locked,
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
                    disabled: locked || selected_device().is_none(),
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

/// One chart line from the Samples that carry a value for it.
fn series(samples: &[Sample], name: &str, value: impl Fn(&Sample) -> Option<f64>) -> Series {
    let points = samples
        .iter()
        .filter_map(|s| value(s).map(|value| Point { at: s.at, value }))
        .collect();
    Series {
        name: name.to_owned(),
        points,
    }
}

/// Points of one metric family further apart than this are drawn as a gap.
fn max_gap(interval: Duration) -> Duration {
    interval.mul_f64(2.5)
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
