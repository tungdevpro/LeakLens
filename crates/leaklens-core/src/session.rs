use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::sync::watch;
use tokio::task::JoinHandle;
use tokio::time::Instant;

use crate::adb::Adb;
use crate::cpu::CpuReading;

/// How often each metric family is read during a Session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SamplingIntervals {
    pub cpu: Duration,
}

impl Default for SamplingIntervals {
    fn default() -> Self {
        Self {
            cpu: Duration::from_secs(1),
        }
    }
}

/// One reading of the Target Process's metrics. Metric families not read at
/// this moment are `None`.
#[derive(Debug, Clone, PartialEq)]
pub struct Sample {
    /// Time since the Session started.
    pub at: Duration,
    /// Percentage of one core; 100.0 means one core fully busy.
    pub cpu_usage: Option<f64>,
}

/// One continuous monitoring run of a Target App on a Device.
///
/// Sampling runs on its own tokio task, so it keeps going while the UI is
/// paused (Dioxus stops its own tasks while the window is covered).
pub struct Session {
    samples: Arc<Mutex<Vec<Sample>>>,
    sampler: JoinHandle<()>,
    updates: watch::Receiver<usize>,
    state: SessionState,
}

impl Session {
    /// Starts sampling right away. Must be called inside a tokio runtime.
    pub fn start<A>(adb: A, device: &str, target_app: &str, intervals: SamplingIntervals) -> Self
    where
        A: Adb + Clone + Send + Sync + 'static,
    {
        let samples = Arc::new(Mutex::new(Vec::new()));
        let target = Target {
            adb,
            device: device.to_owned(),
            target_app: target_app.to_owned(),
        };
        let (notify, updates) = watch::channel(0);
        let recorder = Recorder {
            started: Instant::now(),
            samples: samples.clone(),
            notify,
        };
        let sampler = tokio::spawn(sample_cpu(target, intervals.cpu, recorder));
        Self {
            samples,
            sampler,
            updates,
            state: SessionState::Running,
        }
    }

    pub fn samples(&self) -> Vec<Sample> {
        self.samples.lock().unwrap().clone()
    }

    /// Changes each time a Sample is added, holding the number of Samples.
    /// `changed()` fails once the Session has stopped.
    pub fn subscribe(&self) -> watch::Receiver<usize> {
        self.updates.clone()
    }

    pub fn state(&self) -> SessionState {
        self.state
    }

    /// Ends sampling. The Samples taken so far are kept.
    pub fn stop(&mut self) {
        self.sampler.abort();
        self.state = SessionState::Stopped;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    Running,
    Stopped,
}

impl Drop for Session {
    fn drop(&mut self) {
        self.sampler.abort();
    }
}

struct Target<A> {
    adb: A,
    device: String,
    target_app: String,
}

impl<A: Adb> Target<A> {
    /// The PID of the Target App's main process, if it is running.
    async fn pid(&self) -> Option<String> {
        let output = self
            .adb
            .run(&["-s", &self.device, "shell", "pidof", &self.target_app])
            .await
            .ok()?;
        output.split_whitespace().next().map(str::to_owned)
    }

    async fn read_cpu(&self, pid: &str) -> Option<CpuReading> {
        let stat = format!("/proc/{pid}/stat");
        let output = self
            .adb
            .run(&["-s", &self.device, "shell", "cat", &stat, "/proc/stat"])
            .await
            .ok()?;
        CpuReading::parse(&output)
    }
}

/// Stores Samples and tells subscribers about them. Owned by the sampling
/// task, so dropping it on Stop ends the subscribers' updates.
struct Recorder {
    started: Instant,
    samples: Arc<Mutex<Vec<Sample>>>,
    notify: watch::Sender<usize>,
}

impl Recorder {
    fn record(&self, tick: Instant, cpu_usage: f64) {
        let count = {
            let mut samples = self.samples.lock().unwrap();
            samples.push(Sample {
                at: tick - self.started,
                cpu_usage: Some(cpu_usage),
            });
            samples.len()
        };
        self.notify.send_replace(count);
    }
}

async fn sample_cpu<A: Adb>(target: Target<A>, interval: Duration, recorder: Recorder) {
    let mut ticker = Ticker::new(recorder.started, interval);
    let mut pid = None;
    let mut previous: Option<CpuReading> = None;
    loop {
        let tick = ticker.tick().await;
        if pid.is_none() {
            pid = target.pid().await;
        }
        let Some(current_pid) = &pid else { continue };
        let Some(reading) = target.read_cpu(current_pid).await else {
            // The process may have gone; look it up again next time.
            pid = None;
            previous = None;
            continue;
        };
        if let Some(cpu_usage) = previous.and_then(|p| reading.usage_since(&p)) {
            recorder.record(tick, cpu_usage);
        }
        previous = Some(reading);
    }
}

/// Paces a sampling loop at a fixed interval, starting now. Reads never
/// overlap: if one overruns, the ticks it covered are skipped.
struct Ticker {
    interval: Duration,
    next: Instant,
}

impl Ticker {
    fn new(start: Instant, interval: Duration) -> Self {
        Self {
            interval,
            next: start,
        }
    }

    /// Waits for the next tick and returns when it was due.
    async fn tick(&mut self) -> Instant {
        let now = Instant::now();
        while self.next < now {
            self.next += self.interval;
        }
        tokio::time::sleep_until(self.next).await;
        let due = self.next;
        self.next += self.interval;
        due
    }
}
