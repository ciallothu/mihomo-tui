//! Config file management for mihomo.
//!
//! Handles listing, reading, writing, validating, and switching mihomo
//! configuration files stored on disk. The "active" config is the one that
//! mihomo is currently running with, and can be switched by reconfiguring
//! mihomo via its REST API.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::api::client::MihomoClient;

// ── Data types ─────────────────────────────────────────────────────────────

/// Metadata about a config file on disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigFileInfo {
    /// File name (e.g. `config.yaml`).
    pub name: String,
    /// Full path to the file.
    pub path: PathBuf,
    /// File size in bytes.
    pub size: u64,
    /// Last modification time (seconds since epoch).
    pub modified: Option<u64>,
}

/// Validation result for a mihomo config.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigValidation {
    /// Whether the config is valid.
    pub valid: bool,
    /// Human-readable warnings / errors.
    pub messages: Vec<String>,
}

// ── Config File Manager ────────────────────────────────────────────────────

/// Manages mihomo config files in the config directory.
pub struct ConfigFileManager {
    /// Path to the mihomo config directory.
    config_dir: PathBuf,
}

impl ConfigFileManager {
    /// Create a new manager pointing at the mihomo config directory.
    pub fn new(config_dir: &Path) -> Self {
        Self {
            config_dir: config_dir.to_owned(),
        }
    }

    /// Return the config directory path.
    pub fn config_dir(&self) -> &Path {
        &self.config_dir
    }

    // ── Listing ────────────────────────────────────────────────────────────

