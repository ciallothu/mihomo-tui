//! Subscription management – add, remove, fetch, and parse proxy subscriptions.
//!
//! Subscriptions are remote URLs that serve proxy configurations as YAML or
//! base64-encoded text. This module handles:
//!
//! - Storing subscription metadata locally
//! - Fetching and decoding subscription content
//! - Parsing proxy lists from subscription responses

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ── Data types ─────────────────────────────────────────────────────────────

/// Metadata for a single subscription.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subscription {
    /// Unique identifier (derived from URL hash or user-provided name).
    pub id: String,
    /// User-visible name.
    pub name: String,
    /// Subscription URL.
    pub url: String,
    /// Optional update interval in seconds.
    pub interval: Option<u64>,
    /// Last successful update time (UTC).
    pub last_update: Option<DateTime<Utc>>,
    /// Whether this subscription is currently active / enabled.
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

/// The persisted subscription store (serialized as YAML).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SubscriptionStore {
    pub subscriptions: Vec<Subscription>,
}

/// A parsed proxy node extracted from a subscription.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyNode {
    /// Node name from the subscription.
    pub name: String,
    /// Proxy type (ss, vmess, vless, trojan, etc.).
    pub proxy_type: String,
    /// Server address.
    pub server: String,
    /// Server port.
    pub port: u16,
    /// Raw YAML string for this proxy entry.
    pub raw: String,
}

// ── Subscription Manager ───────────────────────────────────────────────────

/// Manages proxy subscriptions: CRUD, fetching, and parsing.
pub struct SubscriptionManager {
    /// Path to the subscription store file.
    store_path: PathBuf,
    /// HTTP client for fetching subscriptions.
    http: reqwest::Client,
    /// In-memory store.
    store: SubscriptionStore,
}

impl SubscriptionManager {
    /// Create a new manager. The store file is stored in `app_data_dir`.
    pub fn new(app_data_dir: &Path) -> Result<Self> {
        let store_path = app_data_dir.join("subscriptions.yaml");
        let http = reqwest::Client::builder()
            .user_agent("mihomo-tui")
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .context("failed to build HTTP client")?;

        let store = if store_path.exists() {
            let raw =
                std::fs::read_to_string(&store_path).context("cannot read subscription store")?;
            serde_yaml::from_str(&raw).context("cannot parse subscription store")?
        } else {
            SubscriptionStore::default()
        };

        Ok(Self {
            store_path,
            http,
            store,
        })
    }

    // ── Persistence ────────────────────────────────────────────────────────

    /// Save the subscription store to disk.
    pub fn save(&self) -> Result<()> {
        if let Some(parent) = self.store_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let yaml = serde_yaml::to_string(&self.store).context("cannot serialize subscriptions")?;
        std::fs::write(&self.store_path, yaml)
            .with_context(|| format!("cannot write {}", self.store_path.display()))?;
        Ok(())
    }

    // ── CRUD ───────────────────────────────────────────────────────────────

    /// Add a new subscription.
    ///
    /// Returns the generated subscription ID.
    pub fn add(&mut self, name: String, url: String, interval: Option<u64>) -> Result<String> {
        // Check for duplicate URL.
        if self.store.subscriptions.iter().any(|s| s.url == url) {
            bail!("subscription with this URL already exists");
        }

        let id = Self::generate_id(&url);
        let sub = Subscription {
            id: id.clone(),
            name,
            url,
            interval,
            last_update: None,
            enabled: true,
        };
        self.store.subscriptions.push(sub);
        self.save()?;
        Ok(id)
    }

    /// Remove a subscription by ID.
    pub fn remove(&mut self, id: &str) -> Result<()> {
        let before = self.store.subscriptions.len();
        self.store.subscriptions.retain(|s| s.id != id);
        if self.store.subscriptions.len() == before {
            bail!("subscription not found: {id}");
        }
        self.save()?;
        Ok(())
    }

    /// Update the name, URL, or interval of a subscription.
    pub fn update(
        &mut self,
        id: &str,
        name: Option<String>,
        url: Option<String>,
        interval: Option<Option<u64>>,
    ) -> Result<()> {
        let sub = self
            .store
            .subscriptions
            .iter_mut()
            .find(|s| s.id == id)
            .context("subscription not found")?;
        if let Some(n) = name {
            sub.name = n;
        }
        if let Some(u) = url {
            sub.url = u;
        }
        if let Some(i) = interval {
            sub.interval = i;
        }
        self.save()?;
        Ok(())
    }

    /// Get a subscription by ID.
    pub fn get(&self, id: &str) -> Option<&Subscription> {
        self.store.subscriptions.iter().find(|s| s.id == id)
    }

    /// List all subscriptions.
    pub fn list(&self) -> &[Subscription] {
        &self.store.subscriptions
    }

