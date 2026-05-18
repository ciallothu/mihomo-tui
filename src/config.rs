//! Application configuration management.
//!
//! [`AppConfig`] is assembled from (in order of precedence):
//! 1. CLI arguments
//! 2. A TOML/YAML config file at `$XDG_CONFIG_HOME/mihomo-tui/config.yaml`
//! 3. Hard-coded defaults
//!
//! The struct is cheaply cloneable and passed around by `Arc<AppConfig>` at
//! runtime.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::cli::CliArgs;
use crate::error::{AppError, Result};

// ── Config struct ───────────────────────────────────────────────────────────

/// Runtime configuration for the TUI application.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    // ── API ─────────────────────────────────────────────────────────────────
    /// Scheme (`http` or `https`).
    pub api_scheme: String,
    /// Host:port of the mihomo external controller.
    pub api_addr: String,
    /// Base URL derived from scheme + api_addr.
    #[serde(skip)]
    pub api_base_url: String,
    /// Bearer token (the `secret` field from mihomo config).
    pub api_secret: String,

    // ── Paths ───────────────────────────────────────────────────────────────
    /// mihomo working / config directory.
    pub mihomo_config_dir: PathBuf,
    /// Directory where mihomo-tui stores its own state.
    pub app_config_dir: PathBuf,

    // ── UI ──────────────────────────────────────────────────────────────────
    /// UI tick interval in milliseconds.
    pub tick_rate_ms: u64,
    /// Whether to use a light colour theme.
    pub light_theme: bool,

    // ── Logging ─────────────────────────────────────────────────────────────
    /// Log level filter.
    pub log_level: String,
    /// Optional file path to write logs to.
    pub log_file: Option<PathBuf>,
}

// ── Defaults ────────────────────────────────────────────────────────────────

impl Default for AppConfig {
    fn default() -> Self {
        let api_scheme = "http".to_owned();
        let api_addr = "127.0.0.1:9090".to_owned();
        let api_base_url = format!("{api_scheme}://{api_addr}");

        let app_config_dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("mihomo-tui");

        let mihomo_config_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".config")
            .join("mihomo");

        Self {
            api_scheme,
            api_addr,
            api_base_url,
            api_secret: String::new(),

            mihomo_config_dir,
            app_config_dir,

            tick_rate_ms: 100,
            light_theme: false,

            log_level: "warn".to_owned(),
            log_file: None,
        }
    }
}

// ── Construction ────────────────────────────────────────────────────────────

impl AppConfig {
    /// Build the final config.
    ///
    /// 1. Load defaults.
    /// 2. Overlay values from the on-disk config file (if present).
    /// 3. Overlay CLI arguments (highest precedence).
    pub fn from_cli(cli: &CliArgs) -> Result<Self> {
        let mut cfg = Self::default();

        // Try to load on-disk config.
        let cfg_path = cfg.app_config_dir.join("config.yaml");
        if cfg_path.exists() {
            Self::load_from_file(&mut cfg, &cfg_path)?;
        }

        // Apply CLI overrides.
        cfg.api_addr = cli.api_addr.clone();
        cfg.api_scheme = if cli.use_https {
            "https".to_string()
        } else {
            "http".to_string()
        };
        cfg.api_base_url = format!("{}://{}", cfg.api_scheme, cfg.api_addr);
        cfg.api_secret = cli.secret.clone();
        cfg.light_theme = cli.light_theme;
        cfg.tick_rate_ms = cli.tick_rate;
        cfg.log_level = cli.log_level.clone();
        cfg.log_file = cli.log_file.as_ref().map(PathBuf::from);

        if let Some(ref dir) = cli.config_dir {
            cfg.mihomo_config_dir = PathBuf::from(dir);
        }

        Ok(cfg)
    }

    // ── File I/O ────────────────────────────────────────────────────────────

    /// Load values from a YAML config file on top of the current config.
    fn load_from_file(cfg: &mut Self, path: &Path) -> Result<()> {
        let raw = std::fs::read_to_string(path)
            .map_err(|e| AppError::Config(format!("cannot read {}: {e}", path.display())))?;
        let file_cfg: serde_yaml::Value = serde_yaml::from_str(&raw)
            .map_err(|e| AppError::Config(format!("parse error in {}: {e}", path.display())))?;

        if let Some(v) = file_cfg.get("api_addr").and_then(|v| v.as_str()) {
            cfg.api_addr = v.to_owned();
        }
        if let Some(v) = file_cfg.get("api_secret").and_then(|v| v.as_str()) {
            cfg.api_secret = v.to_owned();
        }
        if let Some(v) = file_cfg.get("api_scheme").and_then(|v| v.as_str()) {
            cfg.api_scheme = v.to_owned();
        }
        if let Some(v) = file_cfg.get("tick_rate_ms").and_then(|v| v.as_u64()) {
            cfg.tick_rate_ms = v;
        }
        if let Some(v) = file_cfg.get("light_theme").and_then(|v| v.as_bool()) {
            cfg.light_theme = v;
        }
        if let Some(v) = file_cfg.get("log_level").and_then(|v| v.as_str()) {
            cfg.log_level = v.to_owned();
        }
        if let Some(v) = file_cfg.get("mihomo_config_dir").and_then(|v| v.as_str()) {
            cfg.mihomo_config_dir = PathBuf::from(v);
        }
        if let Some(v) = file_cfg.get("log_file").and_then(|v| v.as_str()) {
            cfg.log_file = Some(PathBuf::from(v));
        }

        // Recompute derived field.
        cfg.api_base_url = format!("{}://{}", cfg.api_scheme, cfg.api_addr);
        Ok(())
    }

    /// Persist current config to the default path.
    pub fn save(&self) -> Result<()> {
        std::fs::create_dir_all(&self.app_config_dir)?;
        let path = self.app_config_dir.join("config.yaml");
        let yaml = serde_yaml::to_string(self)
            .map_err(|e| AppError::Config(format!("serialize error: {e}")))?;
        std::fs::write(&path, yaml)
            .map_err(|e| AppError::Config(format!("write error {}: {e}", path.display())))?;
        Ok(())
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::CliArgs;
    use clap::Parser;

    #[test]
    fn from_cli_defaults() {
        let cli = CliArgs::try_parse_from(["mihomo-tui"]).unwrap();
        let cfg = AppConfig::from_cli(&cli).unwrap();
        assert_eq!(cfg.api_addr, "127.0.0.1:9090");
        assert_eq!(cfg.api_base_url, "http://127.0.0.1:9090");
        assert_eq!(cfg.api_scheme, "http");
        assert_eq!(cfg.api_secret, "");
    }

    #[test]
    fn from_cli_overrides() {
        let cli = CliArgs::try_parse_from([
            "mihomo-tui",
            "--api-addr",
            "192.168.1.1:9090",
            "--secret",
            "mypassword",
            "--use-https",
            "--light-theme",
            "--tick-rate",
            "200",
        ])
        .unwrap();
        let cfg = AppConfig::from_cli(&cli).unwrap();
        assert_eq!(cfg.api_addr, "192.168.1.1:9090");
        assert_eq!(cfg.api_base_url, "https://192.168.1.1:9090");
        assert_eq!(cfg.api_scheme, "https");
        assert_eq!(cfg.api_secret, "mypassword");
        assert!(cfg.light_theme);
        assert_eq!(cfg.tick_rate_ms, 200);
    }
}
