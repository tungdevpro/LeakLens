//! A scripted stand-in for adb, used by tests to replay captured Device output.

use std::collections::HashMap;

use crate::adb::{Adb, AdbError};

#[derive(Debug, Default, Clone)]
pub struct FakeAdb {
    responses: HashMap<String, Result<String, AdbError>>,
}

impl FakeAdb {
    pub fn new() -> Self {
        Self::default()
    }

    /// Replies with `output` when called with `args` (space-separated).
    pub fn respond(mut self, args: &str, output: impl Into<String>) -> Self {
        self.responses.insert(args.to_owned(), Ok(output.into()));
        self
    }

    /// Fails with `error` when called with `args` (space-separated).
    pub fn fail(mut self, args: &str, error: AdbError) -> Self {
        self.responses.insert(args.to_owned(), Err(error));
        self
    }
}

impl Adb for FakeAdb {
    async fn run(&self, args: &[&str]) -> Result<String, AdbError> {
        let key = args.join(" ");
        match self.responses.get(&key) {
            Some(response) => response.clone(),
            None => panic!("FakeAdb has no scripted response for `adb {key}`"),
        }
    }
}
