use std::time::Duration;

use leaklens_core::fake::FakeAdb;
use leaklens_core::{Memory, SamplingIntervals, Session, SessionState};

fn fixture(name: &str) -> String {
    std::fs::read_to_string(format!(
        "{}/tests/fixtures/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

const PIDOF: &str = "-s emulator-5554 shell pidof com.andrewpack.fata";
const READ_CPU: &str = "-s emulator-5554 shell cat /proc/2767/stat /proc/stat";
const READ_MEMORY: &str = "-s emulator-5554 shell dumpsys meminfo 2767";

/// A Device running the Target App as PID 2767, replaying two real CPU readings.
fn device_running_target_app() -> FakeAdb {
    FakeAdb::new()
        .respond(PIDOF, fixture("pidof_api37_real.txt"))
        .respond(READ_MEMORY, fixture("meminfo_api37_real.txt"))
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
        .respond(READ_MEMORY, fixture("meminfo_api37_real.txt"))
        .respond_seq(READ_CPU, steady_cpu_readings(100))
}

/// The CPU Usage Samples, leaving out other metric families.
fn cpu_samples(session: &Session) -> Vec<(Duration, f64)> {
    session
        .samples()
        .into_iter()
        .filter_map(|s| s.cpu_usage.map(|cpu| (s.at, cpu)))
        .collect()
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

    let samples = cpu_samples(&session);
    assert_eq!(samples.len(), 1, "{samples:?}");
    let (at, cpu) = samples[0];
    assert_eq!(at, Duration::from_secs(1));
    // Worked by hand from the fixtures: the process used 17 ticks while each
    // of the 4 cores advanced 270 / 4 = 67.5 ticks, so 17 / 67.5 = 25.19%.
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
    assert_eq!(cpu_samples(&session).len(), 3);
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
    assert_eq!(adb.max_concurrent_calls(READ_CPU), 1);
}

#[tokio::test(start_paused = true)]
async fn tells_subscribers_when_a_new_sample_arrives() {
    let session = start(&device_with_steady_cpu());
    let mut updates = session.subscribe();

    updates.changed().await.unwrap();

    assert_eq!(session.samples().len(), 1);
}

/// The first memory reading of a Session on a Device whose `meminfo` prints `meminfo_fixture`.
async fn first_memory_reading(meminfo_fixture: &str) -> Memory {
    let adb = device_with_steady_cpu().respond(READ_MEMORY, fixture(meminfo_fixture));
    let session = start(&adb);
    tokio::time::sleep(Duration::from_millis(100)).await;
    session
        .samples()
        .into_iter()
        .find_map(|s| s.memory)
        .expect("a memory Sample")
}

#[tokio::test(start_paused = true)]
async fn reads_memory_and_object_counts_on_android_17() {
    let memory = first_memory_reading("meminfo_api37_real.txt").await;

    assert_eq!(memory.total_pss_kb(), Some(155904));
    assert_eq!(memory.java_heap_kb(), Some(320));
    assert_eq!(memory.native_heap_kb(), Some(8576));
    assert_eq!(memory.graphics_kb(), Some(0));
    assert_eq!(memory.activities(), Some(1));
    assert_eq!(memory.views(), Some(6));
    assert_eq!(memory.app_contexts(), Some(6));
}

#[tokio::test(start_paused = true)]
async fn keeps_the_full_breakdown_with_values_in_the_right_column() {
    let memory = first_memory_reading("meminfo_api37_real.txt").await;

    assert_eq!(memory.pss_kb.get("System"), Some(&102784));
    assert_eq!(memory.pss_kb.get("Private Other"), Some(&36688));
    // "Unknown" only has an RSS value; it must not be mistaken for PSS.
    assert_eq!(memory.pss_kb.get("Unknown"), None);
    assert_eq!(memory.rss_kb.get("Unknown"), Some(&39440));
    assert_eq!(memory.rss_kb.get("Java Heap"), Some(&12928));
    assert_eq!(memory.objects.get("Proxy Binders"), Some(&60));
}

#[tokio::test(start_paused = true)]
async fn reads_memory_and_object_counts_on_android_12() {
    let memory = first_memory_reading("meminfo_api31_real.txt").await;

    assert_eq!(memory.total_pss_kb(), Some(131422));
    assert_eq!(memory.java_heap_kb(), Some(8988));
    assert_eq!(memory.native_heap_kb(), Some(46504));
    assert_eq!(memory.activities(), Some(2));
    assert_eq!(memory.views(), Some(193));
    assert_eq!(memory.app_contexts(), Some(8));
    assert_eq!(memory.objects.get("WebViews"), Some(&3));
}

#[tokio::test(start_paused = true)]
async fn reads_the_older_total_label_on_android_9() {
    let memory = first_memory_reading("meminfo_api28_handwritten.txt").await;

    assert_eq!(memory.total_pss_kb(), Some(37158));
    assert_eq!(memory.graphics_kb(), Some(9256));
    assert_eq!(memory.views(), Some(58));
}

fn seconds_since(started: tokio::time::Instant, calls: Vec<tokio::time::Instant>) -> Vec<u64> {
    calls.iter().map(|at| (*at - started).as_secs()).collect()
}

#[tokio::test(start_paused = true)]
async fn reads_memory_every_two_seconds_and_cpu_every_second() {
    let adb = device_with_steady_cpu();
    let started = tokio::time::Instant::now();
    let _session = start(&adb);

    tokio::time::sleep(Duration::from_millis(5500)).await;

    assert_eq!(seconds_since(started, adb.calls(READ_MEMORY)), [0, 2, 4]);
    assert_eq!(
        seconds_since(started, adb.calls(READ_CPU)),
        [0, 1, 2, 3, 4, 5]
    );
}

#[tokio::test(start_paused = true)]
async fn skips_memory_while_the_process_is_gone_and_looks_it_up_again() {
    let adb = device_with_steady_cpu().respond_seq(
        READ_MEMORY,
        [
            fixture("meminfo_api37_real.txt"),
            fixture("meminfo_no_process_real.txt"),
            fixture("meminfo_api37_real.txt"),
        ],
    );
    let started = tokio::time::Instant::now();
    let session = start(&adb);

    tokio::time::sleep(Duration::from_millis(4500)).await;

    let memory_times: Vec<Duration> = session
        .samples()
        .into_iter()
        .filter(|s| s.memory.is_some())
        .map(|s| s.at)
        .collect();
    assert_eq!(memory_times, [Duration::ZERO, Duration::from_secs(4)]);
    assert!(seconds_since(started, adb.calls(PIDOF)).contains(&4));
}
