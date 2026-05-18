mod api;
pub mod app;
mod cli;
mod config;
pub mod core;
mod error;
pub mod ui;
pub mod utils;

use std::io;
use std::time::Duration;

use anyhow::Result;
use clap::Parser;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, MouseEventKind};
use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use tokio::sync::mpsc;

use app::{App, AppMode};
use cli::CliArgs;
use config::AppConfig;
use ui::components::{StatusBarData, render_help, render_statusbar, render_tabbar, tab_hit_test};

// ═══════════════════════════════════════════════════════════════════════════
// Async events sent from background tasks to the main loop
// ═══════════════════════════════════════════════════════════════════════════

#[allow(dead_code)]
enum AppEvent {
    /// Terminal keyboard / resize event.
    Key(Event),
    /// Terminal mouse event.
    Mouse(Event),
    /// Periodic tick for UI refresh.
    Tick,
    /// Traffic WebSocket sample received.
    Traffic { up: u64, down: u64 },
    /// Log entry received.
    Log(api::types::LogEntry),
    /// Memory sample received.
    Memory { inuse: u64 },
    /// Background data refresh completed.
    RefreshDone,
    /// An async action completed with a result.
    ActionDone(Result<String>),
}

// ═══════════════════════════════════════════════════════════════════════════
// Entry point
// ═══════════════════════════════════════════════════════════════════════════

fn main() -> anyhow::Result<()> {
    // Parse CLI arguments.
    let cli = CliArgs::parse();

    // Handle subcommands that don't need the TUI.
    match &cli.command {
        Some(cli::CliCommand::Version) => {
            println!("mihomo-tui {}", env!("CARGO_PKG_VERSION"));
            return Ok(());
        }
        Some(cli::CliCommand::DumpConfig) => {
            let cfg = AppConfig::from_cli(&cli)?;
            println!("{}", serde_yaml::to_string(&cfg)?);
            return Ok(());
        }
        Some(cli::CliCommand::Check) => {
            // Synchronous check via blocking runtime.
            let rt = tokio::runtime::Runtime::new()?;
            let cfg = AppConfig::from_cli(&cli)?;
            let client = api::client::MihomoClient::new(&cfg.api_base_url, &cfg.api_secret);
            match rt.block_on(client.check_alive()) {
                Ok(()) => println!("✓ mihomo API is reachable at {}", cfg.api_base_url),
                Err(e) => eprintln!("✗ Cannot reach mihomo API: {e}"),
            }
            return Ok(());
        }
        None => {}
    }

    // Build config and launch the TUI.
    let config = AppConfig::from_cli(&cli)?;

    // Initialise logger.
    if let Some(ref log_file) = config.log_file {
        let file = std::fs::File::create(log_file)?;
        tui_logger::init_logger(tui_logger::LevelFilter::Info).ok();
        // Optionally redirect to file by setting the output — for now just init.
        let _ = file;
    } else {
        tui_logger::init_logger(tui_logger::LevelFilter::Info).ok();
    }

    // Create the tokio runtime.
    let rt = tokio::runtime::Runtime::new()?;

    // Setup terminal.
    enable_raw_mode()?;
    let stdout = io::stdout();
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    crossterm::execute!(
        terminal.backend_mut(),
        EnterAlternateScreen,
        EnableMouseCapture
    )?;

    // Run the app.
    let result = rt.block_on(run_app(&mut terminal, config));

    // Restore terminal.
    disable_raw_mode()?;
    crossterm::execute!(
        terminal.backend_mut(),
        DisableMouseCapture,
        LeaveAlternateScreen
    )?;
    terminal.show_cursor()?;

    result
}

// ═══════════════════════════════════════════════════════════════════════════
// Main async application loop
// ═══════════════════════════════════════════════════════════════════════════

async fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    config: AppConfig,
) -> anyhow::Result<()> {
    let mut app = App::new(config);

    // Channel for sending events from background tasks.
    let (tx, mut rx) = mpsc::channel::<AppEvent>(256);
    let tick_rate = Duration::from_millis(app.config.tick_rate_ms);

    // ── Spawn background event poller ────────────────────────────────────
    let tx_tick = tx.clone();
    tokio::spawn(async move {
        loop {
            if event::poll(tick_rate).unwrap_or(false) {
                if let Ok(ev) = event::read() {
                    let app_ev = match &ev {
                        Event::Mouse(_) => AppEvent::Mouse(ev),
                        _ => AppEvent::Key(ev),
                    };
                    if tx_tick.send(app_ev).await.is_err() {
                        break;
                    }
                }
            } else {
                let _ = tx_tick.send(AppEvent::Tick).await;
            }
        }
    });

    // ── Spawn traffic WebSocket ──────────────────────────────────────────
    let tx_traffic = tx.clone();
    let ws_base = app.client.ws_base_url();
    let ws_secret = app.client.secret().to_owned();
    tokio::spawn(async move {
        loop {
            if let Ok(mut stream) = api::websocket::traffic_stream(&ws_base, &ws_secret).await {
                use futures_util::StreamExt;
                while let Some(result) = stream.next().await {
                    match result {
                        Ok(data) => {
                            if tx_traffic
                                .send(AppEvent::Traffic {
                                    up: data.up,
                                    down: data.down,
                                })
                                .await
                                .is_err()
                            {
                                return;
                            }
                        }
                        Err(_) => break,
                    }
                }
            }
            // Reconnect after 3 seconds on disconnect.
            tokio::time::sleep(Duration::from_secs(3)).await;
        }
    });

    // ── Spawn log WebSocket ──────────────────────────────────────────────
    let tx_log = tx.clone();
    let ws_base_log = app.client.ws_base_url();
    let ws_secret_log = app.client.secret().to_owned();
    tokio::spawn(async move {
        loop {
            if let Ok(mut stream) =
                api::websocket::log_stream(&ws_base_log, &ws_secret_log, Some("info")).await
            {
                use futures_util::StreamExt;
                while let Some(result) = stream.next().await {
                    match result {
                        Ok(entry) => {
                            if tx_log.send(AppEvent::Log(entry)).await.is_err() {
                                return;
                            }
                        }
                        Err(_) => break,
                    }
                }
            }
            tokio::time::sleep(Duration::from_secs(3)).await;
        }
    });

    // ── Spawn memory WebSocket ───────────────────────────────────────────
    let tx_mem = tx.clone();
    let ws_base_mem = app.client.ws_base_url();
    let ws_secret_mem = app.client.secret().to_owned();
    tokio::spawn(async move {
        loop {
            if let Ok(mut stream) =
                api::websocket::memory_stream(&ws_base_mem, &ws_secret_mem).await
            {
                use futures_util::StreamExt;
                while let Some(result) = stream.next().await {
                    match result {
                        Ok(data) => {
                            if tx_mem
                                .send(AppEvent::Memory { inuse: data.inuse })
                                .await
                                .is_err()
                            {
                                return;
                            }
                        }
                        Err(_) => break,
                    }
                }
            }
            tokio::time::sleep(Duration::from_secs(3)).await;
        }
    });

    // ── Initial data fetch ───────────────────────────────────────────────
    app.refresh_proxies().await.ok();
    app.refresh_config().await.ok();
    app.refresh_version().await.ok();
    app.refresh_connections().await.ok();
    app.refresh_rules().await.ok();
    app.refresh_providers().await.ok();

    // ── Periodic refresh interval counter ────────────────────────────────
    let mut tick_counter: u64 = 0;
    let refresh_interval: u64 = 30; // Refresh every 30 ticks (~3s at 100ms tick)

    // ═════════════════════════════════════════════════════════════════════
    // Main loop
    // ═════════════════════════════════════════════════════════════════════
    while app.running {
        // Render the UI.
        terminal.draw(|f| render_ui(f, &mut app))?;

        // Wait for the next event.
        let Some(ev) = rx.recv().await else {
            break;
        };

        match ev {
            AppEvent::Key(ev) => {
                handle_key_event(&mut app, ev, &tx).await;
            }
            AppEvent::Mouse(ev) => {
                handle_mouse_event(&mut app, ev);
            }
            AppEvent::Tick => {
                tick_counter += 1;
                if tick_counter.is_multiple_of(refresh_interval) {
                    // Periodic background refresh.
                    let _ = app.refresh_current_panel().await;
                }
            }
            AppEvent::Traffic { up, down } => {
                app.push_traffic(up, down);
            }
            AppEvent::Log(entry) => {
                app.push_log(entry);
            }
            AppEvent::Memory { inuse } => {
                app.memory_inuse = inuse;
            }
            AppEvent::RefreshDone | AppEvent::ActionDone(_) => {
                // Status already set by the async task.
            }
        }
    }

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════
// Keyboard event handling
// ═══════════════════════════════════════════════════════════════════════════

async fn handle_key_event(app: &mut App, ev: Event, _tx: &mpsc::Sender<AppEvent>) {
    // Only process key press events (ignore release/repeat on some terminals).
    let key = match ev {
        Event::Key(k) if k.kind == KeyEventKind::Press => k,
        _ => return,
    };

    // ── Global keys ──────────────────────────────────────────────────────

    // If search bar is active, capture input there first.
    if app.search.active {
        match key.code {
            KeyCode::Esc => {
                app.end_search();
                return;
            }
            KeyCode::Enter => {
                app.end_search();
                return;
            }
            KeyCode::Backspace => {
                app.search_backspace();
                return;
            }
            KeyCode::Char(c) => {
                app.search_char(c);
                return;
            }
            _ => return,
        }
    }

    // If help overlay is shown, only Esc closes it.
    if app.show_help {
        if matches!(key.code, KeyCode::Esc | KeyCode::Char('?')) {
            app.show_help = false;
        }
        return;
    }

    match key.code {
        // ── Quit ──────────────────────────────────────────────────────
        KeyCode::Char('q') | KeyCode::Char('Q') => {
            app.quit();
        }

        // ── Help ──────────────────────────────────────────────────────
        KeyCode::Char('?') => {
            app.show_help = true;
        }

        // ── Tab navigation (panel-specific first, then general) ───────
        KeyCode::Tab if app.mode == AppMode::Providers => {
            app.toggle_provider_tab();
        }
        KeyCode::Tab => app.next_tab(),
        KeyCode::BackTab => app.prev_tab(),

        // ── Number keys: log level filters (Logs) take priority ───────
        KeyCode::Char('1') if app.mode == AppMode::Logs => {
            app.logs.level_filter = "debug".to_owned();
        }
        KeyCode::Char('2') if app.mode == AppMode::Logs => {
            app.logs.level_filter = "info".to_owned();
        }
        KeyCode::Char('3') if app.mode == AppMode::Logs => {
            app.logs.level_filter = "warning".to_owned();
        }
        KeyCode::Char('4') if app.mode == AppMode::Logs => {
            app.logs.level_filter = "error".to_owned();
        }
        KeyCode::Char('5') if app.mode == AppMode::Logs => {
            app.logs.level_filter = "silent".to_owned();
        }
        // General number-key tab switching.
        KeyCode::Char('1') => app.switch_tab(1),
        KeyCode::Char('2') => app.switch_tab(2),
        KeyCode::Char('3') => app.switch_tab(3),
        KeyCode::Char('4') => app.switch_tab(4),
        KeyCode::Char('5') => app.switch_tab(5),
        KeyCode::Char('6') => app.switch_tab(6),
        KeyCode::Char('7') => app.switch_tab(7),
        KeyCode::Char('8') => app.switch_tab(8),

        // ── Navigation ────────────────────────────────────────────────
        KeyCode::Down | KeyCode::Char('j') => app.move_down(),
        KeyCode::Up | KeyCode::Char('k') => app.move_up(),

        // ── Search ────────────────────────────────────────────────────
        KeyCode::Char('/') => app.start_search(),

        // ── Panel-specific keys ───────────────────────────────────────
        KeyCode::Enter => handle_enter(app).await,
        KeyCode::Char('t') if app.mode == AppMode::Proxies => {
            app.test_current_latency().await.ok();
        }
        KeyCode::Char('T') if app.mode == AppMode::Proxies => {
            app.test_all_latency().await.ok();
        }
        KeyCode::Char('d') if app.mode == AppMode::Connections => {
            app.close_selected_connection().await.ok();
        }
        KeyCode::Char('D') if app.mode == AppMode::Connections => {
            app.close_all_connections().await.ok();
        }
        KeyCode::Char('M') if app.mode == AppMode::Config => {
            app.cycle_mode().await.ok();
        }
        KeyCode::Char('r') if app.mode == AppMode::Config => {
            app.reload_config().await.ok();
        }
        KeyCode::Char('u') if app.mode == AppMode::Providers => {
            app.update_selected_provider().await.ok();
        }
        KeyCode::Char('U') if app.mode == AppMode::Providers => {
            app.update_all_providers().await.ok();
        }
        KeyCode::Char('h') | KeyCode::Left if app.mode == AppMode::Proxies => {
            app.prev_proxy_group();
        }
        KeyCode::Char('l') | KeyCode::Right if app.mode == AppMode::Proxies => {
            app.next_proxy_group();
        }

        // ── Escape ────────────────────────────────────────────────────
        KeyCode::Esc => {}

        _ => {}
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Mouse event handling
// ═══════════════════════════════════════════════════════════════════════════

fn handle_mouse_event(app: &mut App, ev: Event) {
    let mouse = match ev {
        Event::Mouse(m) => m,
        _ => return,
    };

    match mouse.kind {
        MouseEventKind::Down(event::MouseButton::Left) => {
            // Tab bar is always row 0, full terminal width, height 1.
            let tab_bar = Rect::new(0, 0, app.terminal_width, 1);
            if let Some(idx) = tab_hit_test(tab_bar, mouse.column) {
                app.switch_tab(idx + 1);
            }
            // TODO: future — handle clicks in list panels for selection
        }
        MouseEventKind::ScrollUp => {
            app.move_up();
        }
        MouseEventKind::ScrollDown => {
            app.move_down();
        }
        _ => {}
    }
}

/// Handle Enter key based on current mode.
async fn handle_enter(app: &mut App) {
    match app.mode {
        AppMode::Proxies => {
            app.select_proxy_node().await.ok();
        }
        AppMode::Logs => {
            app.logs.auto_scroll = !app.logs.auto_scroll;
        }
        _ => {}
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// UI rendering
// ═══════════════════════════════════════════════════════════════════════════

fn render_ui(f: &mut ratatui::Frame, app: &mut App) {
    let size = f.area();
    app.terminal_width = size.width;

    // Main layout: tab bar (top) + content (middle) + status bar (bottom).
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // tab bar
            Constraint::Min(0),    // content area
            Constraint::Length(1), // status bar
        ])
        .split(size);

    // Render tab bar.
    render_tabbar(f, chunks[0], app.mode.as_index());

    // Render active panel content.
    let content = chunks[1];
    match app.mode {
        AppMode::Dashboard => ui::dashboard::render(f, app, content),
        AppMode::Proxies => ui::proxies::render(f, app, content),
        AppMode::Connections => ui::connections::render(f, app, content),
        AppMode::Logs => ui::logs::render(f, app, content),
        AppMode::Rules => ui::rules::render(f, app, content),
        AppMode::Config => ui::config_editor::render(f, app, content),
        AppMode::Providers => ui::providers::render(f, app, content),
        AppMode::Kernel => ui::kernel::render(f, app, content),
    }

    // Render status bar.
    let mode_str = match app.cfg.config.as_ref().map(|c| &c.mode) {
        Some(api::types::ClashMode::Rule) => "RULE",
        Some(api::types::ClashMode::Global) => "GLOBAL",
        Some(api::types::ClashMode::Direct) => "DIRECT",
        None => "—",
    };
    let status_data = StatusBarData {
        mode: mode_str.to_owned(),
        upload: format_bytes(app.traffic_up_history.last().copied().unwrap_or(0)),
        download: format_bytes(app.traffic_down_history.last().copied().unwrap_or(0)),
        version: if app.version.is_empty() {
            "—".to_owned()
        } else {
            app.version.clone()
        },
    };
    render_statusbar(f, chunks[2], &status_data);

    // Overlay: help.
    if app.show_help {
        render_help(f, size);
    }

    // Overlay: search bar.
    if app.search.active || !app.search.query.is_empty() {
        let search_area = Rect {
            x: size.x + 1,
            y: size.y + 1,
            width: size.width.saturating_sub(2),
            height: 1,
        };
        ui::components::render_searchbar(f, search_area, &app.search.query, app.search.active);
    }
}

/// Format bytes into a human-readable speed string.
fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.1} GB/s", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB/s", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB/s", bytes as f64 / KB as f64)
    } else {
        format!("{bytes} B/s")
    }
}
