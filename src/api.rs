use std::time::Duration;

use anyhow::{Context, Result};
use chrono::Local;
use futures_util::StreamExt;
use reqwest::{Client, Url};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::time::timeout;
use tokio_tungstenite::connect_async;

use crate::{
    core::MihomoMode,
    panel::{
        ConnectionInfo, ExternalResource, ListenerPorts, LogEntry, PanelSnapshot, ProxyGroup,
        ProxyNode, ResourceKind, TrafficStats, log,
    },
};

#[derive(Debug, Clone)]
pub struct MihomoClient {
    base_url: Url,
    secret: Option<String>,
    http: Client,
}

impl MihomoClient {
    pub fn new(base_url: impl AsRef<str>, secret: Option<String>) -> Result<Self> {
        let base_url = normalize_base_url(base_url.as_ref())?;
        Ok(Self {
            base_url,
            secret: secret.filter(|value| !value.is_empty()),
            http: Client::new(),
        })
    }

    pub fn base_url(&self) -> &Url {
        &self.base_url
    }

    pub async fn snapshot(&self) -> Result<(PanelSnapshot, MihomoMode, String)> {
        let (configs, proxies, proxy_providers, rule_providers, connections, version, memory, logs) = tokio::join!(
            self.configs(),
            self.proxies(),
            self.proxy_providers(),
            self.rule_providers(),
            self.connections(),
            self.version(),
            self.memory(),
            self.logs()
        );

        let configs = configs.context("load mihomo /configs")?;
        let proxies = proxies.context("load mihomo /proxies")?;
        let connections = connections.context("load mihomo /connections")?;
        let version = version.unwrap_or_else(|_| "unknown".to_string());

        let mut resources = Vec::new();
        if let Ok(providers) = proxy_providers {
            resources.extend(providers);
        }
        if let Ok(providers) = rule_providers {
            resources.extend(providers);
        }

        let mut entries = logs.unwrap_or_default();
        entries.push(log("info", format!("connected to {}", self.base_url)));

        let mode = configs.mode.unwrap_or(MihomoMode::Rule);
        let snapshot = PanelSnapshot {
            ports: configs.ports(),
            traffic: TrafficStats {
                upload_bps: connections.upload_total,
                download_bps: connections.download_total,
                memory_mb: memory.unwrap_or(0) / 1024,
                active_connections: connections.connections.len(),
            },
            groups: proxies.into_groups(),
            resources,
            connections: connections.into_connections(),
            logs: entries,
        };

        Ok((snapshot, mode, version))
    }

    pub async fn set_mode(&self, mode: MihomoMode) -> Result<()> {
        self.patch_json("configs", &json!({ "mode": mode.api_value() }))
            .await
            .context("switch mihomo mode")
    }

    pub async fn set_port(&self, key: &str, port: u16) -> Result<()> {
        let payload = if key == "external-controller" {
            json!({ key: format!("127.0.0.1:{port}") })
        } else {
            json!({ key: port })
        };
        self.patch_json("configs", &payload)
            .await
            .with_context(|| format!("update {key}"))
    }

    pub async fn select_proxy(&self, group: &str, proxy: &str) -> Result<()> {
        let path = format!("proxies/{}", encode(group));
        self.put_json(&path, &json!({ "name": proxy }))
            .await
            .with_context(|| format!("select proxy {proxy} in {group}"))
    }

    pub async fn refresh_resource(&self, resource: &ExternalResource) -> Result<()> {
        match resource.kind {
            ResourceKind::ProxyProvider => {
                let path = format!("providers/proxies/{}", encode(&resource.name));
                self.put_empty(&path)
                    .await
                    .with_context(|| format!("refresh proxy provider {}", resource.name))
            }
            ResourceKind::RuleProvider => {
                let path = format!("providers/rules/{}", encode(&resource.name));
                self.put_empty(&path)
                    .await
                    .with_context(|| format!("refresh rule provider {}", resource.name))
            }
        }
    }

    pub async fn close_connection(&self, id: &str) -> Result<()> {
        let path = format!("connections/{}", encode(id));
        self.delete(&path)
            .await
            .with_context(|| format!("close connection {id}"))
    }

    pub async fn close_all_connections(&self) -> Result<()> {
        self.delete("connections")
            .await
            .context("close all connections")
    }

    async fn configs(&self) -> Result<ConfigResponse> {
        self.get_json("configs").await
    }

    async fn proxies(&self) -> Result<ProxiesResponse> {
        self.get_json("proxies").await
    }

    async fn proxy_providers(&self) -> Result<Vec<ExternalResource>> {
        let response: ProvidersResponse = self.get_json("providers/proxies").await?;
        Ok(response
            .providers
            .into_iter()
            .map(|(name, provider)| ExternalResource {
                name,
                url: provider
                    .vehicle_type
                    .or(provider.provider_type)
                    .unwrap_or_else(|| "proxy-provider".to_string()),
                kind: ResourceKind::ProxyProvider,
                updated_at: provider.updated_at,
            })
            .collect())
    }

