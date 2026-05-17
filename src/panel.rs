use chrono::{DateTime, Local};

#[derive(Debug, Clone)]
pub struct ListenerPorts {
    pub mixed: u16,
    pub socks: u16,
    pub http: u16,
    pub redir: Option<u16>,
    pub tproxy: Option<u16>,
    pub external_controller: u16,
}

#[derive(Debug, Clone)]
pub struct TrafficStats {
    pub upload_bps: u64,
    pub download_bps: u64,
    pub memory_mb: u64,
    pub active_connections: usize,
}

#[derive(Debug, Clone)]
pub struct ProxyNode {
    pub name: String,
    pub delay_ms: Option<u16>,
    pub alive: bool,
}

#[derive(Debug, Clone)]
pub struct ProxyGroup {
    pub name: String,
    pub selected: usize,
    pub proxies: Vec<ProxyNode>,
}

#[derive(Debug, Clone)]
pub struct ExternalResource {
    pub name: String,
    pub url: String,
    pub kind: ResourceKind,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceKind {
    ProxyProvider,
    RuleProvider,
}

#[derive(Debug, Clone)]
pub struct ConnectionInfo {
    pub id: String,
    pub host: String,
    pub rule: String,
    pub chain: String,
    pub upload: u64,
    pub download: u64,
}

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub level: String,
    pub message: String,
    pub time: DateTime<Local>,
}

#[derive(Debug, Clone)]
pub struct PanelSnapshot {
    pub ports: ListenerPorts,
    pub traffic: TrafficStats,
    pub groups: Vec<ProxyGroup>,
    pub resources: Vec<ExternalResource>,
    pub connections: Vec<ConnectionInfo>,
    pub logs: Vec<LogEntry>,
}

impl PanelSnapshot {
    pub fn empty() -> Self {
        Self {
            ports: ListenerPorts {
                mixed: 0,
                socks: 0,
                http: 0,
                redir: None,
                tproxy: None,
                external_controller: 0,
            },
            traffic: TrafficStats {
                upload_bps: 0,
                download_bps: 0,
                memory_mb: 0,
                active_connections: 0,
            },
            groups: Vec::new(),
            resources: Vec::new(),
            connections: Vec::new(),
            logs: vec![log("info", "mihomo-tui workspace initialized")],
        }
    }
}

pub fn log(level: &str, message: impl Into<String>) -> LogEntry {
    LogEntry {
        level: level.to_string(),
        message: message.into(),
        time: Local::now(),
    }
}
