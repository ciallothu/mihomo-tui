//! Mihomo subprocess management.
//!
//! Starts, stops, and monitors a mihomo kernel process managed by the TUI.

use std::path::PathBuf;
use std::process::Stdio;

use anyhow::{Context, Result, bail};

// ── Process manager ────────────────────────────────────────────────────────

/// Manages a mihomo child process.
pub struct MihomoProcess {
    /// Path to the mihomo binary.
    binary: PathBuf,
    /// Working directory for the process (where config.yaml lives).
    work_dir: PathBuf,
    /// The running child process, if any.
    child: Option<tokio::process::Child>,
}

impl MihomoProcess {
    /// Create a new process manager.
    pub fn new(binary: PathBuf, work_dir: PathBuf) -> Self {
        Self {
            binary,
            work_dir,
            child: None,
        }
    }

    /// Check if the process is still running.
    pub fn is_running(&mut self) -> bool {
        if let Some(ref mut child) = self.child {
            match child.try_wait() {
                Ok(Some(_)) => {
                    self.child = None;
                    false
                }
                Ok(None) => true,
                Err(_) => {
                    self.child = None;
                    false
                }
            }
        } else {
            false
        }
    }

    /// Start the mihomo process.
    pub async fn start(&mut self) -> Result<()> {
        if self.is_running() {
            return Ok(());
        }
        if !self.binary.exists() {
            bail!("kernel binary not found: {}", self.binary.display());
        }

        let child = tokio::process::Command::new(&self.binary)
            .arg("-d")
            .arg(&self.work_dir)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .with_context(|| format!("failed to start {}", self.binary.display()))?;

        self.child = Some(child);
        Ok(())
    }

    /// Stop the mihomo process.
    pub async fn stop(&mut self) -> Result<()> {
        if let Some(ref mut child) = self.child {
            child.kill().await.context("failed to kill mihomo")?;
            let _ = child.wait().await;
        }
        self.child = None;
        Ok(())
    }
}

impl Drop for MihomoProcess {
    fn drop(&mut self) {
        if let Some(ref mut child) = self.child {
            let _ = child.start_kill();
        }
    }
}