    async fn rule_providers(&self) -> Result<Vec<ExternalResource>> {
        let response: ProvidersResponse = self.get_json("providers/rules").await?;
        Ok(response
            .providers
            .into_iter()
            .map(|(name, provider)| ExternalResource {
                name,
                url: provider
                    .vehicle_type
                    .or(provider.provider_type)
                    .unwrap_or_else(|| "rule-provider".to_string()),
                kind: ResourceKind::RuleProvider,
                updated_at: provider.updated_at,
            })
            .collect())
    }

    async fn connections(&self) -> Result<ConnectionsResponse> {
        self.get_json("connections").await
    }

    async fn version(&self) -> Result<String> {
        let response: VersionResponse = self.get_json("version").await?;
        Ok(response.version.unwrap_or_else(|| "unknown".to_string()))
    }

    async fn memory(&self) -> Result<u64> {
        let response: MemoryResponse = self.get_json("memory").await?;
        Ok(response.inuse.or(response.in_use).unwrap_or_default())
    }

    async fn logs(&self) -> Result<Vec<LogEntry>> {
        let mut url = self.base_url.clone();
        url.set_path("logs");
        url.set_query(Some("level=info"));
        match url.scheme() {
            "https" => url.set_scheme("wss").ok(),
            _ => url.set_scheme("ws").ok(),
        };

        let mut request = url.as_str().into_client_request()?;
        if let Some(secret) = &self.secret {
            request.headers_mut().insert(
                "Authorization",
                format!("Bearer {secret}")
                    .parse()
                    .context("build auth header")?,
            );
        }

        let connect = timeout(Duration::from_millis(50), connect_async(request)).await;
        let Ok(Ok((mut socket, _))) = connect else {
            return Ok(Vec::new());
        };

        let mut entries = Vec::new();
        for _ in 0..10 {
            let next = timeout(Duration::from_millis(10), socket.next()).await;
            let Ok(Some(Ok(message))) = next else {
                break;
            };
            if let Ok(text) = message.to_text() {
                entries.push(parse_log_message(text));
            }
        }
        Ok(entries)
    }

    async fn get_json<T>(&self, path: &str) -> Result<T>
    where
        T: for<'de> Deserialize<'de>,
    {
        self.request(reqwest::Method::GET, path)
            .send()
            .await?
            .error_for_status()?
            .json::<T>()
            .await
            .with_context(|| format!("decode {path} response"))
    }