    /// Enable or disable a subscription.
    pub fn set_enabled(&mut self, id: &str, enabled: bool) -> Result<()> {
        let sub = self
            .store
            .subscriptions
            .iter_mut()
            .find(|s| s.id == id)
            .context("subscription not found")?;
        sub.enabled = enabled;
        self.save()?;
        Ok(())
    }

    // ── Fetching & Parsing ─────────────────────────────────────────────────

    /// Fetch a subscription by ID and return the raw response text.
    ///
    /// Also updates `last_update` on success.
    pub async fn fetch(&mut self, id: &str) -> Result<String> {
        let url = {
            let sub = self
                .store
                .subscriptions
                .iter()
                .find(|s| s.id == id)
                .context("subscription not found")?;
            sub.url.clone()
        };

        let resp = self
            .http
            .get(&url)
            .send()
            .await
            .with_context(|| format!("failed to fetch subscription from {url}"))?;

        if !resp.status().is_success() {
            bail!(
                "subscription fetch failed (HTTP {}): {}",
                resp.status(),
                url
            );
        }

        let text = resp
            .text()
            .await
            .context("failed to read subscription response")?;

        // Update last_update timestamp.
        let sub = self
            .store
            .subscriptions
            .iter_mut()
            .find(|s| s.id == id)
            .unwrap();
        sub.last_update = Some(Utc::now());
        self.save()?;

        Ok(text)
    }

    /// Fetch and parse a subscription, returning a list of proxy nodes.
    ///
    /// Handles both raw YAML and base64-encoded subscriptions.
    pub async fn fetch_and_parse(&mut self, id: &str) -> Result<Vec<ProxyNode>> {
        let raw = self.fetch(id).await?;
        Self::parse_subscription(&raw)
    }

    /// Refresh all enabled subscriptions.
    ///
    /// Returns a map of subscription ID → parse result.
    pub async fn refresh_all(&mut self) -> HashMap<String, Result<Vec<ProxyNode>>> {
        let ids: Vec<String> = self
            .store
            .subscriptions
            .iter()
            .filter(|s| s.enabled)
            .map(|s| s.id.clone())
            .collect();

        let mut results = HashMap::new();
        for id in ids {
            let result = self.fetch_and_parse(&id).await;
            results.insert(id, result);
        }
        results
    }

    // ── Parsing helpers ────────────────────────────────────────────────────

    /// Parse a subscription response body into proxy nodes.
    ///
    /// Tries YAML first; if that fails, attempts base64 decode then YAML parse.
    pub fn parse_subscription(raw: &str) -> Result<Vec<ProxyNode>> {
        let trimmed = raw.trim();

        // Attempt direct YAML parse.
        if let Ok(nodes) = Self::parse_yaml_proxies(trimmed) {
            return Ok(nodes);
        }

        // Attempt base64 decode → YAML.
        if let Ok(decoded) = decode_base64(trimmed) {
            if let Ok(nodes) = Self::parse_yaml_proxies(&decoded) {
                return Ok(nodes);
            }
            // Base64 decoded content might be a list of URI lines (ss://, vmess://, etc.)
            return Self::parse_uri_list(&decoded);
        }

        bail!("unable to parse subscription: not valid YAML or base64-encoded content")
    }

    /// Parse YAML subscription content (proxies: list).
    fn parse_yaml_proxies(content: &str) -> Result<Vec<ProxyNode>> {
        let value: serde_yaml::Value =
            serde_yaml::from_str(content).context("YAML parse failed")?;

        let proxies = value
            .get("proxies")
            .and_then(|v| v.as_sequence())
            .context("no 'proxies' key found in YAML")?;

        let mut nodes = Vec::new();
        for proxy in proxies {
            let name = proxy
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_owned();
            let proxy_type = proxy
                .get("type")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_owned();
            let server = proxy
                .get("server")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_owned();
            let port = proxy.get("port").and_then(|v| v.as_u64()).unwrap_or(0) as u16;

            let raw = serde_yaml::to_string(&proxy).unwrap_or_default();

            nodes.push(ProxyNode {
                name,
                proxy_type,
                server,
                port,
                raw,
            });
        }
        Ok(nodes)
    }

