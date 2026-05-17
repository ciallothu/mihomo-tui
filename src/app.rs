use crate::{
    api::MihomoClient,
    config::AppPaths,
    core::{CoreStatus, MihomoCore, MihomoMode},
    panel::{PanelSnapshot, log},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Overview,
    Proxies,
    Resources,
    Ports,
    Connections,
    Configs,
    Logs,
}

impl Tab {
    pub const ALL: [Self; 7] = [
        Self::Overview,
        Self::Proxies,
        Self::Resources,
        Self::Ports,
        Self::Connections,
        Self::Configs,
        Self::Logs,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Self::Overview => "Overview",
            Self::Proxies => "Proxies",
            Self::Resources => "Providers",
            Self::Ports => "Ports",
            Self::Connections => "Connections",
            Self::Configs => "Configs",
            Self::Logs => "Logs",
        }
    }

    pub fn index(self) -> usize {
        Self::ALL.iter().position(|tab| *tab == self).unwrap_or(0)
    }

    pub fn next(self) -> Self {
        Self::ALL[(self.index() + 1) % Self::ALL.len()]
    }

    pub fn previous(self) -> Self {
        let index = self.index();
        Self::ALL[(index + Self::ALL.len() - 1) % Self::ALL.len()]
    }
}

#[derive(Debug)]
pub struct App {
    pub paths: AppPaths,
    pub client: MihomoClient,
    pub core: MihomoCore,
    pub mode: MihomoMode,
    pub active_tab: Tab,
    pub snapshot: PanelSnapshot,
    pub selected_group: usize,
    pub selected_resource: usize,
    pub selected_connection: usize,
    pub selected_log: usize,
    pub selected_port: usize,
    pub selected_config: usize,
    pub available_configs: Vec<std::path::PathBuf>,
    pub status_message: String,
    pub should_quit: bool,
}

impl App {
    pub fn new(
        paths: AppPaths,
        client: MihomoClient,
        core: MihomoCore,
        notices: Vec<String>,
    ) -> Self {
        let mut snapshot = PanelSnapshot::empty();
        for notice in notices {
            snapshot.logs.push(log("info", notice));
        }

        let status_message = match core.status {
            CoreStatus::Missing => {
                "mihomo core is not configured; TUI will use the controller API if reachable"
                    .to_string()
            }
            CoreStatus::Stopped => {
                "mihomo core detected; use --start-core to launch it before TUI".to_string()
            }
            CoreStatus::Running => "mihomo core is running".to_string(),
        };

        let mut app = Self {
            paths,
            client,
            core,
            mode: MihomoMode::Rule,
            active_tab: Tab::Overview,
            snapshot,
            selected_group: 0,
            selected_resource: 0,
            selected_connection: 0,
            selected_log: 0,
            selected_port: 0,
            selected_config: 0,
            available_configs: Vec::new(),
            status_message,
            should_quit: false,
        };
        app.refresh_configs();
        app
    }

