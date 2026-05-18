//! Error types for mihomo-tui.
//!
//! Defines application-wide error variants using [`thiserror`] for ergonomic
//! error handling, plus a [`Result`] alias used throughout the crate.

use std::path::PathBuf;

// ── Error enum ──────────────────────────────────────────────────────────────

/// Top-level error type for the application.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    // ── Network / API ───────────────────────────────────────────────────────
    /// HTTP request failed (reqwest-level error).
    #[error("HTTP request error: {0}")]
    Http(#[from] reqwest::Error),

    /// The mihomo API returned a non-2xx status code.
    #[error("API error ({status}): {message}")]
    Api { status: u16, message: String },

    /// Failed to connect or communicate over WebSocket.
    #[error("WebSocket error: {0}")]
    WebSocket(#[from] Box<tokio_tungstenite::tungstenite::Error>),

    // ── Serialisation ───────────────────────────────────────────────────────
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("YAML error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    // ── Configuration ───────────────────────────────────────────────────────
    #[error("Configuration error: {0}")]
    Config(String),

    /// I/O error when reading / writing config or data files.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// A required file was not found.
    #[error("File not found: {0}")]
    FileNotFound(PathBuf),

    // ── CLI / argument parsing ──────────────────────────────────────────────
    #[error("Argument error: {0}")]
    Arg(String),

    // ── General ─────────────────────────────────────────────────────────────
    #[error("{0}")]
    Other(String),
}

/// Convenience alias used across the crate.
pub type Result<T> = std::result::Result<T, AppError>;
