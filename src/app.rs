//! Application state machine and event handling.
//!
//! [`App`] holds all mutable state for the TUI: current tab, selection indices,
//! data buffers, search state, etc. It exposes methods to react to keyboard
//! events and async data updates from the mihomo API.

use std::collections::HashMap;

use crate::api::client::MihomoClient;
use crate::api::types::*;
use crate::config::AppConfig;
use crate::core::kernel::{GithubRelease, KernelManager};
use anyhow::bail;

// ═══════════════════════════════════════════════════════════════════════════
// AppMode – tab identifiers
// ═══════════════════════════════════════════════════════════════════════════

/// Which panel/tab is currently active.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AppMode {
    #[default]
    Dashboard,
    Proxies,
    Connections,
    Logs,
    Rules,
    Config,
    Providers,
    Kernel,
}

impl AppMode {
    /// Convert to an index for the tab bar.
    pub fn as_index(self) -> usize {
        match self {
            Self::Dashboard => 0,
            Self::Proxies => 1,
            Self::Connections => 2,
            Self::Logs => 3,
            Self::Rules => 4,
            Self::Config => 5,
            Self::Providers => 6,
            Self::Kernel => 7,
        }
    }

    /// Convert from an index back to AppMode.
    pub fn from_index(i: usize) -> Self {
        match i {
            0 => Self::Dashboard,
            1 => Self::Proxies,
            2 => Self::Connections,
            3 => Self::Logs,
            4 => Self::Rules,
            5 => Self::Config,
            6 => Self::Providers,
            7 => Self::Kernel,
            _ => Self::Dashboard,
        }
    }

    /// Cycle to the next tab.
    pub fn next(self) -> Self {
        Self::from_index((self.as_index() + 1) % 8)
    }

