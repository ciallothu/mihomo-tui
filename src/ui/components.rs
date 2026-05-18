//! Reusable ratatui widgets.
//!
//! - [`render_tabbar`]   – Horizontal tab strip with highlighted active tab.
//! - [`render_statusbar`] – Bottom bar showing mode, traffic, version.
//! - [`render_sparkline`] – Traffic history graph.
//! - [`render_searchbar`] – Inline search / filter input.
//! - [`render_popup`]    – Modal overlay for confirmations.
//! - [`render_help`]     – Keyboard shortcuts reference.

use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Sparkline, Wrap};

use super::theme;

// ═══════════════════════════════════════════════════════════════════════════
// Tab names used across the application
// ═══════════════════════════════════════════════════════════════════════════

/// Tab identifiers corresponding to [`crate::app::AppMode`].
pub const TABS: &[&str] = &[
    "Dashboard",
    "Proxies",
    "Connections",
    "Logs",
    "Rules",
    "Config",
    "Providers",
    "Kernel",
];

// ═══════════════════════════════════════════════════════════════════════════
// TabBar
// ═══════════════════════════════════════════════════════════════════════════

/// Given the tab-bar area and a column offset, return the tab index
/// that was clicked (if any).
///
/// Each tab is rendered as `" {name} "` – we accumulate widths and
/// check whether `col` falls inside one of them.
pub fn tab_hit_test(area: Rect, col: u16) -> Option<usize> {
    if area.height == 0 || col < area.x {
        return None;
    }
    let mut x = area.x;
    for (i, name) in TABS.iter().enumerate() {
        let width = name.len() as u16 + 2; // " {name} "
        if col >= x && col < x + width {
            return Some(i);
        }
        x += width;
    }
    None
}

/// Render a horizontal tab bar at the top of the screen.
pub fn render_tabbar(f: &mut Frame, area: Rect, active_index: usize) {
    let spans: Vec<Span> = TABS
        .iter()
        .enumerate()
        .map(|(i, name)| {
            let label = format!(" {name} ");
            if i == active_index {
                Span::styled(label, theme::tab_active())
            } else {
                Span::styled(label, theme::tab_inactive())
            }
        })
        .collect();

    let paragraph = Paragraph::new(Line::from(spans)).style(theme::base());
    f.render_widget(paragraph, area);
}

// ═══════════════════════════════════════════════════════════════════════════
// StatusBar
// ═══════════════════════════════════════════════════════════════════════════

/// Data shown on the status bar.
pub struct StatusBarData {
    pub mode: String,
    pub upload: String,
    pub download: String,
    pub version: String,
}

/// Render the bottom status bar.
pub fn render_statusbar(f: &mut Frame, area: Rect, data: &StatusBarData) {
    let mode_span = Span::styled(format!(" {} ", data.mode), theme::status_mode());
    let divider = Span::styled(" │ ", theme::dimmed());
    let up_icon = Span::styled("↑", theme::success());
    let up_val = Span::styled(format!(" {} ", data.upload), theme::status_bar());
    let down_icon = Span::styled("↓", theme::cyan());
    let down_val = Span::styled(format!(" {} ", data.download), theme::status_bar());
    let version = Span::styled(format!(" {} ", data.version), theme::dimmed());

    let line = Line::from(vec![
        mode_span,
        divider.clone(),
        up_icon,
        up_val,
        divider.clone(),
        down_icon,
        down_val,
        divider,
        version,
    ]);

    let paragraph = Paragraph::new(line).style(theme::status_bar());
    f.render_widget(paragraph, area);
}

// ═══════════════════════════════════════════════════════════════════════════
// SparkLineChart
// ═══════════════════════════════════════════════════════════════════════════

