//! Comprehensive mihomo API response types.
//!
//! Every struct derives `Serialize` + `Deserialize` so they can be used for
//! both parsing responses *and* building request payloads where needed. Field
//! names follow the exact JSON keys returned by the mihomo external-controller
//! API.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

// ═══════════════════════════════════════════════════════════════════════════
// Proxies
// ═══════════════════════════════════════════════════════════════════════════

/// Wrapper for `GET /proxies`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxiesResponse {
    pub proxies: HashMap<String, ProxyItem>,
}

/// A single proxy node or group.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyItem {
    pub name: String,
    #[serde(default)]
    pub r#type: ProxyType,
    pub all: Option<Vec<String>>,
    pub now: Option<String>,
    pub history: Vec<ProxyHistoryEntry>,
    #[serde(default)]
    pub udp: bool,
    #[serde(default)]
    pub xudp: bool,
    #[serde(default)]
    pub tfo: bool,
    #[serde(default)]
    pub mp: bool,
    #[serde(default)]
    pub smux: bool,
    pub provider: Option<String>,
    #[serde(default)]
    pub alive: bool,
    pub extra: Option<ProxyExtra>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum ProxyType {
    #[serde(rename = "Shadowsocks")]
    Shadowsocks,
    #[serde(rename = "VMess")]
    Vmess,
    #[serde(rename = "VLESS")]
    Vless,
    #[serde(rename = "Trojan")]
    Trojan,
    #[serde(rename = "Hysteria")]
    Hysteria,
    #[serde(rename = "Hysteria2")]
    Hysteria2,
    #[serde(rename = "TUIC")]
    Tuic,
    #[serde(rename = "WireGuard")]
    Wireguard,
    #[serde(rename = "Snell")]
    Snell,
    #[serde(rename = "HTTP")]
    Http,
    #[serde(rename = "Socks5")]
    Socks5,
    #[serde(rename = "Relay")]
    Relay,
    #[serde(rename = "URLTest")]
    UrlTest,
    #[serde(rename = "Fallback")]
    Fallback,
    #[serde(rename = "LoadBalance")]
    LoadBalance,
    #[serde(rename = "Selector")]
    Selector,
    #[serde(rename = "Direct")]
    Direct,
    #[serde(rename = "Reject")]
    Reject,
    #[serde(rename = "Compatible")]
    Compatible,
    #[serde(rename = "Pass")]
    Pass,
    #[serde(rename = "Dns")]
    Dns,
    #[serde(rename = "Group")]
    Group,
    /// Catch-all for any new / unknown types.
    Unknown(String),
}

impl Default for ProxyType {
    fn default() -> Self {
        Self::Unknown("Unknown".into())
    }
}

/// A single delay-history sample for a proxy node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyHistoryEntry {
    pub time: String,
    pub delay: u64,
}

/// Extra metadata attached to proxy items (varies by type).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ProxyExtra {
    #[serde(default)]
    pub uploaded: u64,
    #[serde(default)]
    pub downloaded: u64,
    #[serde(default)]
    pub upload: u64,
    #[serde(default)]
    pub download: u64,
    #[serde(default)]
    pub total: u64,
}

/// Response for `GET /proxies/:name/delay`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyDelayResponse {
    pub delay: u64,
}

/// Request body for `PUT /proxies/:name` (switch selector).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwitchProxyRequest {
    pub name: String,
}

/// Query params for `GET /proxies/:name/delay`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct ProxyDelayQuery {
    pub url: String,
    pub timeout: u64,
}

// ═══════════════════════════════════════════════════════════════════════════
// Connections
// ═══════════════════════════════════════════════════════════════════════════

/// Wrapper for `GET /connections`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionsResponse {
    pub download_total: u64,
    pub upload_total: u64,
    pub connections: Option<Vec<ConnectionItem>>,
    pub memory: u64,
}

/// A single active connection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionItem {
    pub id: String,
    pub metadata: ConnectionMetadata,
    pub upload: u64,
    pub download: u64,
    pub start: String,
    pub chains: Vec<String>,
    pub rule: String,
    pub rule_payload: String,
    pub r#type: String, // e.g. "TCP", "UDP"
    pub host: String,
    pub process: Option<String>,
    pub process_path: Option<String>,
    pub dl_speed: u64,
    pub ul_speed: u64,
}

/// Connection metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionMetadata {
    pub network: String,
    pub r#type: String,
    pub source_ip: String,
    pub destination_ip: String,
    pub source_port: String,
    pub destination_port: String,
    pub host: String,
    pub dns_mode: String,
    pub process: Option<String>,
    pub process_path: Option<String>,
    pub uid: Option<u32>,
}

// ═══════════════════════════════════════════════════════════════════════════
// Rules
// ═══════════════════════════════════════════════════════════════════════════

/// Wrapper for `GET /rules`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RulesResponse {
    pub rules: Vec<RuleItem>,
}

/// A single rule entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleItem {
    #[serde(rename = "type")]
    pub rule_type: String,
    pub payload: String,
    pub proxy: String,
    pub size: Option<i64>,
}

// ═══════════════════════════════════════════════════════════════════════════
// Providers – Proxies
// ═══════════════════════════════════════════════════════════════════════════

/// Wrapper for `GET /providers/proxies`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyProvidersResponse {
    pub providers: HashMap<String, ProxyProviderItem>,
}

