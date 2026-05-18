//! Dashboard / overview panel.
//!
//! Shows real-time traffic graphs, current clash mode, active connection count,
//! memory usage, total traffic, and version information.

use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};

use super::components::render_sparkline;
use super::theme;
use crate::app::App;

// ═══════════════════════════════════════════════════════════════════════════
// Public render entry point
// ═══════════════════════════════════════════════════════════════════════════

/// Render the dashboard panel into `area`.
pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let outer = Block::default()
        .borders(Borders::ALL)
        .border_style(theme::border())
        .title(Span::styled(" Dashboard ", theme::title()));
    let inner = outer.inner(area);
    f.render_widget(outer, area);

    // Layout: top half for traffic graphs, bottom half for stats grid.
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(40), // traffic graphs
            Constraint::Min(0),         // stats
        ])
        .split(inner);

    render_traffic_graphs(f, app, chunks[0]);
    render_stats(f, app, chunks[1]);
}

// ═══════════════════════════════════════════════════════════════════════════
// Traffic graphs (sparkline for upload / download)
// ═══════════════════════════════════════════════════════════════════════════

fn render_traffic_graphs(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    let current_up = app.traffic_up_history.last().copied().unwrap_or(0);
    let current_down = app.traffic_down_history.last().copied().unwrap_or(0);

    // Upload sparkline.
    let up_data: Vec<u64> = app.traffic_up_history.to_vec();
    let up_title = format!("Upload {}", crate::utils::format::format_speed(current_up));
    render_sparkline(f, chunks[0], &up_data, &up_title, theme::GREEN);

    // Download sparkline.
    let down_data: Vec<u64> = app.traffic_down_history.to_vec();
    let down_title = format!(
        "Download {}",
        crate::utils::format::format_speed(current_down)
    );
    render_sparkline(f, chunks[1], &down_data, &down_title, theme::CYAN);
}

// ═══════════════════════════════════════════════════════════════════════════
// Stats grid
// ═══════════════════════════════════════════════════════════════════════════

fn render_stats(f: &mut Frame, app: &App, area: Rect) {
    // Split into two columns.
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    render_stats_left(f, app, cols[0]);
    render_stats_right(f, app, cols[1]);
}

fn render_stats_left(f: &mut Frame, app: &App, area: Rect) {
    let mode_str = app
        .cfg
        .config
        .as_ref()
        .map(|c| format!("{:?}", c.mode))
        .unwrap_or_else(|| "—".to_owned());

    let conn_count = app.connections.connections.len();

    let mem_str = if app.memory_inuse > 0 {
        crate::utils::format::bytes_to_human(app.memory_inuse)
    } else {
        "—".to_owned()
    };

    let total_up = app.connections.upload_total;
    let total_down = app.connections.download_total;

    let lines = vec![
        Line::from(vec![
            Span::styled(" Mode:       ", theme::accent()),
            Span::styled(&mode_str, theme::mauve()),
        ]),
        Line::from(vec![
            Span::styled(" Connections: ", theme::accent()),
            Span::styled(conn_count.to_string(), theme::fg()),
        ]),
        Line::from(vec![
            Span::styled(" Memory:      ", theme::accent()),
            Span::styled(mem_str, theme::fg()),
        ]),
        Line::from(vec![
            Span::styled(" Total ↑:     ", theme::accent()),
            Span::styled(
                crate::utils::format::bytes_to_human(total_up),
                theme::success(),
            ),
        ]),
        Line::from(vec![
            Span::styled(" Total ↓:     ", theme::accent()),
            Span::styled(
                crate::utils::format::bytes_to_human(total_down),
                theme::cyan(),
            ),
        ]),
    ];

    let paragraph = Paragraph::new(lines).style(theme::base());
    f.render_widget(paragraph, area);
}

fn render_stats_right(f: &mut Frame, app: &App, area: Rect) {
    let version_str = if app.version.is_empty() {
        "—".to_owned()
    } else {
        app.version.clone()
    };

    let tun_str = app
        .cfg
        .config
        .as_ref()
        .map(|c| if c.tun.enable { "Active" } else { "Inactive" }.to_owned())
        .unwrap_or_else(|| "—".to_owned());

    let log_level = app
        .cfg
        .config
        .as_ref()
        .map(|c| c.log_level.clone())
        .unwrap_or_else(|| "—".to_owned());

    let mixed_port = app
        .cfg
        .config
        .as_ref()
        .map(|c| {
            if c.mixed_port > 0 {
                c.mixed_port.to_string()
            } else {
                c.port.to_string()
            }
        })
        .unwrap_or_else(|| "—".to_owned());

    let allow_lan = app
        .cfg
        .config
        .as_ref()
        .map(|c| if c.allow_lan { "Yes" } else { "No" })
        .unwrap_or("—");

    let lines = vec![
        Line::from(vec![
            Span::styled(" Version:     ", theme::accent()),
            Span::styled(version_str, theme::peach()),
        ]),
        Line::from(vec![
            Span::styled(" TUN:         ", theme::accent()),
            Span::styled(tun_str, theme::fg()),
        ]),
        Line::from(vec![
            Span::styled(" Log Level:   ", theme::accent()),
            Span::styled(log_level, theme::fg()),
        ]),
        Line::from(vec![
            Span::styled(" Mixed Port:  ", theme::accent()),
            Span::styled(mixed_port, theme::fg()),
        ]),
        Line::from(vec![
            Span::styled(" Allow LAN:   ", theme::accent()),
            Span::styled(allow_lan, theme::fg()),
        ]),
    ];

    let paragraph = Paragraph::new(lines).style(theme::base());
    f.render_widget(paragraph, area);
}

/// Shorthand to get fg() style.
#[allow(dead_code)]
fn theme_fg() -> Style {
    theme::base()
}
