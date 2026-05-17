use std::{
    io::{self, Stdout},
    time::Duration,
};

use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    prelude::{Color, Line, Modifier, Span, Style},
    widgets::{Block, Borders, Gauge, List, ListItem, Paragraph, Tabs, Wrap},
};

use crate::{
    app::{App, PORT_FIELDS, Tab},
    panel::{ConnectionInfo, ExternalResource, ProxyGroup, ResourceKind},
};

type Tui = Terminal<CrosstermBackend<Stdout>>;

pub async fn run(mut app: App) -> Result<()> {
    let mut terminal = init_terminal()?;
    app.load_initial().await;
    let result = run_loop(&mut terminal, &mut app).await;
    restore_terminal(&mut terminal)?;
    result
}

fn init_terminal() -> Result<Tui> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    Ok(Terminal::new(backend)?)
}

fn restore_terminal(terminal: &mut Tui) -> Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}

async fn run_loop(terminal: &mut Tui, app: &mut App) -> Result<()> {
    while !app.should_quit {
        terminal.draw(|frame| render(frame, app))?;

        if event::poll(Duration::from_millis(250))? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => handle_key(app, key).await,
                Event::Resize(_, _) => {}
                _ => {}
            }
        } else {
            app.tick().await;
        }
    }

    Ok(())
}

async fn handle_key(app: &mut App, key: KeyEvent) {
    match (key.code, key.modifiers) {
        (KeyCode::Char('q'), _) | (KeyCode::Esc, _) => app.quit(),
        (KeyCode::Char('c'), KeyModifiers::CONTROL) => app.quit(),
        (KeyCode::Tab, _) => app.next_tab(),
        (KeyCode::BackTab, _) => app.previous_tab(),
        (KeyCode::Right, _) | (KeyCode::Char('l'), _) => app.next_tab(),
        (KeyCode::Left, _) | (KeyCode::Char('h'), _) => app.previous_tab(),
        (KeyCode::Down, _) | (KeyCode::Char('j'), _) => app.move_down(),
        (KeyCode::Up, _) | (KeyCode::Char('k'), _) => app.move_up(),
        (KeyCode::Enter, _) | (KeyCode::Char(' '), _) => app.activate().await,
        (KeyCode::Char('m'), _) => app.cycle_mode().await,
        (KeyCode::Char('r'), _) => app.refresh().await,
        (KeyCode::Char('x'), _) => app.close_all_connections().await,
        (KeyCode::Char('+'), _) | (KeyCode::Char('='), _) if app.active_tab == Tab::Ports => {
            app.bump_selected_port(1)
        }
        (KeyCode::Char('-'), _) if app.active_tab == Tab::Ports => app.bump_selected_port(-1),
        _ => {}
    }
}

fn render(frame: &mut Frame<'_>, app: &App) {
    let area = frame.area();
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(8),
            Constraint::Length(3),
        ])
        .split(area);

    render_header(frame, app, layout[0]);
    match app.active_tab {
        Tab::Overview => render_overview(frame, app, layout[1]),
        Tab::Proxies => render_proxies(frame, app, layout[1]),
        Tab::Resources => render_resources(frame, app, layout[1]),
        Tab::Ports => render_ports(frame, app, layout[1]),
        Tab::Connections => render_connections(frame, app, layout[1]),
        Tab::Logs => render_logs(frame, app, layout[1]),
    }
    render_footer(frame, app, layout[2]);
}

fn render_header(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(26), Constraint::Min(20)])
        .split(area);

    let title = Paragraph::new(Line::from(vec![
        Span::styled(
            "mihomo",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("-tui "),
        Span::styled(app.mode.to_string(), Style::default().fg(Color::Yellow)),
    ]))
    .block(block("Workspace"))
    .alignment(Alignment::Center);
    frame.render_widget(title, chunks[0]);

    let tabs = Tabs::new(
        Tab::ALL
            .iter()
            .map(|tab| Line::from(tab.title()))
            .collect::<Vec<_>>(),
    )
    .select(app.active_tab.index())
    .block(block("Panel"))
    .highlight_style(
        Style::default()
            .fg(Color::Black)
            .bg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )
    .style(Style::default().fg(Color::Gray));
    frame.render_widget(tabs, chunks[1]);
}