    /// List all `.yaml` and `.yml` config files in the config directory.
    ///
    /// Does **not** recurse into subdirectories.
    pub fn list_configs(&self) -> Result<Vec<ConfigFileInfo>> {
        if !self.config_dir.exists() {
            return Ok(Vec::new());
        }
        let mut configs = Vec::new();
        for entry in fs::read_dir(&self.config_dir)
            .with_context(|| format!("cannot read {}", self.config_dir.display()))?
        {
            let entry = entry?;
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if ext != "yaml" && ext != "yml" {
                continue;
            }
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("?")
                .to_owned();
            let meta = fs::metadata(&path).ok();
            let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
            let modified = meta
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs());

            configs.push(ConfigFileInfo {
                name,
                path,
                size,
                modified,
            });
        }
        configs.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(configs)
    }

    // ── Read / Write ───────────────────────────────────────────────────────

    /// Read a config file by name and return its contents as a string.
    pub fn read_config(&self, name: &str) -> Result<String> {
        let path = self.resolve_path(name)?;
        fs::read_to_string(&path)
            .with_context(|| format!("cannot read config file {}", path.display()))
    }

    /// Parse a config file into a `serde_yaml::Value`.
    pub fn read_config_yaml(&self, name: &str) -> Result<serde_yaml::Value> {
        let raw = self.read_config(name)?;
        serde_yaml::from_str(&raw)
            .with_context(|| format!("cannot parse YAML from config file: {name}"))
    }

    /// Write content to a config file by name.
    pub fn write_config(&self, name: &str, content: &str) -> Result<()> {
        let path = self.resolve_path_for_write(name)?;
        fs::write(&path, content)
            .with_context(|| format!("cannot write config file {}", path.display()))
    }

    /// Write a `serde_yaml::Value` to a config file.
    pub fn write_config_yaml(&self, name: &str, value: &serde_yaml::Value) -> Result<()> {
        let yaml = serde_yaml::to_string(value).context("cannot serialize config to YAML")?;
        self.write_config(name, &yaml)
    }

    // ── Validation ─────────────────────────────────────────────────────────

    /// Validate a config file's structure.
    ///
    /// Checks for required top-level keys and common structural issues.
    pub fn validate_config(&self, name: &str) -> Result<ConfigValidation> {
        let mut messages = Vec::new();
        let mut valid = true;

        let yaml_result = self.read_config_yaml(name);
        match yaml_result {
            Ok(value) => {
                // Check for essential top-level keys.
                let required_keys = ["port", "socks-port", "mixed-port"];
                let has_any_port = required_keys.iter().any(|k| value.get(*k).is_some());
                if !has_any_port {
                    messages.push(
                        "No listening port configured (missing port/socks-port/mixed-port)"
                            .to_owned(),
                    );
                    valid = false;
                }

                // Check proxies section.
                if let Some(proxies) = value.get("proxies").and_then(|v| v.as_sequence())
                    && proxies.is_empty()
                {
                    messages.push("'proxies' section is empty".to_owned());
                }

                // Check proxy-groups.
                if value.get("proxy-groups").is_none() && value.get("proxy-providers").is_none() {
                    messages
                        .push("No 'proxy-groups' or 'proxy-providers' section found".to_owned());
                }

                // Check rules.
                if value.get("rules").is_none() && value.get("rule-providers").is_none() {
                    messages.push("No 'rules' or 'rule-providers' section found".to_owned());
                }

                // Warn about missing dns section.
                if value.get("dns").is_none() {
                    messages.push("Warning: no 'dns' section configured".to_owned());
                }
            }
            Err(e) => {
                valid = false;
                messages.push(format!("Cannot parse YAML: {e}"));
            }
        }

        Ok(ConfigValidation { valid, messages })
    }

    // ── Active config ──────────────────────────────────────────────────────

    /// Switch the active config by telling mihomo to reload from a file path.
    pub async fn switch_active_config(&self, client: &MihomoClient, name: &str) -> Result<()> {
        let path = self.resolve_path(name)?;
        let path_str = path.to_str().context("config path is not valid UTF-8")?;
        client
            .reload_configs(path_str)
            .await
            .context("failed to reload config through API")?;
        Ok(())
    }

    /// Reload the current active config through the API.
    pub async fn reload_active_config(&self, client: &MihomoClient) -> Result<()> {
        let default_config = self.config_dir.join("config.yaml");
        if !default_config.exists() {
            bail!(
                "default config.yaml not found in {}",
                self.config_dir.display()
            );
        }
        let path_str = default_config
            .to_str()
            .context("config path is not valid UTF-8")?;
        client
            .reload_configs(path_str)
            .await
            .context("failed to reload config through API")?;
        Ok(())
    }

    // ── Template ───────────────────────────────────────────────────────────

    /// Create a minimal default config file from template.
    pub fn create_default_config(&self, name: &str) -> Result<PathBuf> {
        let template = Self::default_template();
        self.write_config(name, &template)?;
        self.resolve_path(name)
    }

    /// Default minimal mihomo config template.
    fn default_template() -> String {
        r#"# mihomo (Clash.Meta) configuration
# Generated by mihomo-tui

mixed-port: 7890
allow-lan: false
bind-address: '*'
mode: rule
log-level: info
ipv6: false
external-controller: 127.0.0.1:9090
# secret: ""

dns:
  enable: true
  listen: 0.0.0.0:1053
  enhanced-mode: fake-ip
  fake-ip-range: 198.18.0.1/16
  nameserver:
    - https://dns.alidns.com/dns-query
    - https://doh.pub/dns-query
  fallback:
    - https://dns.google/dns-query
    - https://cloudflare-dns.com/dns-query

proxies: []

proxy-groups: []

rules:
  - GEOIP,LAN,DIRECT
  - MATCH,DIRECT
"#
        .to_owned()
    }

    // ── Helpers ────────────────────────────────────────────────────────────

    /// Resolve a config name (or path) to an absolute path.
    ///
    /// Validates that the resolved path stays within the config directory.
    fn resolve_path(&self, name: &str) -> Result<PathBuf> {
        let path = self.config_dir.join(name);

        // Security: ensure the resolved path is within config_dir.
        let canonical_dir = self
            .config_dir
            .canonicalize()
            .unwrap_or_else(|_| self.config_dir.clone());
        let canonical_path = if path.exists() {
            path.canonicalize()
                .with_context(|| format!("cannot resolve path {}", path.display()))?
        } else {
            path.clone()
        };

        if !canonical_path.starts_with(&canonical_dir) {
            bail!("path traversal detected: {}", name);
        }

        if !path.exists() {
            bail!("config file not found: {}", path.display());
        }

        Ok(path)
    }

    /// Resolve a path for writing (file doesn't need to exist yet).
    fn resolve_path_for_write(&self, name: &str) -> Result<PathBuf> {
        let path = self.config_dir.join(name);

        // Basic security: no path traversal.
        if name.contains("..") {
            bail!("path traversal detected: {}", name);
        }

        // Ensure parent directory exists.
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        Ok(path)
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn list_configs_empty_dir() {
        let dir = std::env::temp_dir().join("mihomo_tui_test_list_empty");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let mgr = ConfigFileManager::new(&dir);
        let configs = mgr.list_configs().unwrap();
        assert!(configs.is_empty());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn list_configs_with_files() {
        let dir = std::env::temp_dir().join("mihomo_tui_test_list_files");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        fs::write(dir.join("config.yaml"), "port: 7890\n").unwrap();
        fs::write(dir.join("other.yml"), "mode: rule\n").unwrap();
        fs::write(dir.join("readme.txt"), "ignore me\n").unwrap();

        let mgr = ConfigFileManager::new(&dir);
        let configs = mgr.list_configs().unwrap();
        assert_eq!(configs.len(), 2);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_and_read_config() {
        let dir = std::env::temp_dir().join("mihomo_tui_test_rw");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let mgr = ConfigFileManager::new(&dir);
        mgr.write_config("test.yaml", "port: 7890\n").unwrap();
        let content = mgr.read_config("test.yaml").unwrap();
        assert_eq!(content, "port: 7890\n");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn validate_config_valid() {
        let dir = std::env::temp_dir().join("mihomo_tui_test_validate");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        fs::write(
            dir.join("good.yaml"),
            r#"mixed-port: 7890
proxies:
  - name: test
    type: ss
    server: 1.2.3.4
    port: 443
proxy-groups:
  - name: default
    type: select
rules:
  - MATCH,DIRECT
"#,
        )
        .unwrap();

        let mgr = ConfigFileManager::new(&dir);
        let result = mgr.validate_config("good.yaml").unwrap();
        assert!(result.valid);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn create_default_config() {
        let dir = std::env::temp_dir().join("mihomo_tui_test_default");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let mgr = ConfigFileManager::new(&dir);
        let path = mgr.create_default_config("new-config.yaml").unwrap();
        assert!(path.exists());

        let content = mgr.read_config("new-config.yaml").unwrap();
        assert!(content.contains("mixed-port: 7890"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn reject_path_traversal() {
        let dir = std::env::temp_dir().join("mihomo_tui_test_traversal");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let mgr = ConfigFileManager::new(&dir);
        assert!(mgr.resolve_path_for_write("../../../etc/passwd").is_err());
    }
}