    async fn patch_json<T: Serialize + ?Sized>(&self, path: &str, body: &T) -> Result<()> {
        self.request(reqwest::Method::PATCH, path)
            .json(body)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    async fn put_json<T: Serialize + ?Sized>(&self, path: &str, body: &T) -> Result<()> {
        self.request(reqwest::Method::PUT, path)
            .json(body)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    async fn put_empty(&self, path: &str) -> Result<()> {
        self.request(reqwest::Method::PUT, path)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    async fn delete(&self, path: &str) -> Result<()> {
        self.request(reqwest::Method::DELETE, path)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    fn request(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        let url = self.base_url.join(path).expect("valid API path");
        let request = self.http.request(method, url);
        if let Some(secret) = &self.secret {
            request.bearer_auth(secret)
        } else {
            request
        }
    }
}

#[derive(Debug, Deserialize)]
struct ConfigResponse {
    mode: Option<MihomoMode>,
    port: Option<u16>,
    #[serde(rename = "socks-port")]
    socks_port: Option<u16>,
    #[serde(rename = "mixed-port")]
    mixed_port: Option<u16>,
    #[serde(rename = "redir-port")]
    redir_port: Option<u16>,
    #[serde(rename = "tproxy-port")]
    tproxy_port: Option<u16>,
    #[serde(rename = "external-controller")]
    external_controller: Option<String>,
}

impl ConfigResponse {
    fn ports(&self) -> ListenerPorts {
        ListenerPorts {
            mixed: self.mixed_port.unwrap_or(0),
            socks: self.socks_port.unwrap_or(0),
            http: self.port.unwrap_or(0),
            redir: self.redir_port,
            tproxy: self.tproxy_port,
            external_controller: self
                .external_controller
                .as_deref()
                .and_then(parse_port)
                .unwrap_or(0),
        }
    }
}

#[derive(Debug, Deserialize)]
struct ProxiesResponse {
    proxies: std::collections::BTreeMap<String, ProxyResponse>,
}

impl ProxiesResponse {
    fn into_groups(self) -> Vec<ProxyGroup> {
        self.proxies
            .into_iter()
            .filter_map(|(name, proxy)| {
                if proxy.all.is_empty() {
                    return None;
                }
                let selected = proxy
                    .now
                    .as_ref()
                    .and_then(|now| proxy.all.iter().position(|candidate| candidate == now))
                    .unwrap_or(0);
                let proxies = proxy
                    .all
                    .into_iter()
                    .map(|node| ProxyNode {
                        delay_ms: delay_for(&node, &proxy.history),
                        alive: true,
                        name: node,
                    })
                    .collect();
                Some(ProxyGroup {
                    name,
                    selected,
                    proxies,
                })
            })
            .collect()
    }
}

#[derive(Debug, Deserialize)]
struct ProxyResponse {
    now: Option<String>,
    #[serde(default)]
    all: Vec<String>,
    #[serde(default)]
    history: Vec<DelayHistory>,
}

#[derive(Debug, Deserialize)]
struct DelayHistory {
    #[serde(default)]
    delay: Option<u16>,
    #[serde(default)]
    mean_delay: Option<u16>,
}

#[derive(Debug, Deserialize)]
struct ProvidersResponse {
    providers: std::collections::BTreeMap<String, ProviderResponse>,
}

#[derive(Debug, Deserialize)]
struct ProviderResponse {
    #[serde(rename = "type")]
    provider_type: Option<String>,
    #[serde(rename = "vehicleType")]
    vehicle_type: Option<String>,
    #[serde(rename = "updatedAt")]
    updated_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ConnectionsResponse {
    #[serde(rename = "uploadTotal", default)]
    upload_total: u64,
    #[serde(rename = "downloadTotal", default)]
    download_total: u64,
    #[serde(default)]
    connections: Vec<ConnectionResponse>,
}

impl ConnectionsResponse {
    fn into_connections(self) -> Vec<ConnectionInfo> {
        self.connections
            .into_iter()
            .map(|connection| ConnectionInfo {
                id: connection.id,
                host: connection
                    .metadata
                    .as_ref()
                    .and_then(|metadata| metadata.host.clone().or(metadata.destination_ip.clone()))
                    .unwrap_or_else(|| "unknown".to_string()),
                rule: connection.rule.unwrap_or_else(|| "unknown".to_string()),
                chain: connection.chains.join(" -> "),
                upload: connection.upload,
                download: connection.download,
            })
            .collect()
    }
}

#[derive(Debug, Deserialize)]
struct ConnectionResponse {
    id: String,
    #[serde(default)]
    metadata: Option<ConnectionMetadata>,
    #[serde(default)]
    chains: Vec<String>,
    #[serde(default)]
    rule: Option<String>,
    #[serde(default)]
    upload: u64,
    #[serde(default)]
    download: u64,
}

#[derive(Debug, Deserialize)]
struct ConnectionMetadata {
    #[serde(default)]
    host: Option<String>,
    #[serde(rename = "destinationIP", default)]
    destination_ip: Option<String>,
}

#[derive(Debug, Deserialize)]
struct VersionResponse {
    version: Option<String>,
}

#[derive(Debug, Deserialize)]
struct MemoryResponse {
    #[serde(default)]
    inuse: Option<u64>,
    #[serde(rename = "inUse", default)]
    in_use: Option<u64>,
}

fn normalize_base_url(input: &str) -> Result<Url> {
    let with_scheme = if input.starts_with("http://") || input.starts_with("https://") {
        input.to_string()
    } else {
        format!("http://{input}")
    };
    let mut url = Url::parse(&with_scheme).context("parse controller URL")?;
    if !url.path().ends_with('/') {
        url.set_path(&format!("{}/", url.path().trim_end_matches('/')));
    }
    Ok(url)
}

fn encode(value: &str) -> String {
    urlencoding::encode(value).into_owned()
}

fn parse_port(value: &str) -> Option<u16> {
    value.rsplit_once(':')?.1.parse().ok()
}

fn delay_for(_name: &str, history: &[DelayHistory]) -> Option<u16> {
    history
        .iter()
        .rev()
        .find_map(|history| history.delay.or(history.mean_delay))
}

fn parse_log_message(text: &str) -> LogEntry {
    let Ok(value) = serde_json::from_str::<Value>(text) else {
        return log("info", text);
    };
    let level = value
        .get("type")
        .or_else(|| value.get("level"))
        .and_then(Value::as_str)
        .unwrap_or("info");
    let message = value
        .get("payload")
        .or_else(|| value.get("message"))
        .and_then(Value::as_str)
        .unwrap_or(text);
    LogEntry {
        level: level.to_string(),
        message: message.to_string(),
        time: Local::now(),
    }
}

trait IntoClientRequest {
    fn into_client_request(self) -> Result<http::Request<()>>;
}

impl IntoClientRequest for &str {
    fn into_client_request(self) -> Result<http::Request<()>> {
        Ok(http::Request::builder().uri(self).body(())?)
    }
}

mod http {
    pub use tokio_tungstenite::tungstenite::http::*;
}