fn render_footer(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let help = Line::from(vec![
        Span::styled(" q ", key_style()),
        Span::raw("quit "),
        Span::styled(" tab/h/l ", key_style()),
        Span::raw("switch "),
        Span::styled(" j/k ", key_style()),
        Span::raw("move "),
        Span::styled(" enter/space ", key_style()),
        Span::raw("apply "),
        Span::styled(" m ", key_style()),
        Span::raw("mode "),
        Span::styled(" r ", key_style()),
        Span::raw("refresh "),
        Span::styled(" x ", key_style()),
        Span::raw("close-all "),
        Span::styled(
            format!("  {}", app.status_message),
            Style::default().fg(Color::Green),
        ),
    ]);

    frame.render_widget(
        Paragraph::new(help)
            .block(block("Keys"))
            .wrap(Wrap { trim: true }),
        area,
    );
}

fn render_overview(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(7), Constraint::Min(10)])
        .split(area);
    let top = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(30),
            Constraint::Percentage(35),
            Constraint::Percentage(35),
        ])
        .split(rows[0]);

    render_core_card(frame, app, top[0]);
    render_traffic_card(frame, app, top[1]);
    render_ports_card(frame, app, top[2]);

    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(45),
            Constraint::Percentage(25),
            Constraint::Percentage(30),
        ])
        .split(rows[1]);
    render_group_list(frame, app, body[0]);
    render_resource_list(frame, app, body[1]);
    render_log_list(frame, app, body[2]);
}

fn render_core_card(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let core_path = app
        .core
        .binary_path
        .as_ref()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "not configured".to_string());
    let text = vec![
        Line::from(vec![
            Span::raw("Status: "),
            status_span(app.core.status.to_string()),
        ]),
        Line::from(vec![
            Span::raw("Version: "),
            Span::styled(&app.core.version, Color::Yellow),
        ]),
        Line::from(vec![Span::raw("Binary: "), Span::raw(core_path)]),
        Line::from(vec![
            Span::raw("Data: "),
            Span::raw(app.paths.root.display().to_string()),
        ]),
    ];
    frame.render_widget(
        Paragraph::new(text)
            .block(block("Mihomo Core"))
            .wrap(Wrap { trim: true }),
        area,
    );
}

fn render_traffic_card(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let traffic = &app.snapshot.traffic;
    let text = vec![
        Line::from(format!("Down  {}", format_bytes(traffic.download_bps))),
        Line::from(format!("Up    {}", format_bytes(traffic.upload_bps))),
        Line::from(format!("Conn  {}", traffic.active_connections)),
        Line::from(format!("Mem   {} MB", traffic.memory_mb)),
    ];
    frame.render_widget(Paragraph::new(text).block(block("Runtime")), area);
}

fn render_ports_card(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let ports = &app.snapshot.ports;
    let text = vec![
        Line::from(format!("mixed               {}", ports.mixed)),
        Line::from(format!("socks               {}", ports.socks)),
        Line::from(format!("http                {}", ports.http)),
        Line::from(format!("external-controller {}", ports.external_controller)),
    ];
    frame.render_widget(Paragraph::new(text).block(block("Listeners")), area);
}

fn render_proxies(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(area);
    render_group_list(frame, app, columns[0]);

    let group = app.snapshot.groups.get(app.selected_group);
    let items = group
        .map(proxy_items)
        .unwrap_or_else(|| vec![ListItem::new("No proxy groups")]);

    frame.render_widget(List::new(items).block(block("Nodes")), columns[1]);
}

fn render_resources(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(area);
    render_resource_list(frame, app, columns[0]);

    let selected = app.snapshot.resources.get(app.selected_resource);
    let details = selected
        .map(resource_details)
        .unwrap_or_else(|| vec![Line::from("No resources")]);
    frame.render_widget(
        Paragraph::new(details)
            .block(block("Resource Detail"))
            .wrap(Wrap { trim: true }),
        columns[1],
    );
}