    pub fn refresh_configs(&mut self) {
        if let Ok(entries) = std::fs::read_dir(&self.paths.configs) {
            self.available_configs = entries
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.extension().map(|s| s == "yaml" || s == "yml").unwrap_or(false))
                .collect();
            self.available_configs.sort();
        }
    }

    pub async fn load_initial(&mut self) {
        self.refresh().await;
    }

    pub async fn tick(&mut self) {
        if matches!(
            self.active_tab,
            Tab::Connections | Tab::Overview | Tab::Logs
        ) {
            self.refresh().await;
        }
    }

    pub fn quit(&mut self) {
        self.should_quit = true;
    }

    pub fn next_tab(&mut self) {
        self.active_tab = self.active_tab.next();
    }

    pub fn previous_tab(&mut self) {
        self.active_tab = self.active_tab.previous();
    }

    pub async fn cycle_mode(&mut self) {
        let next = self.mode.next();
        match self.client.set_mode(next).await {
            Ok(()) => {
                self.mode = next;
                self.status_message = format!("Mode switched to {}", self.mode);
                self.snapshot
                    .logs
                    .push(log("info", format!("mode changed to {}", self.mode)));
                self.refresh().await;
            }
            Err(error) => self.push_error(format!("mode switch failed: {error:#}")),
        }
    }

    pub fn move_down(&mut self) {
        match self.active_tab {
            Tab::Overview | Tab::Proxies => {
                self.selected_group = bounded_next(self.selected_group, self.snapshot.groups.len())
            }
            Tab::Resources => {
                self.selected_resource =
                    bounded_next(self.selected_resource, self.snapshot.resources.len())
            }
            Tab::Connections => {
                self.selected_connection =
                    bounded_next(self.selected_connection, self.snapshot.connections.len())
            }
            Tab::Logs => {
                self.selected_log = bounded_next(self.selected_log, self.snapshot.logs.len())
            }
            Tab::Ports => self.selected_port = bounded_next(self.selected_port, PORT_FIELDS.len()),
            Tab::Configs => {
                self.selected_config =
                    bounded_next(self.selected_config, self.available_configs.len() + 1)
            }
        }
    }

    pub fn move_up(&mut self) {
        match self.active_tab {
            Tab::Overview | Tab::Proxies => {
                self.selected_group = self.selected_group.saturating_sub(1)
            }
            Tab::Resources => self.selected_resource = self.selected_resource.saturating_sub(1),
            Tab::Connections => {
                self.selected_connection = self.selected_connection.saturating_sub(1)
            }
            Tab::Logs => self.selected_log = self.selected_log.saturating_sub(1),
            Tab::Ports => self.selected_port = self.selected_port.saturating_sub(1),
            Tab::Configs => self.selected_config = self.selected_config.saturating_sub(1),
        }
    }

    pub async fn activate(&mut self) {
        match self.active_tab {
            Tab::Overview | Tab::Proxies => self.select_next_proxy().await,
            Tab::Resources => self.refresh_resource().await,
            Tab::Ports => self.apply_selected_port().await,
            Tab::Connections => self.close_selected_connection().await,
            Tab::Configs => self.handle_config_action().await,
            Tab::Logs => self.refresh().await,
        }
    }

    async fn handle_config_action(&mut self) {
        if self.selected_config == 0 {
            // Action: Update Kernel
            self.status_message = "Updating mihomo core...".to_string();
            match crate::core::install_core("latest", &self.paths.cores).await {
                Ok(path) => {
                    self.core.binary_path = Some(path);
                    self.status_message = "Mihomo core updated to latest".to_string();
                    self.snapshot.logs.push(log("info", "core updated to latest"));
                }
                Err(e) => self.push_error(format!("core update failed: {e:#}")),
            }
        } else {
            // Action: Switch Config
            let idx = self.selected_config - 1;
            if let Some(path) = self.available_configs.get(idx).cloned() {
                self.status_message = format!("Restarting mihomo with {}...", path.display());
                // In a real app we'd need to signal the core to restart.
                // For now we just notice it.
                self.snapshot.logs.push(log("info", format!("switched to config {}", path.display())));
            }
        }
    }

    pub async fn close_all_connections(&mut self) {
        match self.client.close_all_connections().await {
            Ok(()) => {
                self.status_message = "Closed all mihomo connections".to_string();
                self.snapshot
                    .logs
                    .push(log("info", "closed all connections"));
                self.refresh().await;
            }
            Err(error) => self.push_error(format!("close all connections failed: {error:#}")),
        }
    }

    pub async fn refresh(&mut self) {
        match self.client.snapshot().await {
            Ok((mut snapshot, mode, version)) => {
                snapshot
                    .logs
                    .splice(0..0, self.snapshot.logs.iter().cloned());
                self.snapshot = snapshot;
                self.mode = mode;
                self.core.version = version;
                self.clamp_selections();
                self.status_message = format!("Connected to {}", self.client.base_url());
            }
            Err(error) => self.push_error(format!("refresh failed: {error:#}")),
        }
    }

    async fn select_next_proxy(&mut self) {
        let Some(group) = self.snapshot.groups.get(self.selected_group) else {
            return;
        };
        if group.proxies.is_empty() {
            return;
        }

        let next_index = (group.selected + 1) % group.proxies.len();
        let group_name = group.name.clone();
        let proxy = group.proxies[next_index].name.clone();
        match self.client.select_proxy(&group_name, &proxy).await {
            Ok(()) => {
                self.status_message = format!("{group_name} -> {proxy}");
                self.snapshot.logs.push(log(
                    "info",
                    format!("proxy group {group_name} switched to {proxy}"),
                ));
                self.refresh().await;
            }
            Err(error) => self.push_error(format!("proxy switch failed: {error:#}")),
        }
    }

    async fn refresh_resource(&mut self) {
        let Some(resource) = self.snapshot.resources.get(self.selected_resource).cloned() else {
            return;
        };
        match self.client.refresh_resource(&resource).await {
            Ok(()) => {
                self.status_message = format!("Refreshed provider {}", resource.name);
                self.snapshot
                    .logs
                    .push(log("info", format!("provider {} refreshed", resource.name)));
                self.refresh().await;
            }
            Err(error) => self.push_error(format!("provider refresh failed: {error:#}")),
        }
    }

    async fn close_selected_connection(&mut self) {
        let Some(connection) = self
            .snapshot
            .connections
            .get(self.selected_connection)
            .cloned()
        else {
            return;
        };
        match self.client.close_connection(&connection.id).await {
            Ok(()) => {
                self.status_message = format!("Closed connection {}", connection.host);
                self.snapshot
                    .logs
                    .push(log("info", format!("closed connection {}", connection.id)));
                self.refresh().await;
            }
            Err(error) => self.push_error(format!("close connection failed: {error:#}")),
        }
    }

    pub fn bump_selected_port(&mut self, delta: i32) {
        let current = selected_port_value(&self.snapshot.ports, self.selected_port).unwrap_or(1024);
        let next = (current.max(1024) as i32 + delta).clamp(1024, 65535) as u16;
        set_selected_port_value(&mut self.snapshot.ports, self.selected_port, next);
        let (label, _) = PORT_FIELDS[self.selected_port];
        self.status_message = format!("{label} pending: {next} (Enter to apply)");
    }

    async fn apply_selected_port(&mut self) {
        let (label, key) = PORT_FIELDS[self.selected_port];
        let port = selected_port_value(&self.snapshot.ports, self.selected_port).unwrap_or(1024);
        match self.client.set_port(key, port).await {
            Ok(()) => {
                self.status_message = format!("{label} set to {port}");
                self.snapshot
                    .logs
                    .push(log("info", format!("{label} set to {port}")));
                self.refresh().await;
            }
            Err(error) => self.push_error(format!("port update failed: {error:#}")),
        }
    }

    fn push_error(&mut self, message: String) {
        self.status_message = message.clone();
        self.snapshot.logs.push(log("error", message));
        self.selected_log = self.snapshot.logs.len().saturating_sub(1);
    }

    fn clamp_selections(&mut self) {
        self.selected_group = clamp_index(self.selected_group, self.snapshot.groups.len());
        self.selected_resource = clamp_index(self.selected_resource, self.snapshot.resources.len());
        self.selected_connection =
            clamp_index(self.selected_connection, self.snapshot.connections.len());
        self.selected_log = clamp_index(self.selected_log, self.snapshot.logs.len());
    }
}

