//! Provider management – interact with proxy and rule providers via the API.
//!
//! Providers are mihomo's mechanism for dynamically updating proxy node lists
//! and rule sets from remote URLs. This module wraps the API client to offer
//! higher-level operations like bulk updates and status summarisation.

use std::collections::HashMap;

use anyhow::{Context, Result};

use crate::api::client::MihomoClient;
use crate::api::types::{ProxyProviderItem, RuleProviderItem};

// ── Status types ───────────────────────────────────────────────────────────

/// Summary status for a single proxy provider.
#[derive(Debug, Clone)]
pub struct ProxyProviderStatus {
    pub name: String,
    pub provider_type: String,
    pub vehicle_type: String,
    pub node_count: usize,
    pub last_updated: String,
    /// Number of alive (healthy) nodes.
    pub alive_count: usize,
    /// Total nodes.
    pub total_count: usize,
    /// Subscription info if available.
    pub subscription_info: Option<SubscriptionStatus>,
}

/// Summary status for a single rule provider.
#[derive(Debug, Clone)]
pub struct RuleProviderStatus {
    pub name: String,
    pub provider_type: String,
    pub behavior: String,
    pub vehicle_type: String,
    pub rule_count: i64,
    pub last_updated: String,
}

/// Subscription traffic info.
#[derive(Debug, Clone)]
pub struct SubscriptionStatus {
    pub upload: u64,
    pub download: u64,
    pub total: u64,
    /// Remaining bytes.
    pub remaining: u64,
    /// Usage percentage (0.0 – 100.0).
    pub usage_percent: f64,
    /// Expiry timestamp if available.
    pub expire: Option<u64>,
}

/// Aggregate result of updating all providers.
#[derive(Debug, Clone)]
pub struct BulkUpdateResult {
    pub updated: Vec<String>,
    pub failed: HashMap<String, String>,
}

// ── Provider Manager ───────────────────────────────────────────────────────

/// High-level provider management using the mihomo API.
pub struct ProviderManager {
    client: MihomoClient,
}

impl ProviderManager {
    /// Create a new provider manager wrapping an API client.
    pub fn new(client: MihomoClient) -> Self {
        Self { client }
    }

    // ── List providers ─────────────────────────────────────────────────────

    /// List all proxy providers with full details.
    pub async fn list_proxy_providers(&self) -> Result<HashMap<String, ProxyProviderItem>> {
        let resp = self
            .client
            .get_proxy_providers()
            .await
            .context("failed to fetch proxy providers")?;
        Ok(resp.providers)
    }

    /// List all rule providers with full details.
    pub async fn list_rule_providers(&self) -> Result<HashMap<String, RuleProviderItem>> {
        let resp = self
            .client
            .get_rule_providers()
            .await
            .context("failed to fetch rule providers")?;
        Ok(resp.providers)
    }

    // ── Status ─────────────────────────────────────────────────────────────

    /// Get summarised status for all proxy providers.
    pub async fn proxy_provider_statuses(&self) -> Result<Vec<ProxyProviderStatus>> {
        let providers = self.list_proxy_providers().await?;
        let mut statuses = Vec::new();

        for (_key, item) in providers {
            let total_count = item.proxies.len();
            let alive_count = item.proxies.iter().filter(|p| p.alive).count();

            let subscription_info = item.subscription_info.or(item.sub_info).map(|info| {
                let used = info.upload + info.download;
                let remaining = info.total.saturating_sub(used);
                let usage_percent = if info.total > 0 {
                    (used as f64 / info.total as f64) * 100.0
                } else {
                    0.0
                };
                SubscriptionStatus {
                    upload: info.upload,
                    download: info.download,
                    total: info.total,
                    remaining,
                    usage_percent,
                    expire: info.expire,
                }
            });

            statuses.push(ProxyProviderStatus {
                name: item.name.clone(),
                provider_type: item.provider_type,
                vehicle_type: item.vehicle_type,
                node_count: total_count,
                last_updated: item.updated.clone(),
                alive_count,
                total_count,
                subscription_info,
            });
        }

        // Sort by name.
        statuses.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(statuses)
    }

    /// Get summarised status for all rule providers.
    pub async fn rule_provider_statuses(&self) -> Result<Vec<RuleProviderStatus>> {
        let providers = self.list_rule_providers().await?;
        let mut statuses = Vec::new();

        for (_key, item) in providers {
            statuses.push(RuleProviderStatus {
                name: item.name.clone(),
                provider_type: item.provider_type,
                behavior: item.behavior,
                vehicle_type: item.vehicle_type,
                rule_count: item.rule_count,
                last_updated: item.updated.clone(),
            });
        }

        statuses.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(statuses)
    }

    // ── Update individual providers ────────────────────────────────────────

    /// Update (refresh) a single proxy provider.
    pub async fn update_proxy_provider(&self, name: &str) -> Result<()> {
        self.client
            .update_proxy_provider(name)
            .await
            .with_context(|| format!("failed to update proxy provider: {name}"))
    }

    /// Update (refresh) a single rule provider.
    pub async fn update_rule_provider(&self, name: &str) -> Result<()> {
        self.client
            .update_rule_provider(name)
            .await
            .with_context(|| format!("failed to update rule provider: {name}"))
    }

    /// Run a health check on a proxy provider.
    pub async fn healthcheck_proxy_provider(&self, name: &str) -> Result<()> {
        self.client
            .proxy_provider_healthcheck(name)
            .await
            .with_context(|| format!("failed to health-check proxy provider: {name}"))
    }

    // ── Bulk operations ────────────────────────────────────────────────────

    /// Update all proxy providers.
    pub async fn update_all_proxy_providers(&self) -> Result<BulkUpdateResult> {
        let providers = self.list_proxy_providers().await?;
        let mut updated = Vec::new();
        let mut failed = HashMap::new();

        for (name, _item) in providers {
            match self.update_proxy_provider(&name).await {
                Ok(()) => updated.push(name),
                Err(e) => {
                    failed.insert(name, e.to_string());
                }
            }
        }

        Ok(BulkUpdateResult { updated, failed })
    }

    /// Update all rule providers.
    pub async fn update_all_rule_providers(&self) -> Result<BulkUpdateResult> {
        let providers = self.list_rule_providers().await?;
        let mut updated = Vec::new();
        let mut failed = HashMap::new();

        for (name, _item) in providers {
            match self.update_rule_provider(&name).await {
                Ok(()) => updated.push(name),
                Err(e) => {
                    failed.insert(name, e.to_string());
                }
            }
        }

        Ok(BulkUpdateResult { updated, failed })
    }

    /// Update all providers (both proxy and rule).
    pub async fn update_all(&self) -> (BulkUpdateResult, BulkUpdateResult) {
        let proxy_result = self
            .update_all_proxy_providers()
            .await
            .unwrap_or(BulkUpdateResult {
                updated: Vec::new(),
                failed: {
                    let mut m = HashMap::new();
                    m.insert("(all)".to_owned(), "failed to list providers".to_owned());
                    m
                },
            });

        let rule_result = self
            .update_all_rule_providers()
            .await
            .unwrap_or(BulkUpdateResult {
                updated: Vec::new(),
                failed: {
                    let mut m = HashMap::new();
                    m.insert("(all)".to_owned(), "failed to list providers".to_owned());
                    m
                },
            });

        (proxy_result, rule_result)
    }
}
