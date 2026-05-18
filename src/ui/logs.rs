//! Streaming log viewer panel.
//!
//! Displays real-time logs from the mihomo WebSocket `/logs` endpoint.
//! Log entries are colour-coded by level and stored in a ring buffer.
//!
//! Key bindings:
//! - `j`/`↓` / `k`/`↑`  – scroll
//! - `1`-`5`            – filter by log level (debug/info/warning/error/silent)
//! - `/`                – search / filter
//! - `Enter`            – toggle auto-scroll

use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use super::theme;
use crate::app::App;

// ═══════════════════════════════════════════════════════════════════════════
// Public render entry point
// ═══════════════════════════════════════════════════════════════════════════

/// Render the log viewer panel.
pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let level_filter = &app.logs.level_filter;
    let log_count = app.logs.buffer.len();
    let auto_scroll = if app.logs.auto_scroll { "ON" } else { "OFF" };

    let title = format!(
        " Logs [{}] ({}) autoscroll:{} ",
        level_filter, log_count, auto_scroll
    );

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme::border())
        .title(Span::styled(format!(" {title} "), theme::title()));
    let inner = block.inner(area);
    f.render_widget(block, area);

    // Render log lines.
    let visible_height = inner.height as usize;

    // Apply level filter and search.
    let filtered: Vec<&crate::api::types::LogEntry> = app
        .logs
        .buffer
        .iter()
        .filter(|entry| {
            // Level filter.
            if level_filter != "debug" {
                let entry_level = entry.log_type.to_lowercase();
                match level_filter.as_str() {
                    "info" => entry_level != "debug",
                    "warning" => !matches!(entry_level.as_str(), "debug" | "info"),
                    "error" => entry_level == "error",
                    "silent" => false,
                    _ => true,
                }
            } else {
                true
            }
        })
        .filter(|entry| {
            // Search filter.
            if app.search.query.is_empty() {
                true
            } else {
                entry
                    .payload
                    .to_lowercase()
                    .contains(&app.search.query.to_lowercase())
            }
        })
        .collect();

    let total = filtered.len();

    // Calculate scroll offset.
    let scroll_offset: usize = if app.logs.auto_scroll {
        total.saturating_sub(visible_height)
    } else {
        (app.logs.scroll_offset as usize).min(total.saturating_sub(visible_height))
    };

    let visible_slice =
        &filtered[scroll_offset.min(total)..total.min(scroll_offset + visible_height)];

    let lines: Vec<Line> = visible_slice
        .iter()
        .map(|entry| {
            let level_style = theme::log_level_style(&entry.log_type);
            let level_str = format!("{:>7} ", entry.log_type.to_uppercase());
            vec![
                Span::styled(level_str, level_style),
                Span::styled(&entry.payload, theme::base()),
            ]
        })
        .map(Line::from)
        .collect();

    let paragraph = Paragraph::new(lines)
        .style(theme::base())
        .wrap(Wrap { trim: false });

    f.render_widget(paragraph, inner);
}