fn render_ports(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let ports = &app.snapshot.ports;
    let ratio = ports.mixed.saturating_sub(1024) as f64 / (65535 - 1024) as f64;
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(5), Constraint::Min(8)])
        .split(area);

    frame.render_widget(
        Gauge::default()
            .block(block("Mixed Port"))
            .gauge_style(Style::default().fg(Color::Cyan))
            .ratio(ratio)
            .label(format!("{}", ports.mixed)),
        rows[0],
    );

    let text = vec![
        Line::from("Use j/k to select, +/- to change, then Enter to apply through /configs."),
        port_line(app, 0, ports.http),
        port_line(app, 1, ports.socks),
        port_line(app, 2, ports.mixed),
        port_line(app, 3, ports.redir.unwrap_or(0)),
        port_line(app, 4, ports.tproxy.unwrap_or(0)),
        port_line(app, 5, ports.external_controller),
    ];
    frame.render_widget(
        Paragraph::new(text).block(block("Listener Settings")),
        rows[1],
    );
}

fn render_connections(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(area);

    let items = app
        .snapshot
        .connections
        .iter()
        .enumerate()
        .map(|(index, connection)| connection_item(connection, index == app.selected_connection))
        .collect::<Vec<_>>();
    frame.render_widget(List::new(items).block(block("Connections")), columns[0]);

    let detail = app
        .snapshot
        .connections
        .get(app.selected_connection)
        .map(connection_details)
        .unwrap_or_else(|| vec![Line::from("No active connections")]);
    frame.render_widget(
        Paragraph::new(detail)
            .block(block("Connection Detail"))
            .wrap(Wrap { trim: true }),
        columns[1],
    );
}

fn render_logs(frame: &mut Frame<'_>, app: &App, area: Rect) {
    render_log_list(frame, app, area);
}

fn render_group_list(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let items = app
        .snapshot
        .groups
        .iter()
        .enumerate()
        .map(|(index, group)| group_item(group, index == app.selected_group))
        .collect::<Vec<_>>();
    frame.render_widget(List::new(items).block(block("Proxy Groups")), area);
}

fn render_resource_list(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let items = app
        .snapshot
        .resources
        .iter()
        .enumerate()
        .map(|(index, resource)| resource_item(resource, index == app.selected_resource))
        .collect::<Vec<_>>();
    frame.render_widget(List::new(items).block(block("External Resources")), area);
}

fn render_log_list(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let len = app.snapshot.logs.len();
    let start = len.saturating_sub(area.height.saturating_sub(2) as usize);
    let items = app.snapshot.logs[start..]
        .iter()
        .enumerate()
        .map(|(offset, entry)| {
            let index = start + offset;
            let style = if index == app.selected_log {
                selected_style()
            } else {
                Style::default()
            };
            ListItem::new(Line::from(vec![
                Span::styled(entry.time.format("%H:%M:%S").to_string(), Color::DarkGray),
                Span::raw(" "),
                Span::styled(entry.level.to_uppercase(), level_style(&entry.level)),
                Span::raw(" "),
                Span::raw(&entry.message),
            ]))
            .style(style)
        })
        .collect::<Vec<_>>();
    frame.render_widget(List::new(items).block(block("Logs")), area);
}

fn group_item(group: &ProxyGroup, selected: bool) -> ListItem<'static> {
    let current = group
        .proxies
        .get(group.selected)
        .map(|proxy| proxy.name.as_str())
        .unwrap_or("none");
    let line = Line::from(vec![
        marker(selected),
        Span::styled(group.name.clone(), Style::default().fg(Color::Cyan)),
        Span::raw(" -> "),
        Span::styled(current.to_string(), Style::default().fg(Color::Yellow)),
    ]);
    ListItem::new(line).style(if selected {
        selected_style()
    } else {
        Style::default()
    })
}

fn proxy_items(group: &ProxyGroup) -> Vec<ListItem<'static>> {
    group
        .proxies
        .iter()
        .enumerate()
        .map(|(index, proxy)| {
            let selected = index == group.selected;
            let delay = proxy
                .delay_ms
                .map(|delay| format!("{delay} ms"))
                .unwrap_or_else(|| "direct".to_string());
            let state = if proxy.alive { "up" } else { "down" };
            ListItem::new(Line::from(vec![
                marker(selected),
                Span::raw(proxy.name.clone()),
                Span::raw("  "),
                Span::styled(delay, Color::Yellow),
                Span::raw("  "),
                Span::styled(
                    state,
                    if proxy.alive {
                        Color::Green
                    } else {
                        Color::Red
                    },
                ),
            ]))
            .style(if selected {
                selected_style()
            } else {
                Style::default()
            })
        })
        .collect()
}