    /// Cycle to the previous tab.
    pub fn prev(self) -> Self {
        Self::from_index((self.as_index() + 7) % 8)
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Internal panel states
// ═══════════════════════════════════════════════════════════════════════════

/// Proxy panel state.
#[derive(Debug, Default)]
pub struct ProxyState {
    /// Ordered list of group names.
    pub groups: Vec<String>,
    /// Map from group name to its proxy list.
    pub group_members: HashMap<String, Vec<String>>,
    /// Map from group name to its "now" selection.
    pub group_now: HashMap<String, String>,
    /// Map from node name to last known delay (ms). 0 = untested.
    pub delays: HashMap<String, u64>,
    /// Currently selected group index.
    pub selected_group: usize,
    /// Currently selected node index within the active group.
    pub selected_node: usize,
}

/// Connection panel state.
#[derive(Debug, Default)]
pub struct ConnectionState {
    /// Active connections.
    pub connections: Vec<ConnectionItem>,
    /// Total download bytes.
    pub download_total: u64,
    /// Total upload bytes.
    pub upload_total: u64,
    /// Selected row.
    pub selected: usize,
    /// Sort column index.
    pub sort_col: usize,
    /// Sort descending.
    pub sort_desc: bool,
}

/// Log panel state.
#[derive(Debug)]
pub struct LogState {
    /// Ring buffer of log entries.
    pub buffer: Vec<LogEntry>,
    /// Maximum buffer size.
    pub capacity: usize,
    /// Current log level filter.
    pub level_filter: String,
    /// Auto-scroll to bottom.
    pub auto_scroll: bool,
    /// Vertical scroll offset.
    pub scroll_offset: u16,
}

impl Default for LogState {
    fn default() -> Self {
        Self {
            buffer: Vec::with_capacity(2000),
            capacity: 2000,
            level_filter: "info".to_owned(),
            auto_scroll: true,
            scroll_offset: 0,
        }
    }
}

/// Rules panel state.
#[derive(Debug, Default)]
pub struct RulesState {
    /// Fetched rules.
    pub rules: Vec<RuleItem>,
    /// Selected row.
    pub selected: usize,
    /// Scroll offset.
    pub scroll_offset: u16,
}

/// Config panel state.
#[derive(Debug, Default)]
pub struct ConfigState {
    /// Current config from API.
    pub config: Option<ConfigResponse>,
}

/// Provider panel state.
#[derive(Debug, Default)]
pub struct ProviderState {
    /// Proxy providers.
    pub proxy_providers: HashMap<String, ProxyProviderItem>,
    /// Rule providers.
    pub rule_providers: HashMap<String, RuleProviderItem>,
    /// Selected item index.
    pub selected: usize,
    /// Active tab: 0 = proxy providers, 1 = rule providers.
    pub active_tab: usize,
}

/// Kernel panel state.
#[derive(Debug, Default)]
pub struct KernelState {
    /// Current version string.
    pub current_version: String,
    /// Available versions from GitHub.
    pub available_versions: Vec<String>,
    /// Locally installed versions.
    pub installed_versions: Vec<String>,
    /// Full release data for download.
    pub releases: Vec<GithubRelease>,
    /// Selected index.
    pub selected: usize,
    /// Download progress (0..100).
    pub download_progress: u8,
    /// Whether we are currently downloading.
    pub downloading: bool,
}

// ═══════════════════════════════════════════════════════════════════════════
// Search / filter state
// ═══════════════════════════════════════════════════════════════════════════

/// Search bar state (shared across panels).
#[derive(Debug, Default)]
pub struct SearchState {
    /// Whether the search bar is active (receiving input).
    pub active: bool,
    /// Current query string.
    pub query: String,
}

// ═══════════════════════════════════════════════════════════════════════════
// App – the main state container
// ═══════════════════════════════════════════════════════════════════════════

/// Top-level application state.
pub struct App {
    // ── Configuration & API ──────────────────────────────────────────────
    pub config: AppConfig,
    pub client: MihomoClient,
    pub kernel_manager: KernelManager,

    // ── Core UI state ────────────────────────────────────────────────────
    pub mode: AppMode,
    pub running: bool,
    pub show_help: bool,
    /// Last known terminal width (updated each render frame).
    pub terminal_width: u16,

    // ── Traffic history ──────────────────────────────────────────────────
    /// Upload speed history for sparkline (samples).
    pub traffic_up_history: Vec<u64>,
    /// Download speed history for sparkline (samples).
    pub traffic_down_history: Vec<u64>,
    /// Maximum number of traffic history samples.
    pub traffic_history_capacity: usize,

    // ── Memory ───────────────────────────────────────────────────────────
    pub memory_inuse: u64,

    // ── Version ──────────────────────────────────────────────────────────
    pub version: String,

    // ── Panel states ─────────────────────────────────────────────────────
    pub proxy: ProxyState,
    pub connections: ConnectionState,
    pub logs: LogState,
    pub rules: RulesState,
    pub cfg: ConfigState,
    pub providers: ProviderState,
    pub kernel: KernelState,

    // ── Search ───────────────────────────────────────────────────────────
    pub search: SearchState,

    // ── Status message ───────────────────────────────────────────────────
    pub status_message: String,

    /// Set when initial connection to mihomo API fails.
    pub connection_error: Option<String>,
}

impl App {
    /// Create a new `App` from configuration.
    pub fn new(config: AppConfig) -> Self {
        let base_url = config.api_base_url.clone();
        let secret = config.api_secret.clone();
        let client = MihomoClient::new(&base_url, &secret);

        let data_dir = dirs::data_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
        let kernel_manager = KernelManager::new(&data_dir);

        Self {
            config,
            client,
            kernel_manager,
            mode: AppMode::default(),
            running: true,
            show_help: false,
            terminal_width: 80,

            traffic_up_history: Vec::with_capacity(120),
            traffic_down_history: Vec::with_capacity(120),
            traffic_history_capacity: 120,

            memory_inuse: 0,
            version: String::new(),

            proxy: ProxyState::default(),
            connections: ConnectionState::default(),
            logs: LogState::default(),
            rules: RulesState::default(),
            cfg: ConfigState::default(),
            providers: ProviderState::default(),
            kernel: KernelState::default(),

            search: SearchState::default(),
            status_message: String::new(),
            connection_error: None,
        }
    }

    // ═════════════════════════════════════════════════════════════════════
    // Quit
    // ═════════════════════════════════════════════════════════════════════

    /// Signal the application to exit.
    pub fn quit(&mut self) {
        self.running = false;
    }

    // ═════════════════════════════════════════════════════════════════════
    // Tab switching
    // ═════════════════════════════════════════════════════════════════════

    /// Switch to the next tab.
    pub fn next_tab(&mut self) {
        self.mode = self.mode.next();
    }

    /// Switch to the previous tab.
    pub fn prev_tab(&mut self) {
        self.mode = self.mode.prev();
    }

    /// Switch to a specific tab by number (1-8).
    pub fn switch_tab(&mut self, n: usize) {
        if n > 0 && n <= 8 {
            self.mode = AppMode::from_index(n - 1);
        }
    }

    // ═════════════════════════════════════════════════════════════════════
    // Traffic data
    // ═════════════════════════════════════════════════════════════════════

    /// Push a traffic sample into the history buffers.
    pub fn push_traffic(&mut self, up: u64, down: u64) {
        self.traffic_up_history.push(up);
        self.traffic_down_history.push(down);
        if self.traffic_up_history.len() > self.traffic_history_capacity {
            self.traffic_up_history.remove(0);
        }
        if self.traffic_down_history.len() > self.traffic_history_capacity {
            self.traffic_down_history.remove(0);
        }
    }

    // ═════════════════════════════════════════════════════════════════════
    // Log data
    // ═════════════════════════════════════════════════════════════════════

    /// Push a log entry into the ring buffer.
    pub fn push_log(&mut self, entry: LogEntry) {
        if self.logs.buffer.len() >= self.logs.capacity {
            self.logs.buffer.remove(0);
        }
        self.logs.buffer.push(entry);
    }

    // ═════════════════════════════════════════════════════════════════════
    // Search
    // ═════════════════════════════════════════════════════════════════════

    /// Activate the search bar.
    pub fn start_search(&mut self) {
        self.search.active = true;
        self.search.query.clear();
    }

    /// Deactivate the search bar.
    pub fn end_search(&mut self) {
        self.search.active = false;
    }

    /// Append a character to the search query.
    pub fn search_char(&mut self, c: char) {
        self.search.query.push(c);
    }

    /// Remove the last character from the search query.
    pub fn search_backspace(&mut self) {
        self.search.query.pop();
    }

    // ═════════════════════════════════════════════════════════════════════
    // Data refresh helpers (called from async tasks)
    // ═════════════════════════════════════════════════════════════════════

    /// Refresh proxy data from the API.
    pub async fn refresh_proxies(&mut self) -> anyhow::Result<()> {
        let resp = self.client.get_proxies().await?;
        let mut groups = Vec::new();
        let mut group_members = HashMap::new();
        let mut group_now = HashMap::new();
        let mut delays = HashMap::new();

        let mut sortable: Vec<_> = resp.proxies.iter().collect();
        sortable.sort_by_key(|(name, _)| name.as_str());

        for (name, item) in &sortable {
            let name: String = (**name).clone();
            if let Some(all) = &item.all {
                groups.push(name.clone());
                group_members.insert(name.clone(), all.clone());
                if let Some(now) = &item.now {
                    group_now.insert(name.clone(), now.clone());
                }
            }
            if let Some(last) = item.history.last() {
                delays.insert(name, last.delay);
            }
        }

        self.proxy.groups = groups;
        self.proxy.group_members = group_members;
        self.proxy.group_now = group_now;
        self.proxy.delays = delays;

        if !self.proxy.groups.is_empty() {
            self.proxy.selected_group = self.proxy.selected_group.min(self.proxy.groups.len() - 1);
            self.clamp_node_selection();
        }

        Ok(())
    }

    /// Clamp the selected node index within the active group.
    fn clamp_node_selection(&mut self) {
        let group_name = match self.proxy.groups.get(self.proxy.selected_group) {
            Some(g) => g,
            None => return,
        };
        let count = self
            .proxy
            .group_members
            .get(group_name)
            .map(|v| v.len())
            .unwrap_or(0);
        if count > 0 {
            self.proxy.selected_node = self.proxy.selected_node.min(count - 1);
        }
    }

    /// Refresh connections data.
    pub async fn refresh_connections(&mut self) -> anyhow::Result<()> {
        let resp = self.client.get_connections().await?;
        self.connections.download_total = resp.download_total;
        self.connections.upload_total = resp.upload_total;
        self.connections.connections = resp.connections.unwrap_or_default();

        if !self.connections.connections.is_empty() {
            self.connections.selected = self
                .connections
                .selected
                .min(self.connections.connections.len() - 1);
        }
        Ok(())
    }

    /// Refresh rules data.
    pub async fn refresh_rules(&mut self) -> anyhow::Result<()> {
        let resp = self.client.get_rules().await?;
        self.rules.rules = resp.rules;
        if !self.rules.rules.is_empty() {
            self.rules.selected = self.rules.selected.min(self.rules.rules.len() - 1);
        }
        Ok(())
    }

    /// Refresh config data.
    pub async fn refresh_config(&mut self) -> anyhow::Result<()> {
        let resp = self.client.get_configs().await?;
        self.cfg.config = Some(resp);
        Ok(())
    }

    /// Refresh provider data.
    pub async fn refresh_providers(&mut self) -> anyhow::Result<()> {
        let proxy_resp = self.client.get_proxy_providers().await?;
        let rule_resp = self.client.get_rule_providers().await?;
        self.providers.proxy_providers = proxy_resp.providers;
        self.providers.rule_providers = rule_resp.providers;
        Ok(())
    }

    /// Refresh version info.
    pub async fn refresh_version(&mut self) -> anyhow::Result<()> {
        let resp = self.client.get_version().await?;
        self.version = resp.version;
        Ok(())
    }

    /// Fetch available versions from GitHub and list installed versions.
    pub async fn refresh_kernel(&mut self) -> anyhow::Result<()> {
        // Fetch remote releases
        let releases = self.kernel_manager.list_remote_versions().await?;
        self.kernel.available_versions = releases.iter().map(|r| r.tag_name.clone()).collect();
        self.kernel.releases = releases;
        // Fetch installed versions
        self.kernel.installed_versions = self
            .kernel_manager
            .list_installed_versions()
            .unwrap_or_default();
        // Set current active version
        self.kernel.current_version = self.kernel_manager.get_active_version().unwrap_or_default();
        Ok(())
    }

    /// Download the selected kernel version from GitHub.
    pub async fn download_selected_kernel(&mut self) -> anyhow::Result<()> {
        let idx = self.kernel.selected;
        let releases = &self.kernel.releases;
        if idx >= releases.len() {
            bail!("no version selected");
        }
        self.kernel.downloading = true;
        let result = self.kernel_manager.download_version(&releases[idx]).await;
        self.kernel.downloading = false;
        if let Err(e) = &result {
            self.status_message = format!("Download failed: {e}");
        } else {
            // Refresh installed list
            self.kernel.installed_versions = self
                .kernel_manager
                .list_installed_versions()
                .unwrap_or_default();
            let tag = &releases[idx].tag_name;
            self.status_message = format!("Downloaded {tag}");
        }
        result?;
        Ok(())
    }

    /// Switch to the selected kernel version.
    pub async fn switch_selected_kernel(&mut self) -> anyhow::Result<()> {
        let idx = self.kernel.selected;
        let versions = &self.kernel.available_versions;
        if idx >= versions.len() {
            bail!("no version selected");
        }
        let version = &versions[idx];
        // Check if installed
        if !self.kernel.installed_versions.contains(version) {
            self.status_message =
                format!("{version} not installed locally, press 'd' to download first");
            return Ok(());
        }
        self.kernel_manager.set_active_version(version)?;
        self.kernel.current_version = version.clone();
        self.status_message = format!("Switched to {version}");
        Ok(())
    }

    // ═════════════════════════════════════════════════════════════════════
    // Actions (triggered by keyboard)
    // ═════════════════════════════════════════════════════════════════════

    /// Get the current node name in the active proxy group.
    fn current_node_name(&self) -> Option<String> {
        let group_name = self.proxy.groups.get(self.proxy.selected_group)?;
        let members = self.proxy.group_members.get(group_name)?;
        members.get(self.proxy.selected_node).cloned()
    }

    /// Select current node in the active proxy group.
    pub async fn select_proxy_node(&mut self) -> anyhow::Result<()> {
        if let Some(group_name) = self.proxy.groups.get(self.proxy.selected_group).cloned()
            && let Some(node_name) = self.current_node_name()
        {
            self.client.switch_proxy(&group_name, &node_name).await?;
            self.proxy.group_now.insert(group_name, node_name);
            self.status_message = "Switched proxy".to_owned();
        }
        Ok(())
    }

    /// Test latency for the current node.
    pub async fn test_current_latency(&mut self) -> anyhow::Result<()> {
        if let Some(node_name) = self.current_node_name() {
            match self
                .client
                .get_proxy_delay(&node_name, "https://www.gstatic.com/generate_204", 5000)
                .await
            {
                Ok(resp) => {
                    self.proxy.delays.insert(node_name.clone(), resp.delay);
                    self.status_message = format!("{}: {}ms", node_name, resp.delay);
                }
                Err(e) => {
                    self.proxy.delays.insert(node_name.clone(), 0);
                    self.status_message = format!("{}: timeout/error ({})", node_name, e);
                }
            }
        }
        Ok(())
    }

    /// Test latency for all nodes in the current group.
    pub async fn test_all_latency(&mut self) -> anyhow::Result<()> {
        let group_name = match self.proxy.groups.get(self.proxy.selected_group).cloned() {
            Some(g) => g,
            None => return Ok(()),
        };
        let members = match self.proxy.group_members.get(&group_name).cloned() {
            Some(m) => m,
            None => return Ok(()),
        };

        self.status_message = format!("Testing {} nodes…", members.len());

        for node_name in &members {
            match self
                .client
                .get_proxy_delay(node_name, "https://www.gstatic.com/generate_204", 5000)
                .await
            {
                Ok(resp) => {
                    self.proxy.delays.insert(node_name.clone(), resp.delay);
                }
                Err(_) => {
                    self.proxy.delays.insert(node_name.clone(), 0);
                }
            }
        }

        self.status_message = format!("{} nodes tested", members.len());
        Ok(())
    }

    /// Close the selected connection.
    pub async fn close_selected_connection(&mut self) -> anyhow::Result<()> {
        if let Some(conn) = self
            .connections
            .connections
            .get(self.connections.selected)
            .cloned()
        {
            self.client.close_connection(&conn.id).await?;
            self.connections
                .connections
                .remove(self.connections.selected);
            if !self.connections.connections.is_empty() {
                self.connections.selected = self
                    .connections
                    .selected
                    .min(self.connections.connections.len() - 1);
            }
            self.status_message = "Connection closed".to_owned();
        }
        Ok(())
    }

    /// Close all connections.
    pub async fn close_all_connections(&mut self) -> anyhow::Result<()> {
        self.client.close_all_connections().await?;
        self.connections.connections.clear();
        self.connections.selected = 0;
        self.status_message = "All connections closed".to_owned();
        Ok(())
    }

    /// Cycle the clash mode (Rule → Global → Direct).
    pub async fn cycle_mode(&mut self) -> anyhow::Result<()> {
        let new_mode = match self.cfg.config.as_ref().map(|c| &c.mode) {
            Some(ClashMode::Rule) => ClashMode::Global,
            Some(ClashMode::Global) => ClashMode::Direct,
            _ => ClashMode::Rule,
        };
        let patch = PatchConfigRequest {
            mode: Some(new_mode.clone()),
            ..Default::default()
        };
        self.client.patch_configs(&patch).await?;
        if let Some(ref mut cfg) = self.cfg.config {
            cfg.mode = new_mode;
        }
        self.status_message = format!("Mode: {:?}", self.cfg.config.as_ref().unwrap().mode);
        Ok(())
    }

    /// Reload config from disk.
    pub async fn reload_config(&mut self) -> anyhow::Result<()> {
        let config_path = self
            .config
            .mihomo_config_dir
            .join("config.yaml")
            .to_string_lossy()
            .to_string();
        self.client.reload_configs(&config_path).await?;
        self.status_message = "Config reloaded".to_owned();
        self.refresh_config().await?;
        Ok(())
    }

    /// Update the selected provider.
    pub async fn update_selected_provider(&mut self) -> anyhow::Result<()> {
        if self.providers.active_tab == 0 {
            let names: Vec<_> = self.providers.proxy_providers.keys().cloned().collect();
            if let Some(name) = names.get(self.providers.selected) {
                self.client.update_proxy_provider(name).await?;
                self.status_message = format!("Provider '{}' updated", name);
            }
        } else {
            let names: Vec<_> = self.providers.rule_providers.keys().cloned().collect();
            if let Some(name) = names.get(self.providers.selected) {
                self.client.update_rule_provider(name).await?;
                self.status_message = format!("Rule provider '{}' updated", name);
            }
        }
        Ok(())
    }

    /// Update all providers.
    pub async fn update_all_providers(&mut self) -> anyhow::Result<()> {
        self.status_message = "Updating all providers…".to_owned();
        for name in self.providers.proxy_providers.keys() {
            let _ = self.client.update_proxy_provider(name).await;
        }
        for name in self.providers.rule_providers.keys() {
            let _ = self.client.update_rule_provider(name).await;
        }
        self.status_message = "All providers updated".to_owned();
        Ok(())
    }

    // ═════════════════════════════════════════════════════════════════════
    // Navigation helpers
    // ═════════════════════════════════════════════════════════════════════

    /// Move selection down in the current panel.
    pub fn move_down(&mut self) {
        match self.mode {
            AppMode::Proxies => {
                let count = self.current_group_node_count();
                if count > 0 {
                    self.proxy.selected_node = (self.proxy.selected_node + 1).min(count - 1);
                }
            }
            AppMode::Connections if !self.connections.connections.is_empty() => {
                self.connections.selected =
                    (self.connections.selected + 1).min(self.connections.connections.len() - 1);
            }
            AppMode::Rules if !self.rules.rules.is_empty() => {
                self.rules.selected = (self.rules.selected + 1).min(self.rules.rules.len() - 1);
            }
            AppMode::Providers => {
                let count = self.provider_count();
                if count > 0 {
                    self.providers.selected = (self.providers.selected + 1).min(count - 1);
                }
            }
            AppMode::Kernel if !self.kernel.available_versions.is_empty() => {
                self.kernel.selected =
                    (self.kernel.selected + 1).min(self.kernel.available_versions.len() - 1);
            }
            _ => {}
        }
    }

    /// Move selection up in the current panel.
    pub fn move_up(&mut self) {
        match self.mode {
            AppMode::Proxies => {
                self.proxy.selected_node = self.proxy.selected_node.saturating_sub(1);
            }
            AppMode::Connections => {
                self.connections.selected = self.connections.selected.saturating_sub(1);
            }
            AppMode::Rules => {
                self.rules.selected = self.rules.selected.saturating_sub(1);
            }
            AppMode::Providers => {
                self.providers.selected = self.providers.selected.saturating_sub(1);
            }
            AppMode::Kernel => {
                self.kernel.selected = self.kernel.selected.saturating_sub(1);
            }
            _ => {}
        }
    }

    /// Move to the next proxy group (left panel).
    pub fn next_proxy_group(&mut self) {
        if !self.proxy.groups.is_empty() {
            self.proxy.selected_group =
                (self.proxy.selected_group + 1).min(self.proxy.groups.len() - 1);
            self.proxy.selected_node = 0;
        }
    }

    /// Move to the previous proxy group (left panel).
    pub fn prev_proxy_group(&mut self) {
        self.proxy.selected_group = self.proxy.selected_group.saturating_sub(1);
        self.proxy.selected_node = 0;
    }

    /// Number of nodes in the currently selected proxy group.
    fn current_group_node_count(&self) -> usize {
        let group_name = match self.proxy.groups.get(self.proxy.selected_group) {
            Some(g) => g,
            None => return 0,
        };
        self.proxy
            .group_members
            .get(group_name)
            .map(|v| v.len())
            .unwrap_or(0)
    }

    /// Number of items in the active provider tab.
    fn provider_count(&self) -> usize {
        if self.providers.active_tab == 0 {
            self.providers.proxy_providers.len()
        } else {
            self.providers.rule_providers.len()
        }
    }

    /// Toggle the provider tab.
    pub fn toggle_provider_tab(&mut self) {
        self.providers.active_tab = if self.providers.active_tab == 0 { 1 } else { 0 };
        self.providers.selected = 0;
    }

    // ═════════════════════════════════════════════════════════════════════
    // Refresh the current panel's data
    // ═════════════════════════════════════════════════════════════════════

    /// Trigger a data refresh for the current panel.
    pub async fn refresh_current_panel(&mut self) -> anyhow::Result<()> {
        match self.mode {
            AppMode::Dashboard | AppMode::Proxies => {
                let _ = self.refresh_proxies().await;
                let _ = self.refresh_version().await;
            }
            AppMode::Connections => {
                let _ = self.refresh_connections().await;
            }
            AppMode::Rules => {
                let _ = self.refresh_rules().await;
            }
            AppMode::Config => {
                let _ = self.refresh_config().await;
            }
            AppMode::Providers => {
                let _ = self.refresh_providers().await;
            }
            AppMode::Kernel => {
                let _ = self.refresh_kernel().await;
            }
            AppMode::Logs => {}
        }
        Ok(())
    }
}
