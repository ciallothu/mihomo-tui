//! WebSocket client for streaming mihomo endpoints.
//!
//! The mihomo external controller exposes three streaming WebSocket endpoints:
//!
//! | Endpoint    | Payload type           |
//! |-------------|------------------------|
//! | `/traffic`  | [`TrafficData`]        |
//! | `/logs`     | [`LogEntry`]           |
//! | `/memory`   | [`MemoryData`]         |
//!
//! Each function returns a [`futures_util::Stream`] yielding decoded items,
//! making it trivial to plug into a Tokio select-loop or TUI event loop.

use std::pin::Pin;

use anyhow::{Context, Result};
use futures_util::{Stream, StreamExt};
use tokio_tungstenite::{connect_async, tungstenite};

use super::types::{LogEntry, MemoryData, TrafficData};

// ── Helpers ─────────────────────────────────────────────────────────────────

/// Build a WebSocket URL from an HTTP base URL + path.
fn ws_url(base_url: &str, path: &str, secret: &str) -> String {
    let base = base_url
        .replace("http://", "ws://")
        .replace("https://", "wss://");
    let trimmed = base.trim_end_matches('/');
    if secret.is_empty() {
        format!("{trimmed}{path}")
    } else {
        format!("{trimmed}{path}?token={secret}")
    }
}

/// Type-erased boxed stream for returning from async functions.
type BoxStream<T> = Pin<Box<dyn Stream<Item = Result<T>> + Send>>;

/// Connect to a WebSocket endpoint and return a typed message stream.
async fn connect_stream<T>(base_url: &str, path: &str, secret: &str) -> Result<BoxStream<T>>
where
    T: serde::de::DeserializeOwned + Send + 'static,
{
    let url = ws_url(base_url, path, secret);
    let (ws_stream, _resp) = connect_async(&url)
        .await
        .with_context(|| format!("WebSocket connect failed: {url}"))?;

    let stream = ws_stream.filter_map(|msg| async move {
        match msg {
            Ok(tungstenite::Message::Text(text)) => match serde_json::from_str::<T>(&text) {
                Ok(item) => Some(Ok(item)),
                Err(e) => {
                    log::trace!("WS parse error: {e} – raw: {text}");
                    None
                }
            },
            Ok(tungstenite::Message::Close(_)) => None,
            Ok(_) => None,
            Err(e) => {
                log::warn!("WS read error: {e}");
                None
            }
        }
    });

    Ok(Box::pin(stream))
}

// ── Public API ──────────────────────────────────────────────────────────────

/// Connect to `/traffic` and stream real-time bandwidth samples.
pub async fn traffic_stream(base_url: &str, secret: &str) -> Result<BoxStream<TrafficData>> {
    connect_stream(base_url, "/traffic", secret).await
}

/// Connect to `/logs` and stream log entries.
///
/// `level` is an optional log-level filter (e.g. `"info"`).
pub async fn log_stream(
    base_url: &str,
    secret: &str,
    level: Option<&str>,
) -> Result<BoxStream<LogEntry>> {
    let path = match level {
        Some(lvl) => format!("/logs?level={lvl}"),
        None => "/logs".to_owned(),
    };
    connect_stream(base_url, &path, secret).await
}

/// Connect to `/memory` and stream memory-usage samples.
pub async fn memory_stream(base_url: &str, secret: &str) -> Result<BoxStream<MemoryData>> {
    connect_stream(base_url, "/memory", secret).await
}