fn resource_item(resource: &ExternalResource, selected: bool) -> ListItem<'static> {
    let kind = match resource.kind {
        ResourceKind::ProxyProvider => "proxy",
        ResourceKind::RuleProvider => "rule ",
    };
    ListItem::new(Line::from(vec![
        marker(selected),
        Span::styled(kind, Color::Green),
        Span::raw(" "),
        Span::raw(resource.name.clone()),
    ]))
    .style(if selected {
        selected_style()
    } else {
        Style::default()
    })
}

fn resource_details(resource: &ExternalResource) -> Vec<Line<'static>> {
    let updated = resource
        .updated_at
        .clone()
        .unwrap_or_else(|| "unknown".to_string());
    let kind = match resource.kind {
        ResourceKind::ProxyProvider => "proxy-provider",
        ResourceKind::RuleProvider => "rule-provider",
    };
    vec![
        Line::from(vec![
            Span::raw("Name: "),
            Span::styled(resource.name.clone(), Color::Cyan),
        ]),
        Line::from(vec![Span::raw("Kind: "), Span::raw(kind)]),
        Line::from(vec![Span::raw("Updated: "), Span::raw(updated)]),
        Line::from(vec![Span::raw("Source: "), Span::raw(resource.url.clone())]),
        Line::from("Press Enter to refresh this provider through mihomo."),
    ]
}

fn connection_item(connection: &ConnectionInfo, selected: bool) -> ListItem<'static> {
    ListItem::new(Line::from(vec![
        marker(selected),
        Span::raw(connection.host.clone()),
        Span::raw(" "),
        Span::styled(format_bytes(connection.download), Color::Green),
    ]))
    .style(if selected {
        selected_style()
    } else {
        Style::default()
    })
}

fn connection_details(connection: &ConnectionInfo) -> Vec<Line<'static>> {
    vec![
        Line::from(vec![Span::raw("ID: "), Span::raw(connection.id.clone())]),
        Line::from(vec![
            Span::raw("Host: "),
            Span::styled(connection.host.clone(), Color::Cyan),
        ]),
        Line::from(vec![
            Span::raw("Rule: "),
            Span::raw(connection.rule.clone()),
        ]),
        Line::from(vec![
            Span::raw("Chain: "),
            Span::raw(connection.chain.clone()),
        ]),
        Line::from(vec![
            Span::raw("Upload: "),
            Span::raw(format_bytes(connection.upload)),
        ]),
        Line::from(vec![
            Span::raw("Download: "),
            Span::raw(format_bytes(connection.download)),
        ]),
    ]
}

fn block(title: &'static str) -> Block<'static> {
    Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
}

fn marker(selected: bool) -> Span<'static> {
    if selected {
        Span::styled(
            "> ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        Span::raw("  ")
    }
}

fn selected_style() -> Style {
    Style::default().bg(Color::Rgb(24, 48, 56))
}

fn key_style() -> Style {
    Style::default()
        .fg(Color::Black)
        .bg(Color::Cyan)
        .add_modifier(Modifier::BOLD)
}

fn level_style(level: &str) -> Style {
    match level {
        "warn" => Style::default().fg(Color::Yellow),
        "error" => Style::default().fg(Color::Red),
        _ => Style::default().fg(Color::Green),
    }
}

fn status_span(status: String) -> Span<'static> {
    let color = match status.as_str() {
        "Running" => Color::Green,
        "Stopped" => Color::Yellow,
        _ => Color::Red,
    };
    Span::styled(status, Style::default().fg(color))
}

fn port_line(app: &App, index: usize, value: u16) -> Line<'static> {
    let (label, key) = PORT_FIELDS[index];
    let display = if value == 0 {
        "disabled".to_string()
    } else {
        value.to_string()
    };
    Line::from(vec![
        marker(app.selected_port == index),
        Span::styled(label, Color::Cyan),
        Span::raw(format!(" ({key}) ")),
        Span::styled(display, Color::Yellow),
    ])
}

fn format_bytes(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    let value = bytes as f64;
    if value >= MB {
        format!("{:.1} MB/s", value / MB)
    } else if value >= KB {
        format!("{:.1} KB/s", value / KB)
    } else {
        format!("{bytes} B/s")
    }
}
