mod api;
mod app;
mod config;
mod core;
mod panel;
mod tui;

use std::{path::PathBuf, process::Child};

use anyhow::Result;
use clap::Parser;

use crate::{
    api::MihomoClient,
    app::App,
    config::ConfigManager,
    core::{CoreStatus, MihomoCore},
};

#[derive(Debug, Parser)]
#[command(
    name = "mihomo-tui",
    version,
    about = "A cross-platform TUI workspace for mihomo"
)]
struct Cli {
    /// Import a local mihomo YAML config before opening the TUI.
    #[arg(short, long)]
    config: Option<PathBuf>,

    /// Pull a remote mihomo subscription URL before opening the TUI.
    #[arg(short, long)]
    subscribe: Option<String>,

    /// Use a specific mihomo core binary path.
    #[arg(long)]
    core: Option<PathBuf>,

    /// Install a mihomo core release before opening the TUI. Use "latest" or a tag like v1.19.24.
    #[arg(long)]
    install_core: Option<String>,

    /// Start mihomo with the bundled/default core and an imported config before opening the TUI.
    #[arg(long)]
    start_core: bool,

    /// Mihomo external-controller URL.
    #[arg(long, default_value = "http://127.0.0.1:9090")]
    controller: String,

    /// Mihomo external-controller secret.
    #[arg(long)]
    secret: Option<String>,

    /// Override the app data directory.
    #[arg(long)]
    data_dir: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let config_manager = ConfigManager::new(cli.data_dir)?;
    let mut notices = Vec::new();
    let mut active_config = None;
    let mut core_path = cli.core;

    if let Some(version) = cli.install_core.as_deref() {
        let installed = core::install_core(version, &config_manager.paths().cores).await?;
        notices.push(format!("Installed mihomo core: {}", installed.display()));
        core_path = Some(installed);
    }

    if let Some(path) = cli.config.as_ref() {
        let imported = config_manager.import_file(path)?;
        notices.push(format!("Imported config: {}", imported.display()));
        active_config = Some(imported);
    }

    if let Some(url) = cli.subscribe.as_ref() {
        let pulled = config_manager.pull_subscription(url).await?;
        notices.push(format!("Pulled subscription: {}", pulled.display()));
        active_config = Some(pulled);
    }

    if core_path.is_none() {
        core_path = core::find_default_core(&config_manager.paths().cores);
        if let Some(path) = core_path.as_ref() {
            notices.push(format!(
                "Using bundled/default mihomo core: {}",
                path.display()
            ));
        }
    }

    if cli.start_core && core_path.is_none() {
        let installed = core::install_core("latest", &config_manager.paths().cores).await?;
        notices.push(format!(
            "Installed latest mihomo core for startup: {}",
            installed.display()
        ));
        core_path = Some(installed);
    }

    if cli.start_core && active_config.is_none() {
        let default_config = config_manager.ensure_default_config()?;
        notices.push(format!(
            "Using default config for startup: {}",
            default_config.display()
        ));
        active_config = Some(default_config);
    }

    let mut child = start_core_if_requested(
        cli.start_core,
        core_path.as_ref(),
        active_config.as_ref(),
        &mut notices,
    )?;

    let client = MihomoClient::new(&cli.controller, cli.secret)?;
    let mut core = MihomoCore::new(core_path);
    if child.is_some() {
        core.status = CoreStatus::Running;
    }
    let app = App::new(config_manager.paths().clone(), client, core, notices);
    let result = tui::run(app).await;

    if let Some(child) = child.as_mut() {
        let _ = child.kill();
        let _ = child.wait();
    }

    result
}

fn start_core_if_requested(
    start: bool,
    core_path: Option<&PathBuf>,
    config_path: Option<&PathBuf>,
    notices: &mut Vec<String>,
) -> Result<Option<Child>> {
    if !start {
        return Ok(None);
    }

    let Some(core_path) = core_path else {
        anyhow::bail!("--start-core requires --core or --install-core");
    };
    let Some(config_path) = config_path else {
        anyhow::bail!("--start-core requires --config or --subscribe");
    };
    let child = core::start_core(core_path, config_path)?;
    notices.push(format!(
        "Started mihomo core {} with {}",
        core_path.display(),
        config_path.display()
    ));
    Ok(Some(child))
}
