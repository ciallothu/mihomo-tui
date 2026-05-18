//! Asynchronous mihomo REST API client.
//!
//! [`MihomoClient`] wraps [`reqwest::Client`] and exposes typed methods for
//! every endpoint offered by the mihomo external-controller. Each method
//! returns an `anyhow::Result`, keeping caller code clean.

use anyhow::{Context, Result};
use reqwest::Method;

use super::types::*;

// ── Client ──────────────────────────────────────────────────────────────────

/// HTTP client for the mihomo external controller API.
#[derive(Debug, Clone)]
pub struct MihomoClient {
    http: reqwest::Client,
    base_url: String,
    secret: String,
}

impl MihomoClient {
    // ── Constructor ─────────────────────────────────────────────────────────

    /// Create a new client.
    ///
    /// * `base_url` – e.g. `http://127.0.0.1:9090`
    /// * `secret`   – Bearer token (may be empty).
    pub fn new(base_url: &str, secret: &str) -> Self {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .expect("failed to build reqwest client");

        Self {
            http,
            base_url: base_url.trim_end_matches('/').to_owned(),
            secret: secret.to_owned(),
        }
    }

    // ── Internal helpers ────────────────────────────────────────────────────

    /// Build an authenticated request builder.
    fn request(&self, method: Method, path: &str) -> reqwest::RequestBuilder {
        let url = format!("{}{path}", self.base_url);
        let mut rb = self.http.request(method, &url);
        if !self.secret.is_empty() {
            rb = rb.bearer_auth(&self.secret);
        }
        rb
    }

