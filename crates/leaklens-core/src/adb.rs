use std::future::Future;

/// Why a command sent to a Device through adb did not produce output.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AdbError {
    #[error("could not run adb: {0}")]
    Spawn(String),
    #[error("adb exited with an error: {0}")]
    Failed(String),
}

/// Everything LeakLens needs from a Device goes through this one command:
/// adb arguments in, text output out.
pub trait Adb {
    fn run(&self, args: &[&str]) -> impl Future<Output = Result<String, AdbError>> + Send;
}

/// Runs the real `adb` program once per command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdbBinary {
    path: std::path::PathBuf,
}

impl AdbBinary {
    pub fn new(path: std::path::PathBuf) -> Self {
        Self { path }
    }
}

impl Adb for AdbBinary {
    async fn run(&self, args: &[&str]) -> Result<String, AdbError> {
        let output = tokio::process::Command::new(&self.path)
            .args(args)
            .kill_on_drop(true)
            .output()
            .await
            .map_err(|e| AdbError::Spawn(e.to_string()))?;
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).into_owned())
        } else {
            Err(AdbError::Failed(
                String::from_utf8_lossy(&output.stderr).trim().to_owned(),
            ))
        }
    }
}
