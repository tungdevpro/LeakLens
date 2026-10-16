//! A scripted stand-in for adb, used by tests to replay captured Device output.
//!
//! Clones share one script and one call log, so a test can hand a clone to a
//! Session and still inspect what the Session asked for.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::time::Instant;

use crate::adb::{Adb, AdbError};

#[derive(Debug, Default, Clone)]
pub struct FakeAdb {
    state: Arc<Mutex<State>>,
}

#[derive(Debug, Default)]
struct State {
    scripts: HashMap<String, Script>,
    calls: Vec<(String, Instant)>,
    in_flight: usize,
    max_in_flight: usize,
}

#[derive(Debug, Default)]
struct Script {
    /// Replies in order; the last one repeats forever.
    replies: VecDeque<Result<String, AdbError>>,
    delay: Duration,
}

impl FakeAdb {
    pub fn new() -> Self {
        Self::default()
    }

    /// Replies with `output` every time it is called with `args` (space-separated).
    pub fn respond(self, args: &str, output: impl Into<String>) -> Self {
        self.script(args, [Ok(output.into())])
    }

    /// Replies with each of `outputs` in turn, then keeps repeating the last one.
    pub fn respond_seq(
        self,
        args: &str,
        outputs: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.script(args, outputs.into_iter().map(|o| Ok(o.into())))
    }

    /// Fails with `error` every time it is called with `args`.
    pub fn fail(self, args: &str, error: AdbError) -> Self {
        self.script(args, [Err(error)])
    }

    /// Makes every call with `args` take `delay` before replying.
    pub fn delay(self, args: &str, delay: Duration) -> Self {
        self.state
            .lock()
            .unwrap()
            .scripts
            .entry(args.to_owned())
            .or_default()
            .delay = delay;
        self
    }

    /// When each call with `args` started.
    pub fn calls(&self, args: &str) -> Vec<Instant> {
        let state = self.state.lock().unwrap();
        state
            .calls
            .iter()
            .filter(|(a, _)| a == args)
            .map(|(_, at)| *at)
            .collect()
    }

    /// The most calls that were ever running at the same time.
    pub fn max_concurrent_calls(&self) -> usize {
        self.state.lock().unwrap().max_in_flight
    }

    fn script(
        self,
        args: &str,
        replies: impl IntoIterator<Item = Result<String, AdbError>>,
    ) -> Self {
        let mut state = self.state.lock().unwrap();
        state.scripts.entry(args.to_owned()).or_default().replies = replies.into_iter().collect();
        drop(state);
        self
    }
}

impl Adb for FakeAdb {
    async fn run(&self, args: &[&str]) -> Result<String, AdbError> {
        let key = args.join(" ");
        let delay = {
            let mut state = self.state.lock().unwrap();
            let Some(script) = state.scripts.get(&key) else {
                panic!("FakeAdb has no scripted response for `adb {key}`");
            };
            let delay = script.delay;
            state.calls.push((key.clone(), Instant::now()));
            state.in_flight += 1;
            state.max_in_flight = state.max_in_flight.max(state.in_flight);
            delay
        };
        if !delay.is_zero() {
            tokio::time::sleep(delay).await;
        }
        let mut state = self.state.lock().unwrap();
        state.in_flight -= 1;
        let replies = &mut state.scripts.get_mut(&key).unwrap().replies;
        if replies.len() > 1 {
            replies.pop_front().unwrap()
        } else {
            replies[0].clone()
        }
    }
}