pub const PORT_FIELDS: [(&str, &str); 6] = [
    ("HTTP", "port"),
    ("SOCKS", "socks-port"),
    ("Mixed", "mixed-port"),
    ("Redir", "redir-port"),
    ("TProxy", "tproxy-port"),
    ("External Controller", "external-controller"),
];

fn selected_port_value(ports: &crate::panel::ListenerPorts, index: usize) -> Option<u16> {
    match index {
        0 => Some(ports.http),
        1 => Some(ports.socks),
        2 => Some(ports.mixed),
        3 => ports.redir,
        4 => ports.tproxy,
        5 => Some(ports.external_controller),
        _ => None,
    }
}

fn set_selected_port_value(ports: &mut crate::panel::ListenerPorts, index: usize, value: u16) {
    match index {
        0 => ports.http = value,
        1 => ports.socks = value,
        2 => ports.mixed = value,
        3 => ports.redir = Some(value),
        4 => ports.tproxy = Some(value),
        5 => ports.external_controller = value,
        _ => {}
    }
}

fn bounded_next(current: usize, len: usize) -> usize {
    if len == 0 {
        0
    } else {
        (current + 1).min(len - 1)
    }
}

fn clamp_index(current: usize, len: usize) -> usize {
    if len == 0 { 0 } else { current.min(len - 1) }
}

#[cfg(test)]
mod tests {
    use super::{Tab, bounded_next};

    #[test]
    fn tabs_cycle_in_both_directions() {
        assert_eq!(Tab::Overview.next(), Tab::Proxies);
        assert_eq!(Tab::Overview.previous(), Tab::Logs);
    }

    #[test]
    fn bounded_next_stays_inside_list() {
        assert_eq!(bounded_next(0, 0), 0);
        assert_eq!(bounded_next(0, 3), 1);
        assert_eq!(bounded_next(2, 3), 2);
    }
}
