use std::time::Duration;

use leaklens_core::fake::FakeAdb;
use leaklens_core::{SamplingIntervals, Session, SessionState};

fn fixture(name: &str) -> String {
    std::fs::read_to_string(format!(
        "{}/tests/fixtures/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

const PIDOF: &str = "-s emulator-5554 shell pidof com.andrewpack.fata";
const READ_CPU: &str = "-s emulator-5554 shell cat /proc/2767/stat /proc/stat";

/// A Device running the Target App as PID 2767, replaying two real CPU readings.
fn device_running_target_app() -> FakeAdb {
    FakeAdb::new()
        .respond(PIDOF, fixture("pidof_api37_real.txt"))
        .respond_seq(
            READ_CPU,
            [
                fixture("proc_stat_api37_real_1.txt"),
                fixture("proc_stat_api37_real_2.txt"),
            ],
        )
}

/// `count` CPU readings one second apart in which the Target App uses
/// 10 ticks while 4 cores advance 400 ticks together: 10% of one core.
fn steady_cpu_readings(count: u64) -> Vec<String> {
    (0..count)
        .map(|i| {
            let process = 100 + 10 * i;
            let idle = 1000 + 400 * i;
            format!(
                "2767 (andrewpack.fata) S 427 427 0 0 -1 4194624 0 0 0 0 {process} 0 0 0 10 -10 34 0 852 0 0\n\
                 cpu  0 0 0 {idle} 0 0 0 0 0 0\n\
                 cpu0 0 0 0 0 0 0 0 0 0 0\ncpu1 0 0 0 0 0 0 0 0 0 0\n\
                 cpu2 0 0 0 0 0 0 0 0 0 0\ncpu3 0 0 0 0 0 0 0 0 0 0\n"
            )
        })
        .collect()
}

fn device_with_steady_cpu() -> FakeAdb {
    FakeAdb::new()
        .respond(PIDOF, "2767\n")
        .respond_seq(READ_CPU, steady_cpu_readings(100))
}

fn start(adb: &FakeAdb) -> Session {
    Session::start(
        adb.clone(),
        "emulator-5554",
        "com.andrewpack.fata",
        SamplingIntervals::default(),
    )
}

#[tokio::test(start_paused = true)]
async fn samples_cpu_usage_as_a_percentage_of_one_core() {
    let session = start(&device_running_target_app());

    tokio::time::sleep(Duration::from_millis(1500)).await;

    let samples = session.samples();
    assert_eq!(samples.len(), 1, "{samples:?}");
    assert_eq!(samples[0].at, Duration::from_secs(1));
    // Worked by hand from the fixtures: the process used 17 ticks while each
    // of the 4 cores advanced 270 / 4 = 67.5 ticks, so 17 / 67.5 = 25.19%.
    let cpu = samples[0].cpu_usage.unwrap();
    assert!((cpu - 25.19).abs() < 0.01, "{cpu}");
}

#[tokio::test(start_paused = true)]
async fn stopping_ends_sampling_and_keeps_the_samples() {
    let adb = device_with_steady_cpu();
    let mut session = start(&adb);
    tokio::time::sleep(Duration::from_millis(3500)).await;

    session.stop();
    assert_eq!(session.state(), SessionState::Stopped);
    let reads_at_stop = adb.calls(READ_CPU).len();
    tokio::time::sleep(Duration::from_secs(10)).await;

    assert_eq!(session.state(), SessionState::Stopped);
    assert_eq!(session.samples().len(), 3);
    assert_eq!(adb.calls(READ_CPU).len(), reads_at_stop);
}

#[tokio::test(start_paused = true)]
async fn a_slow_read_never_overlaps_the_next_and_skips_the_ticks_it_covered() {
    let adb = device_with_steady_cpu().delay(READ_CPU, Duration::from_millis(1500));
    let started = tokio::time::Instant::now();
    let _session = start(&adb);

    tokio::time::sleep(Duration::from_millis(9900)).await;

    let read_times: Vec<Duration> = adb.calls(READ_CPU).iter().map(|at| *at - started).collect();
    let secs = |s: &[u64]| {
        s.iter()
            .map(|&s| Duration::from_secs(s))
            .collect::<Vec<_>>()
    };
    assert_eq!(read_times, secs(&[0, 2, 4, 6, 8]));
    assert_eq!(adb.max_concurrent_calls(), 1);
}

#[tokio::test(start_paused = true)]
async fn tells_subscribers_when_a_new_sample_arrives() {
    let session = start(&device_with_steady_cpu());
    let mut updates = session.subscribe();

    updates.changed().await.unwrap();

    assert_eq!(session.samples().len(), 1);
}
