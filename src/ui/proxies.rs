//! Proxy management panel.
//!
//! Left: proxy groups list with type indicator.
//! Right: nodes in the selected group with colour-coded latency bars.
//!
//! Key bindings handled by [`crate::app`]:
//! - `j`/`↓` / `k`/`↑` – navigate
//! - `Enter`            – select node (selector groups)
//! - `t`                – test current node latency
//! - `T`                – test all nodes in group
//! - `/`                – filter/search nodes

use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};

use super::theme;
use crate::app::App;
use crate::utils::format::pad_or_truncate;

// ═══════════════════════════════════════════════════════════════════════════
// Public render entry point
// ═══════════════════════════════════════════════════════════════════════════

/// Render the proxies panel.
pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let outer = Block::default()
        .borders(Borders::ALL)
        .border_style(theme::border())
        .title(Span::styled(" Proxies ", theme::title()));
    let inner = outer.inner(area);
    f.render_widget(outer, area);

    // Split into left (groups) and right (nodes).
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(28), // groups list
            Constraint::Min(0),     // nodes list
        ])
        .split(inner);

    render_groups(f, app, chunks[0]);
    render_nodes(f, app, chunks[1]);
}

// ═══════════════════════════════════════════════════════════════════════════
// Groups list (left panel)
// ═══════════════════════════════════════════════════════════════════════════

fn render_groups(f: &mut Frame, app: &App, area: Rect) {
    let groups = &app.proxy.groups;

    if groups.is_empty() {
        return render_loading(f, area, "No proxy groups found");
    }

    let items: Vec<ListItem> = groups
        .iter()
        .enumerate()
        .map(|(i, name)| {
            let now_indicator = match app.proxy.group_now.get(name) {
                Some(n) if !n.is_empty() => format!(" → {n}"),
                _ => String::new(),
            };

            let label = format!("● {name}{now_indicator}");
            let display = pad_or_truncate(&label, area.width as usize);

            let style = if i == app.proxy.selected_group {
                theme::selected()
            } else {
                theme::base()
            };

            ListItem::new(Line::from(Span::styled(display, style)))
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::NONE)
            .title(Span::styled(" Groups ", theme::title())),
    );

    f.render_widget(list, area);
}

// ═══════════════════════════════════════════════════════════════════════════
// Nodes list (right panel)
// ═══════════════════════════════════════════════════════════════════════════

fn render_nodes(f: &mut Frame, app: &App, area: Rect) {
    let group_name = match app.proxy.groups.get(app.proxy.selected_group) {
        Some(name) => name.as_str(),
        None => return render_loading(f, area, "No group selected"),
    };

    let members: &[String] = match app.proxy.group_members.get(group_name) {
        Some(m) => m,
        None => return render_loading(f, area, "No members"),
    };

    // Apply search filter.
    let filtered: Vec<&str> = if app.search.query.is_empty() {
        members.iter().map(|s| s.as_str()).collect()
    } else {
        members
            .iter()
            .filter(|n| n.to_lowercase().contains(&app.search.query.to_lowercase()))
            .map(|s| s.as_str())
            .collect()
    };

    let title_str = format!(" {} ({}) ", group_name, filtered.len());

    let current_now = app
        .proxy
        .group_now
        .get(group_name)
        .map(|s| s.as_str())
        .unwrap_or("");

    let items: Vec<ListItem> = filtered
        .iter()
        .enumerate()
        .map(|(i, &name)| {
            let delay = app.proxy.delays.get(name).copied().unwrap_or(0);

            let is_current = name == current_now;

            let delay_str = if delay == 0 {
                "timeout".to_owned()
            } else {
                format!("{delay}ms")
            };

            let selector = if is_current { "●" } else { " " };

            let label = format!(
                "{selector} {:width$} {:>8}",
                name,
                delay_str,
                width = (area.width as usize).saturating_sub(12),
            );
            let display = pad_or_truncate(&label, area.width as usize);

            let style = if i == app.proxy.selected_node {
                theme::selected()
            } else if is_current {
                theme::accent()
            } else {
                theme::latency_style(delay)
            };

            ListItem::new(Line::from(Span::styled(display, style)))
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::NONE)
            .title(Span::styled(title_str, theme::title())),
    );

    f.render_widget(list, area);
}

// ═══════════════════════════════════════════════════════════════════════════
// Helpers
// ═══════════════════════════════════════════════════════════════════════════

fn render_loading(f: &mut Frame, area: Rect, msg: &str) {
    let paragraph = Paragraph::new(msg).style(theme::dimmed());
    f.render_widget(paragraph, area);
}
