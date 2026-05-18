//! Kernel management panel.
//!
//! Shows the current mihomo kernel version and available versions from
//! GitHub releases.
//!
//! Key bindings:
//! - `j`/`↓` / `k`/`↑` – navigate
//! - `d` – download selected version
//! - `Enter` – switch to selected version
//! - `r` – refresh available versions

use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};

use super::theme;
use crate::app::App;
use crate::utils::format::pad_or_truncate;

// ═══════════════════════════════════════════════════════════════════════════
// Public render entry point
// ═══════════════════════════════════════════════════════════════════════════

/// Render the kernel management panel.
pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme::border())
        .title(Span::styled(" Kernel ", theme::title()));
    let inner = block.inner(area);
    f.render_widget(block, area);

    // 上方：当前版本和平台信息，下方：版本列表
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(6), Constraint::Min(0)])
        .split(inner);

    render_info(f, app, chunks[0]);
    render_version_list(f, app, chunks[1]);
}

// ═══════════════════════════════════════════════════════════════════════════
// 顶部信息区：当前版本、平台、下载状态
// ═══════════════════════════════════════════════════════════════════════════

fn render_info(f: &mut Frame, app: &App, area: Rect) {
    let current = if app.kernel.current_version.is_empty() {
        "Not installed"
    } else {
        &app.kernel.current_version
    };

    let platform_str = format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH);

    let mut lines = vec![
        Line::from(vec![
            Span::styled("  Active Version: ", theme::accent()),
            Span::styled(current, theme::success()),
        ]),
        Line::from(vec![
            Span::styled("  Platform:       ", theme::accent()),
            Span::styled(platform_str, theme::fg()),
        ]),
    ];

    // 下载进度
    if app.kernel.downloading {
        lines.push(Line::from(vec![
            Span::styled("  Status: ", theme::accent()),
            Span::styled("Downloading…", theme::warning()),
        ]));
    } else {
        lines.push(Line::from(vec![
            Span::styled("  Status: ", theme::accent()),
            Span::styled("Ready", theme::success()),
        ]));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("  [d] ", theme::warning()),
        Span::styled("Download  ", theme::fg()),
        Span::styled("[Enter] ", theme::warning()),
        Span::styled("Switch  ", theme::fg()),
        Span::styled("[r] ", theme::warning()),
        Span::styled("Refresh", theme::fg()),
    ]));

    let paragraph = Paragraph::new(lines).style(theme::base());
    f.render_widget(paragraph, area);
}

// ═══════════════════════════════════════════════════════════════════════════
// 版本列表：显示 GitHub releases
// ═══════════════════════════════════════════════════════════════════════════

fn render_version_list(f: &mut Frame, app: &App, area: Rect) {
    let releases = &app.kernel.available_versions;

    if releases.is_empty() {
        let msg = Paragraph::new("No versions available").style(theme::dimmed());
        f.render_widget(msg, area);
        return;
    }

    let items: Vec<ListItem> = releases
        .iter()
        .enumerate()
        .map(|(i, version)| {
            let is_active = version == &app.kernel.current_version;

            let status_icon = if is_active {
                "●" // 当前激活
            } else {
                " " // 可用
            };

            let label = format!(" {} {}", status_icon, version);
            let display = pad_or_truncate(&label, area.width as usize);

            let style = if i == app.kernel.selected {
                theme::selected()
            } else if is_active {
                theme::success()
            } else {
                theme::base()
            };

            ListItem::new(Line::from(Span::styled(display, style)))
        })
        .collect();

    let list = List::new(items);
    f.render_widget(list, area);
}
