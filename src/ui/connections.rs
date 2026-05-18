//! Active connections panel.
//!
//! Displays a sortable table of all active connections with columns:
//! Host, Network, Type, Rule, Chain, DL, UL, Time.
//!
//! Key bindings:
//! - `j`/`↓` / `k`/`↑`   – navigate
//! - `<` / `>`            – sort by previous/next column
//! - `d`                  – close selected connection
//! - `D`                  – close all connections

use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Cell, Row, Table};

use super::theme;
use crate::api::types::ConnectionItem;
use crate::app::App;
use crate::utils::format::{bytes_to_human, pad_or_truncate};

// ═══════════════════════════════════════════════════════════════════════════
// Column definitions
// ═══════════════════════════════════════════════════════════════════════════

const COLUMN_HEADERS: &[&str] = &["Host", "Net", "Type", "Rule", "Chain", "DL", "UL", "Time"];

/// Constraint widths for each column.
fn column_constraints() -> Vec<Constraint> {
    vec![
        Constraint::Min(20),    // Host
        Constraint::Length(5),  // Network
        Constraint::Length(6),  // Type
        Constraint::Min(12),    // Rule
        Constraint::Min(10),    // Chain
        Constraint::Length(10), // DL
        Constraint::Length(10), // UL
        Constraint::Length(8),  // Time
    ]
}

// ═══════════════════════════════════════════════════════════════════════════
// Public render entry point
// ═══════════════════════════════════════════════════════════════════════════

/// Render the connections panel.
pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let conn_state = &app.connections;
    let active = &conn_state.connections;

    let total_count = active.len();
    let total_dl = bytes_to_human(conn_state.download_total);
    let total_ul = bytes_to_human(conn_state.upload_total);

    let title = format!(" Connections ({total_count}) ↓{total_dl} ↑{total_ul} ");

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme::border())
        .title(Span::styled(format!(" {title} "), theme::title()));
    let inner = block.inner(area);
    f.render_widget(block, area);

    // Build header row.
    let sort_idx = conn_state.sort_col.min(COLUMN_HEADERS.len() - 1);
    let sort_arrow = if conn_state.sort_desc { "▼" } else { "▲" };
    let header_cells: Vec<Cell> = COLUMN_HEADERS
        .iter()
        .enumerate()
        .map(|(i, &h)| {
            let label = if i == sort_idx {
                format!("{sort_arrow}{h}")
            } else {
                h.to_owned()
            };
            Cell::from(Span::styled(label, theme::header()))
        })
        .collect();
    let header = Row::new(header_cells).style(theme::base()).bottom_margin(1);

    // Build data rows.
    let rows: Vec<Row> = active
        .iter()
        .enumerate()
        .map(|(i, conn)| {
            let host = conn_host(conn);
            let chain = conn.chains.join("→");
            let dl = bytes_to_human(conn.download);
            let ul = bytes_to_human(conn.upload);
            let time = conn.start.chars().take(8).collect::<String>();

            let cells = vec![
                Cell::from(Span::styled(pad_or_truncate(&host, 20), theme::fg())),
                Cell::from(Span::styled(&conn.metadata.network, theme::dimmed())),
                Cell::from(Span::styled(&conn.r#type, theme::dimmed())),
                Cell::from(Span::styled(pad_or_truncate(&conn.rule, 12), theme::fg())),
                Cell::from(Span::styled(pad_or_truncate(&chain, 10), theme::mauve())),
                Cell::from(Span::styled(dl, theme::cyan())),
                Cell::from(Span::styled(ul, theme::success())),
                Cell::from(Span::styled(time, theme::dimmed())),
            ];

            let style = if i == conn_state.selected {
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

// ═══════════════════════════════════════════════════════════════════════════
// Helpers
// ═══════════════════════════════════════════════════════════════════════════

fn conn_host(c: &ConnectionItem) -> String {
    let host = if c.host.is_empty() {
        let meta = &c.metadata;
        if meta.host.is_empty() {
            format!("{}:{}", meta.destination_ip, meta.destination_port)
        } else {
            format!("{}:{}", meta.host, meta.destination_port)
        }
    } else {
        c.host.clone()
    };
    pad_or_truncate(&host, 20)
}
