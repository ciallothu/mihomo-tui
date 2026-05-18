//! Rules viewer panel.
//!
//! Displays a scrollable table of all active rules fetched from `GET /rules`.
//! Columns: Type, Payload, Proxy, Size.
//!
//! Key bindings:
//! - `j`/`↓` / `k`/`↑` – navigate
//! - `/` – search / filter
//! - `G` / `g` – jump to bottom / top

use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Cell, Row, Table};

use super::theme;
use crate::api::types::RuleItem;
use crate::app::App;
use crate::utils::format::pad_or_truncate;

// ═══════════════════════════════════════════════════════════════════════════
// Column definitions
// ═══════════════════════════════════════════════════════════════════════════

const COLUMN_HEADERS: &[&str] = &["Type", "Payload", "Proxy", "Size"];

fn column_constraints() -> Vec<Constraint> {
    vec![
        Constraint::Length(16), // Type
        Constraint::Min(20),    // Payload
        Constraint::Length(16), // Proxy
        Constraint::Length(10), // Size
    ]
}

// ═══════════════════════════════════════════════════════════════════════════
// Public render entry point
// ═══════════════════════════════════════════════════════════════════════════

/// Render the rules panel.
pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let rules = &app.rules.rules;

    // Apply search filter.
    let filtered: Vec<&RuleItem> = if app.search.query.is_empty() {
        rules.iter().collect()
    } else {
        let q = app.search.query.to_lowercase();
        rules
            .iter()
            .filter(|r| {
                r.rule_type.to_lowercase().contains(&q)
                    || r.payload.to_lowercase().contains(&q)
                    || r.proxy.to_lowercase().contains(&q)
            })
            .collect()
    };

    let total = filtered.len();
    let title = format!(" Rules ({total}) ");

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme::border())
        .title(Span::styled(title, theme::title()));
    let inner = block.inner(area);
    f.render_widget(block, area);

    // Header row.
    let header_cells: Vec<Cell> = COLUMN_HEADERS
        .iter()
        .map(|&h| Cell::from(Span::styled(h, theme::header())))
        .collect();
    let header = Row::new(header_cells).style(theme::base()).bottom_margin(1);

    // 计算可视区域高度，用于滚动
    let visible_height = inner.height as usize;
    let scroll_offset =
        (app.rules.scroll_offset as usize).min(total.saturating_sub(visible_height));
    let end = (scroll_offset + visible_height).min(total);

    let rows: Vec<Row> = filtered[scroll_offset..end]
        .iter()
        .enumerate()
        .map(|(i, rule)| {
            let size_str = rule
                .size
                .map(|s| crate::utils::format::bytes_to_human(s as u64))
                .unwrap_or_else(|| "—".to_owned());

            let cells = vec![
                Cell::from(Span::styled(
                    pad_or_truncate(&rule.rule_type, 16),
                    theme::mauve(),
                )),
                Cell::from(Span::styled(
                    pad_or_truncate(&rule.payload, 30),
                    theme::fg(),
                )),
                Cell::from(Span::styled(
                    pad_or_truncate(&rule.proxy, 16),
                    theme::accent(),
                )),
                Cell::from(Span::styled(size_str, theme::dimmed())),
            ];

            let style = if i + scroll_offset == app.rules.selected {
                theme::selected()
            } else {
                theme::base()
            };

            Row::new(cells).style(style)
        })
        .collect();

    let table = Table::new(rows, column_constraints())
        .header(header)
        .block(Block::default())
        .style(theme::base());

    f.render_widget(table, inner);
}
