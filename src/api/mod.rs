//! mihomo API client, types, and WebSocket support.
//!
//! - [`types`]  – Serde-compatible structs for every mihomo REST API response.
//! - [`client`] – Asynchronous HTTP client wrapping `reqwest`.
//! - [`websocket`] – Streaming WebSocket client using `tokio-tungstenite`.

pub mod client;
pub mod types;
pub mod websocket;