/// A proxy provider entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyProviderItem {
    pub name: String,
    #[serde(rename = "type")]
    pub provider_type: String,
    pub proxies: Vec<ProxyItem>,
    pub vehicle_type: String,
    pub sub_info: Option<ProviderSubInfo>,
    pub subscription_info: Option<ProviderSubInfo>,
    pub updated: String,
    pub expected_status: Option<String>,
}

/// Subscription info (remaining traffic / expiry).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderSubInfo {
    #[serde(default)]
    pub upload: u64,
    #[serde(default)]
    pub download: u64,
    #[serde(default)]
    pub total: u64,
    pub expire: Option<u64>,
}

/// Request body for `PUT /providers/proxies/:name`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct UpdateProxyProviderRequest {
    /// Empty body is acceptable for a simple health-check / update.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

// ═══════════════════════════════════════════════════════════════════════════
// Providers – Rules
// ═══════════════════════════════════════════════════════════════════════════

/// Wrapper for `GET /providers/rules`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleProvidersResponse {
    pub providers: HashMap<String, RuleProviderItem>,
}

/// A rule provider entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleProviderItem {
    pub name: String,
    #[serde(rename = "type")]
    pub provider_type: String,
    pub behavior: String,
    pub rule_count: i64,
    pub vehicle_type: String,
    pub updated: String,
}

// ═══════════════════════════════════════════════════════════════════════════
// Configs
// ═══════════════════════════════════════════════════════════════════════════

/// Response for `GET /configs`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigResponse {
    pub port: u16,
    #[serde(default)]
    pub socks_port: u16,
    #[serde(default)]
    pub mixed_port: u16,
    #[serde(default)]
    pub redir_port: u16,
    #[serde(default)]
    pub tproxy_port: u16,
    #[serde(default)]
    pub mitm_port: Option<u16>,
    #[serde(default)]
    pub allow_lan: bool,
    #[serde(default)]
    pub bind_address: String,
    #[serde(default)]
    pub log_level: String,
    #[serde(default)]
    pub ipv6: bool,
    #[serde(default)]
    pub mode: ClashMode,
    #[serde(default)]
    pub unified_delay: bool,
    #[serde(default)]
    pub tun: TunConfig,
    #[serde(default)]
    pub sniffer: Option<SnifferConfig>,
    #[serde(default)]
    pub dns: Option<DnsConfig>,
    #[serde(default)]
    pub geo_x_url: Option<GeoXUrl>,
    #[serde(default)]
    pub external_controller: String,
    #[serde(default)]
    pub external_ui: Option<String>,
    #[serde(default)]
    pub secret: String,
    #[serde(default)]
    pub interface_name: Option<String>,
}

/// Clash running mode.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum ClashMode {
    #[default]
    Rule,
    Global,
    Direct,
}

/// TUN configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub struct TunConfig {
    #[serde(default)]
    pub enable: bool,
    pub device: Option<String>,
    pub stack: Option<String>,
    #[serde(default)]
    pub dns_hijack: Vec<String>,
    #[serde(default)]
    pub auto_route: bool,
    #[serde(default)]
    pub auto_detect_interface: bool,
}

/// Sniffer configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct SnifferConfig {
    #[serde(default)]
    pub sniffing: bool,
    #[serde(default)]
    pub sniff: HashMap<String, bool>,
}

/// DNS configuration (simplified).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct DnsConfig {
    #[serde(default)]
    pub enable: bool,
    #[serde(default)]
    pub listen: String,
    #[serde(default)]
    pub enhanced_mode: String,
    #[serde(default)]
    pub nameserver: Vec<String>,
    #[serde(default)]
    pub fallback: Vec<String>,
}

/// Custom GeoIP / GeoSite download URLs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeoXUrl {
    pub geo_ip: Option<String>,
    pub geo_site: Option<String>,
    pub mmdb: Option<String>,
    pub asn: Option<String>,
}

/// Request body for `PATCH /configs`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PatchConfigRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<ClashMode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub log_level: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allow_lan: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ipv6: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unified_delay: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tun: Option<TunConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dns: Option<DnsConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sniffer: Option<SnifferConfig>,
}

/// Request body for `PUT /configs` (reload from path).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReloadConfigRequest {
    pub path: String,
    pub payload: Option<String>,
}

// ═══════════════════════════════════════════════════════════════════════════
// Version
// ═══════════════════════════════════════════════════════════════════════════

/// Response for `GET /version`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionResponse {
    pub version: String,
    pub meta: Option<bool>,
}

// ═══════════════════════════════════════════════════════════════════════════
// Streaming / WebSocket payloads
// ═══════════════════════════════════════════════════════════════════════════

/// Traffic sample pushed on `/traffic` WebSocket.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrafficData {
    pub up: u64,
    pub down: u64,
}

/// Memory usage sample pushed on `/memory` WebSocket.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryData {
    pub inuse: u64,
    pub oslimit: u64,
}

/// A single log entry pushed on `/logs` WebSocket.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    #[serde(rename = "type")]
    pub log_type: String,
    pub payload: String,
}

// ═══════════════════════════════════════════════════════════════════════════
// Grouped connection snapshot (used internally by the TUI)
// ═══════════════════════════════════════════════════════════════════════════

/// Snapshot combining connection list with aggregate bandwidth.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct ConnectionsSnapshot {
    pub upload_total: u64,
    pub download_total: u64,
    pub active: Vec<ConnectionItem>,
}