    /// Parse a newline-separated list of proxy URIs (ss://, vmess://, etc.).
    fn parse_uri_list(content: &str) -> Result<Vec<ProxyNode>> {
        let mut nodes = Vec::new();
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if let Some(rest) = line.strip_prefix("ss://") {
                nodes.push(ProxyNode {
                    name: "ss-node".into(),
                    proxy_type: "ss".into(),
                    server: String::new(),
                    port: 0,
                    raw: format!("ss://{rest}"),
                });
            } else if let Some(rest) = line.strip_prefix("vmess://") {
                nodes.push(ProxyNode {
                    name: "vmess-node".into(),
                    proxy_type: "vmess".into(),
                    server: String::new(),
                    port: 0,
                    raw: format!("vmess://{rest}"),
                });
            } else if let Some(rest) = line.strip_prefix("vless://") {
                nodes.push(ProxyNode {
                    name: "vless-node".into(),
                    proxy_type: "vless".into(),
                    server: String::new(),
                    port: 0,
                    raw: format!("vless://{rest}"),
                });
            } else if let Some(rest) = line.strip_prefix("trojan://") {
                nodes.push(ProxyNode {
                    name: "trojan-node".into(),
                    proxy_type: "trojan".into(),
                    server: String::new(),
                    port: 0,
                    raw: format!("trojan://{rest}"),
                });
            }
        }
        Ok(nodes)
    }

    // ── Helpers ────────────────────────────────────────────────────────────

    /// Generate a short ID from a URL for use as a subscription identifier.
    fn generate_id(url: &str) -> String {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        url.hash(&mut hasher);
        format!("{:016x}", hasher.finish())
    }
}

// ── Base64 decoding (no external crate needed) ─────────────────────────────

/// Decode a base64-encoded string to UTF-8, tolerating missing padding.
fn decode_base64(input: &str) -> Result<String> {
    let clean: String = input.chars().filter(|c| !c.is_whitespace()).collect();
    // Add padding if needed.
    let padded = {
        let pad = (4 - clean.len() % 4) % 4;
        let mut s = clean;
        for _ in 0..pad {
            s.push('=');
        }
        s
    };

    const DECODE_TABLE: [i8; 128] = [
        -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1,
        -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, 62, -1, -1, -1,
        63, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, -1, -1, -1, -1, -1, -1, -1, 0, 1, 2, 3, 4, 5,
        6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, -1, -1, -1, -1,
        -1, -1, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46,
        47, 48, 49, 50, 51, -1, -1, -1, -1, -1, -1,
    ];

    let bytes: Vec<u8> = padded
        .as_bytes()
        .chunks(4)
        .flat_map(|chunk| {
            let mut buf = [0u8; 3];
            let mut valid = 0usize;
            let mut tmp = [0u32; 4];
            for (i, &b) in chunk.iter().enumerate() {
                let v = if (b as usize) < DECODE_TABLE.len() {
                    DECODE_TABLE[b as usize]
                } else {
                    -1
                };
                if v < 0 {
                    tmp[i] = 0;
                } else {
                    tmp[i] = v as u32;
                    valid = i + 1;
                }
            }
            if valid > 1 {
                buf[0] = ((tmp[0] << 2) | (tmp[1] >> 4)) as u8;
            }
            if valid > 2 {
                buf[1] = (((tmp[1] & 0xF) << 4) | (tmp[2] >> 2)) as u8;
            }
            if valid > 3 {
                buf[2] = (((tmp[2] & 0x3) << 6) | tmp[3]) as u8;
            }
            let out_len = if valid == 0 { 0 } else { valid - 1 };
            buf[..out_len].to_vec()
        })
        .collect();

    String::from_utf8(bytes).context("base64 result is not valid UTF-8")
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_id_deterministic() {
        let id1 = SubscriptionManager::generate_id("https://example.com/sub");
        let id2 = SubscriptionManager::generate_id("https://example.com/sub");
        assert_eq!(id1, id2);
    }

    #[test]
    fn generate_id_different_urls() {
        let id1 = SubscriptionManager::generate_id("https://example.com/a");
        let id2 = SubscriptionManager::generate_id("https://example.com/b");
        assert_ne!(id1, id2);
    }

    #[test]
    fn parse_yaml_proxies_basic() {
        let yaml = r#"
proxies:
  - name: "node-1"
    type: ss
    server: 1.2.3.4
    port: 443
    cipher: aes-256-gcm
    password: "test"
  - name: "node-2"
    type: vmess
    server: 5.6.7.8
    port: 8080
    uuid: abcd
    alterId: 0
    cipher: auto
"#;
        let nodes = SubscriptionManager::parse_subscription(yaml).unwrap();
        assert_eq!(nodes.len(), 2);
        assert_eq!(nodes[0].name, "node-1");
        assert_eq!(nodes[0].proxy_type, "ss");
        assert_eq!(nodes[0].server, "1.2.3.4");
        assert_eq!(nodes[0].port, 443);
        assert_eq!(nodes[1].name, "node-2");
    }

    #[test]
    fn decode_base64_basic() {
        let encoded = "SGVsbG8gV29ybGQ=";
        let decoded = decode_base64(encoded).unwrap();
        assert_eq!(decoded, "Hello World");
    }

    #[test]
    fn decode_base64_no_padding() {
        // "Hello World" without the trailing =
        let encoded = "SGVsbG8gV29ybGQ";
        let decoded = decode_base64(encoded).unwrap();
        assert_eq!(decoded, "Hello World");
    }
}
