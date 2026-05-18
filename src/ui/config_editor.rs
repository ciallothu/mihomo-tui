//! Config editor / viewer panel.
//!
//! Shows current mihomo configuration: ports, mode, DNS, TUN, etc.
//! Allows switching mode and reloading config via key bindings.
//!
//! Key bindings:
//! - `M` – cycle clash mode (Rule → Global → Direct)
//! - `r` – reload config from disk
//! - `Enter` – toggle boolean settings

use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};

use super::theme;
use crate::app::App;

// ═══════════════════════════════════════════════════════════════════════════
// Public render entry point
// ═══════════════════════════════════════════════════════════════════════════

/// Render the config panel.
pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme::border())
        .title(Span::styled(" Config ", theme::title()));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let config = match &app.cfg.config {
        Some(c) => c,
        None => {
            let msg = Paragraph::new("Loading config…").style(theme::dimmed());
            f.render_widget(msg, inner);
            return;
        }
    };

    // 分两列显示配置信息
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(inner);

    render_left(f, app, config, cols[0]);
    render_right(f, config, cols[1]);
}

// ═══════════════════════════════════════════════════════════════════════════
// 左栏：端口、模式、基本设置
// ═══════════════════════════════════════════════════════════════════════════

fn render_left(f: &mut Frame, _app: &App, config: &crate::api::types::ConfigResponse, area: Rect) {
    let mode_str = format!("{:?}", config.mode);
    let mode_style = theme::mode_style(&config.mode);

    let port_str = if config.mixed_port > 0 {
        config.mixed_port.to_string()
    } else {
        config.port.to_string()
    };

    let lines = vec![
        Line::from(vec![
            Span::styled("  Mode:          ", theme::accent()),
            Span::styled(mode_str, mode_style),
        ]),
        Line::from(vec![
            Span::styled("  Mixed Port:    ", theme::accent()),
            Span::styled(port_str, theme::fg()),
        ]),
        Line::from(vec![
            Span::styled("  SOCKS Port:    ", theme::accent()),
            Span::styled(config.socks_port.to_string(), theme::fg()),
        ]),
        Line::from(vec![
            Span::styled("  Redir Port:    ", theme::accent()),
            Span::styled(config.redir_port.to_string(), theme::fg()),
        ]),
        Line::from(vec![
            Span::styled("  TProxy Port:   ", theme::accent()),
            Span::styled(config.tproxy_port.to_string(), theme::fg()),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Allow LAN:     ", theme::accent()),
            Span::styled(
                if config.allow_lan { "Yes" } else { "No" },
                if config.allow_lan {
                    theme::success()
                } else {
                    theme::dimmed()
                },
            ),
        ]),
        Line::from(vec![
            Span::styled("  IPv6:          ", theme::accent()),
            Span::styled(if config.ipv6 { "Yes" } else { "No" }, theme::fg()),
        ]),
        Line::from(vec![
            Span::styled("  Unified Delay: ", theme::accent()),
            Span::styled(if config.unified_delay { "Yes" } else { "No" }, theme::fg()),
        ]),
        Line::from(vec![
            Span::styled("  Log Level:     ", theme::accent()),
            Span::styled(&config.log_level, theme::fg()),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  [M] ", theme::warning()),
            Span::styled("Cycle mode  ", theme::fg()),
            Span::styled("[r] ", theme::warning()),
            Span::styled("Reload config", theme::fg()),
        ]),
    ];

    let paragraph = Paragraph::new(lines).style(theme::base());
    f.render_widget(paragraph, area);
}

// ═══════════════════════════════════════════════════════════════════════════
// 右栏：DNS、TUN、Sniffer
// ═══════════════════════════════════════════════════════════════════════════

fn render_right(f: &mut Frame, config: &crate::api::types::ConfigResponse, area: Rect) {
    // TUN 配置
    let tun_status = if config.tun.enable {
        "Active"
    } else {
        "Inactive"
    };
    let tun_style = if config.tun.enable {
        theme::success()
    } else {
        theme::dimmed()
    };

    let tun_stack = config.tun.stack.as_deref().unwrap_or("—");
    let tun_device = config.tun.device.as_deref().unwrap_or("—");

    let mut lines = vec![
        Line::from(vec![Span::styled("  ── TUN ──", theme::title())]),
        Line::from(vec![
            Span::styled("  Status:        ", theme::accent()),
            Span::styled(tun_status, tun_style),
        ]),
        Line::from(vec![
            Span::styled("  Stack:         ", theme::accent()),
            Span::styled(tun_stack, theme::fg()),
        ]),
        Line::from(vec![
            Span::styled("  Device:        ", theme::accent()),
            Span::styled(tun_device, theme::fg()),
        ]),
        Line::from(vec![
            Span::styled("  Auto Route:    ", theme::accent()),
            Span::styled(
                if config.tun.auto_route { "Yes" } else { "No" },
                theme::fg(),
            ),
        ]),
        Line::from(""),
    ];

    // DNS 配置
    if let Some(ref dns) = config.dns {
        lines.push(Line::from(vec![Span::styled(
            "  ── DNS ──",
            theme::title(),
        )]));
        lines.push(Line::from(vec![
            Span::styled("  Enable:        ", theme::accent()),
            Span::styled(if dns.enable { "Yes" } else { "No" }, theme::fg()),
        ]));
        lines.push(Line::from(vec![
            Span::styled("  Listen:        ", theme::accent()),
            Span::styled(&dns.listen, theme::fg()),
        ]));
        lines.push(Line::from(vec![
            Span::styled("  Enhanced Mode: ", theme::accent()),
            Span::styled(&dns.enhanced_mode, theme::mauve()),
        ]));
        lines.push(Line::from(vec![
            Span::styled("  Nameservers:   ", theme::accent()),
            Span::styled(
                dns.nameserver.first().map(|s| s.as_str()).unwrap_or("—"),
                theme::fg(),
            ),
        ]));
        if dns.nameserver.len() > 1 {
            for ns in &dns.nameserver[1..] {
                lines.push(Line::from(vec![
                    Span::styled("                 ", theme::accent()),
                    Span::styled(ns.as_str(), theme::fg()),
                ]));
            }
        }
    } else {
        lines.push(Line::from(vec![
            Span::styled("  DNS: ", theme::accent()),
            Span::styled("Not configured", theme::dimmed()),
        ]));
    }

    // 外部控制器
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("  Controller:    ", theme::accent()),
        Span::styled(&config.external_controller, theme::fg()),
    ]));

    let paragraph = Paragraph::new(lines).style(theme::base());
    f.render_widget(paragraph, area);
}