    /// Execute a request and return the raw response, raising on non-2xx.
    async fn send(&self, rb: reqwest::RequestBuilder) -> Result<reqwest::Response> {
        let resp = rb.send().await.context("HTTP request failed")?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("API error (HTTP {status}): {body}");
        }
        Ok(resp)
    }

    // ═══════════════════════════════════════════════════════════════════════
    // Proxies
    // ═══════════════════════════════════════════════════════════════════════

    /// `GET /proxies` – list all proxies.
    pub async fn get_proxies(&self) -> Result<ProxiesResponse> {
        let resp = self.send(self.request(Method::GET, "/proxies")).await?;
        resp.json().await.context("failed to parse proxies")
    }

    /// `GET /proxies/:name` – get a single proxy.
    pub async fn get_proxy(&self, name: &str) -> Result<ProxyItem> {
        let encoded = urlencoding::encode(name);
        let resp = self
            .send(self.request(Method::GET, &format!("/proxies/{encoded}")))
            .await?;
        resp.json().await.context("failed to parse proxy")
    }

    /// `PUT /proxies/:name` – switch the active node in a selector group.
    pub async fn switch_proxy(&self, group: &str, name: &str) -> Result<()> {
        let encoded = urlencoding::encode(group);
        let body = SwitchProxyRequest {
            name: name.to_owned(),
        };
        self.send(
            self.request(Method::PUT, &format!("/proxies/{encoded}"))
                .json(&body),
        )
        .await?;
        Ok(())
    }

    /// `GET /proxies/:name/delay?url=…&timeout=…` – test proxy delay.
    pub async fn get_proxy_delay(
        &self,
        name: &str,
        test_url: &str,
        timeout: u64,
    ) -> Result<ProxyDelayResponse> {
        let encoded = urlencoding::encode(name);
        let resp = self
            .send(
                self.request(Method::GET, &format!("/proxies/{encoded}/delay"))
                    .query(&[("url", test_url), ("timeout", &timeout.to_string())]),
            )
            .await?;
        resp.json().await.context("failed to parse delay")
    }

    // ═══════════════════════════════════════════════════════════════════════
    // Connections
    // ═══════════════════════════════════════════════════════════════════════

    /// `GET /connections` – list all active connections.
    pub async fn get_connections(&self) -> Result<ConnectionsResponse> {
        let resp = self.send(self.request(Method::GET, "/connections")).await?;
        resp.json().await.context("failed to parse connections")
    }

    /// `DELETE /connections` – close all connections.
    pub async fn close_all_connections(&self) -> Result<()> {
        self.send(self.request(Method::DELETE, "/connections"))
            .await?;
        Ok(())
    }

    /// `DELETE /connections/:id` – close a single connection.
    pub async fn close_connection(&self, id: &str) -> Result<()> {
        self.send(self.request(Method::DELETE, &format!("/connections/{id}")))
            .await?;
        Ok(())
    }

    // ═══════════════════════════════════════════════════════════════════════
    // Rules
    // ═══════════════════════════════════════════════════════════════════════

    /// `GET /rules` – list all rules.
    pub async fn get_rules(&self) -> Result<RulesResponse> {
        let resp = self.send(self.request(Method::GET, "/rules")).await?;
        resp.json().await.context("failed to parse rules")
    }

    // ═══════════════════════════════════════════════════════════════════════
    // Proxy Providers
    // ═══════════════════════════════════════════════════════════════════════

    /// `GET /providers/proxies` – list all proxy providers.
    pub async fn get_proxy_providers(&self) -> Result<ProxyProvidersResponse> {
        let resp = self
            .send(self.request(Method::GET, "/providers/proxies"))
            .await?;
        resp.json().await.context("failed to parse proxy providers")
    }

    /// `PUT /providers/proxies/:name` – health-check / update a proxy provider.
    pub async fn update_proxy_provider(&self, name: &str) -> Result<()> {
        let encoded = urlencoding::encode(name);
        self.send(
            self.request(Method::PUT, &format!("/providers/proxies/{encoded}"))
                .body(""),
        )
        .await?;
        Ok(())
    }

    /// `GET /providers/proxies/:name/healthcheck` – run health check.
    pub async fn proxy_provider_healthcheck(&self, name: &str) -> Result<()> {
        let encoded = urlencoding::encode(name);
        self.send(self.request(
            Method::GET,
            &format!("/providers/proxies/{encoded}/healthcheck"),
        ))
        .await?;
        Ok(())
    }

    // ═══════════════════════════════════════════════════════════════════════
    // Rule Providers
    // ═══════════════════════════════════════════════════════════════════════

    /// `GET /providers/rules` – list all rule providers.
    pub async fn get_rule_providers(&self) -> Result<RuleProvidersResponse> {
        let resp = self
            .send(self.request(Method::GET, "/providers/rules"))
            .await?;
        resp.json().await.context("failed to parse rule providers")
    }

    /// `PUT /providers/rules/:name` – update a rule provider.
    pub async fn update_rule_provider(&self, name: &str) -> Result<()> {
        let encoded = urlencoding::encode(name);
        self.send(
            self.request(Method::PUT, &format!("/providers/rules/{encoded}"))
                .body(""),
        )
        .await?;
        Ok(())
    }

    // ═══════════════════════════════════════════════════════════════════════
    // Configs
    // ═══════════════════════════════════════════════════════════════════════

    /// `GET /configs` – retrieve running configuration.
    pub async fn get_configs(&self) -> Result<ConfigResponse> {
        let resp = self.send(self.request(Method::GET, "/configs")).await?;
        resp.json().await.context("failed to parse configs")
    }

    /// `PATCH /configs` – patch running configuration (mode, log-level, etc.).
    pub async fn patch_configs(&self, patch: &PatchConfigRequest) -> Result<()> {
        self.send(self.request(Method::PATCH, "/configs").json(patch))
            .await?;
        Ok(())
    }

    /// `PUT /configs` – reload configuration from a file path.
    pub async fn reload_configs(&self, path: &str) -> Result<()> {
        let body = ReloadConfigRequest {
            path: path.to_owned(),
            payload: None,
        };
        self.send(self.request(Method::PUT, "/configs").json(&body))
            .await?;
        Ok(())
    }

    // ═══════════════════════════════════════════════════════════════════════
    // Version
    // ═══════════════════════════════════════════════════════════════════════

    /// `GET /version` – retrieve mihomo version information.
    pub async fn get_version(&self) -> Result<VersionResponse> {
        let resp = self.send(self.request(Method::GET, "/version")).await?;
        resp.json().await.context("failed to parse version")
    }

    // ═══════════════════════════════════════════════════════════════════════
    // Convenience
    // ═══════════════════════════════════════════════════════════════════════

    /// Return the WebSocket base URL (`ws://` or `wss://` derived from the
    /// HTTP base URL). This is used by [`super::websocket`].
    pub fn ws_base_url(&self) -> String {
        self.base_url
            .replace("http://", "ws://")
            .replace("https://", "wss://")
    }

    /// Return a reference to the secret string (for WebSocket auth).
    pub fn secret(&self) -> &str {
        &self.secret
    }

    /// Simple liveness check – returns `Ok(())` if the API is reachable.
    pub async fn check_alive(&self) -> Result<()> {
        self.get_version().await?;
        Ok(())
    }
}
