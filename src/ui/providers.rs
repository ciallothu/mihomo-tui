//! Provider management panel.
//!
//! Shows both proxy providers and rule providers with their status.
//! Users can update individual providers.
//!
//! Key bindings:
//! - `j`/`↓` / `k`/`↑` – navigate
//! - `Tab` – switch between proxy/rule provider tabs
//! - `u` – update selected provider
//! - `U` – update all providers

use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};

use super::theme;
use crate::api::types::{ProxyProviderItem, RuleProviderItem};
use crate::app::App;
use crate::utils::format::pad_or_truncate;

// ═══════════════════════════════════════════════════════════════════════════
// Provider tab selection
// ═══════════════════════════════════════════════════════════════════════════

/// Which provider tab is active.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProviderTab {
    #[default]
    Proxy,
    Rule,
}

impl ProviderTab {
    fn from_active_tab(tab: usize) -> Self {
        match tab {
            0 => Self::Proxy,
            _ => Self::Rule,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Public render entry point
// ═══════════════════════════════════════════════════════════════════════════

/// Render the providers panel.
pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme::border())
        .title(Span::styled(" Providers ", theme::title()));
    let inner = block.inner(area);
    f.render_widget(block, area);

    // 顶部 Tab 栏
    let tabs_area = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(inner);

    let active_tab = ProviderTab::from_active_tab(app.providers.active_tab);
    render_provider_tabs(f, active_tab, tabs_area[0]);

    match active_tab {
        ProviderTab::Proxy => render_proxy_providers(f, app, tabs_area[1]),
        ProviderTab::Rule => render_rule_providers(f, app, tabs_area[1]),
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Sub-tabs (Proxy / Rule)
// ═══════════════════════════════════════════════════════════════════════════

fn render_provider_tabs(f: &mut Frame, active: ProviderTab, area: Rect) {
    let proxy_label = " Proxy Providers ";
    let rule_label = " Rule Providers ";

    let spans = vec![
        if active == ProviderTab::Proxy {
            Span::styled(proxy_label, theme::tab_active())
        } else {
            Span::styled(proxy_label, theme::tab_inactive())
        },
        Span::styled(" ", theme::base()),
        if active == ProviderTab::Rule {
            Span::styled(rule_label, theme::tab_active())
        } else {
            Span::styled(rule_label, theme::tab_inactive())
        },
    ];

    let paragraph = Paragraph::new(Line::from(spans)).style(theme::base());
    f.render_widget(paragraph, area);
}

// ═══════════════════════════════════════════════════════════════════════════
// Proxy providers list
// ═══════════════════════════════════════════════════════════════════════════

fn render_proxy_providers(f: &mut Frame, app: &App, area: Rect) {
    let providers = &app.providers.proxy_providers;

    // 排序后显示
    let mut sorted: Vec<(&String, &ProxyProviderItem)> = providers.iter().collect();
    sorted.sort_by(|a, b| a.1.name.cmp(&b.1.name));

    if sorted.is_empty() {
        let msg = Paragraph::new("No proxy providers found").style(theme::dimmed());
        f.render_widget(msg, area);
        return;
    }

    let items: Vec<ListItem> = sorted
        .iter()
        .enumerate()
        .map(|(i, (_key, prov))| {
            let node_count = prov.proxies.len();
            let alive = prov.proxies.iter().filter(|p| p.alive).count();

            let label = format!(
                " {} {:20} type={:<10} nodes={:<4} alive={:<4} updated={}",
                if i == app.providers.selected {
                    "▸"
                } else {
                    " "
                },
                prov.name,
                prov.vehicle_type,
                node_count,
                alive,
                prov.updated,
            );
            let display = pad_or_truncate(&label, area.width as usize);

            let style = if i == app.providers.selected {
                theme::selected()
            } else {
                theme::base()
            };

            ListItem::new(Line::from(Span::styled(display, style)))
        })
        .collect();

    let list = List::new(items);
    f.render_widget(list, area);
}

// ═══════════════════════════════════════════════════════════════════════════
// Rule providers list
// ═══════════════════════════════════════════════════════════════════════════

fn render_rule_providers(f: &mut Frame, app: &App, area: Rect) {
    let providers = &app.providers.rule_providers;

    let mut sorted: Vec<(&String, &RuleProviderItem)> = providers.iter().collect();
    sorted.sort_by(|a, b| a.1.name.cmp(&b.1.name));

    if sorted.is_empty() {
        let msg = Paragraph::new("No rule providers found").style(theme::dimmed());
        f.render_widget(msg, area);
        return;
    }

    let items: Vec<ListItem> = sorted
        .iter()
        .enumerate()
        .map(|(i, (_key, prov))| {
            let label = format!(
                " {} {:20} behavior={:<8} count={:<5} type={:<10} updated={}",
                if i == app.providers.selected {
                    "▸"
                } else {
                    " "
                },
                prov.name,
                prov.behavior,
                prov.rule_count,
                prov.vehicle_type,
                prov.updated,
            );
            let display = pad_or_truncate(&label, area.width as usize);

            let style = if i == app.providers.selected {
                theme::selected()
            } else {
                theme::base()
            };

            ListItem::new(Line::from(Span::styled(display, style)))
        })
        .collect();

    let list = List::new(items);
    f.render_widget(list, area);
}