/// Render a sparkline traffic chart.
pub fn render_sparkline(f: &mut Frame, area: Rect, data: &[u64], title: &str, color: Color) {
    let sparkline = Sparkline::default()
        .data(data)
        .style(Style::default().fg(color))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(theme::border())
                .title(Span::styled(format!(" {title} "), theme::title())),
        );
    f.render_widget(sparkline, area);
}

// ═══════════════════════════════════════════════════════════════════════════
// SearchBar
// ═══════════════════════════════════════════════════════════════════════════

/// Render an inline search / filter bar.
pub fn render_searchbar(f: &mut Frame, area: Rect, query: &str, active: bool) {
    let display = if query.is_empty() && !active {
        "Press / to search…".to_owned()
    } else if active {
        format!("/{query}▎")
    } else {
        format!("/{query}")
    };

    let style = if active {
        theme::search_bar()
    } else {
        theme::dimmed()
    };
    let paragraph = Paragraph::new(display).style(style);
    f.render_widget(paragraph, area);
}

// ═══════════════════════════════════════════════════════════════════════════
// Popup
// ═══════════════════════════════════════════════════════════════════════════

/// Render a modal confirmation popup in the centre of the screen.
pub fn render_popup(f: &mut Frame, title: &str, message: &str, area: Rect) {
    let width = 50.min(area.width.saturating_sub(4));
    let height = 5.min(area.height.saturating_sub(4));
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    let popup_area = Rect::new(x, y, width, height);

    f.render_widget(Clear, popup_area);

    let text = vec![
        Line::from(Span::styled(message, theme::base())),
        Line::from(""),
        Line::from(vec![
            Span::styled(" [Enter] ", theme::accent()),
            Span::styled("Confirm  ", theme::base()),
            Span::styled(" [Esc] ", theme::accent()),
            Span::styled("Cancel", theme::base()),
        ]),
    ];

    let paragraph = Paragraph::new(text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(theme::accent())
                .title(Span::styled(format!(" {title} "), theme::title())),
        )
        .wrap(Wrap { trim: true });

    f.render_widget(paragraph, popup_area);
}

// ═══════════════════════════════════════════════════════════════════════════
// HelpOverlay
// ═══════════════════════════════════════════════════════════════════════════

/// Render a keyboard shortcuts help overlay.
pub fn render_help(f: &mut Frame, area: Rect) {
    let width = 54.min(area.width.saturating_sub(4));
    let height = 18.min(area.height.saturating_sub(4));
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    let popup_area = Rect::new(x, y, width, height);

    f.render_widget(Clear, popup_area);

    let key = |k: &str| Span::styled(format!("{k:>10} "), theme::accent());
    let desc = |d: &str| Span::styled(String::from(d), theme::base());

    let lines = vec![
        Line::from(Span::styled(" Keyboard Shortcuts ", theme::title())),
        Line::from(""),
        Line::from(vec![key("Tab/1-8"), desc("Switch panel")]),
        Line::from(vec![key("j/↓"), desc("Move down")]),
        Line::from(vec![key("k/↑"), desc("Move up")]),
        Line::from(vec![key("Enter"), desc("Select / confirm")]),
        Line::from(vec![key("/"), desc("Search / filter")]),
        Line::from(vec![key("Esc"), desc("Cancel / close overlay")]),
        Line::from(vec![key("q"), desc("Quit application")]),
        Line::from(vec![key("?"), desc("Toggle this help")]),
        Line::from(""),
        Line::from(vec![key("t"), desc("Test latency (proxies)")]),
        Line::from(vec![key("T"), desc("Test all latency (proxies)")]),
        Line::from(vec![key("d"), desc("Close connection")]),
        Line::from(vec![key("D"), desc("Close all connections")]),
        Line::from(vec![key("M"), desc("Cycle clash mode")]),
        Line::from(vec![key("u"), desc("Update provider")]),
        Line::from(vec![key("r"), desc("Reload config")]),
    ];

    let paragraph = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(theme::accent()),
    );
    f.render_widget(paragraph, popup_area);
}
