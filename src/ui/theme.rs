//! Colour theme constants and ratatui style helpers.
//!
//! Uses a dark theme optimised for high contrast. Every colour is a named
//! constant so switching to an alternate palette later only requires editing
//! this single module.

use ratatui::style::{Color, Modifier, Style};

// ═══════════════════════════════════════════════════════════════════════════
// Colour palette – dark theme
// ═══════════════════════════════════════════════════════════════════════════

/// Base background colour.
pub const BG: Color = Color::Rgb(22, 22, 30);
/// Primary foreground / text colour.
pub const FG: Color = Color::Rgb(205, 214, 244);
/// Accent colour used for highlights and active items.
pub const ACCENT: Color = Color::Rgb(137, 180, 250);
/// Success / green.
pub const GREEN: Color = Color::Rgb(166, 227, 161);
/// Warning / yellow.
pub const YELLOW: Color = Color::Rgb(249, 226, 175);
/// Error / red.
pub const RED: Color = Color::Rgb(243, 139, 168);
/// Dimmed / secondary text.
pub const DIM: Color = Color::Rgb(108, 112, 134);
/// Mauve / purple for special labels.
pub const MAUVE: Color = Color::Rgb(203, 166, 247);
/// Surface / slightly lighter than BG for panel backgrounds.
pub const SURFACE: Color = Color::Rgb(30, 30, 44);
/// Overlay / border colour.
pub const BORDER: Color = Color::Rgb(69, 71, 90);
/// Cyan for informational elements.
pub const CYAN: Color = Color::Rgb(148, 226, 213);
/// Peach / orange.
pub const PEACH: Color = Color::Rgb(250, 179, 135);

// ═══════════════════════════════════════════════════════════════════════════
// Reusable style constructors
// ═══════════════════════════════════════════════════════════════════════════

/// Base style for the whole application background.
pub fn base() -> Style {
    Style::default().fg(FG).bg(BG)
}

/// Panel border.
pub fn border() -> Style {
    Style::default().fg(BORDER)
}

/// Title text (bold, accent colour).
pub fn title() -> Style {
    Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
}

/// Header row in a table.
pub fn header() -> Style {
    Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
}

/// Selected / highlighted item.
pub fn selected() -> Style {
    Style::default()
        .fg(BG)
        .bg(ACCENT)
        .add_modifier(Modifier::BOLD)
}

/// Dimmed / secondary text.
pub fn dimmed() -> Style {
    Style::default().fg(DIM)
}

/// Success text (green).
pub fn success() -> Style {
    Style::default().fg(GREEN)
}

/// Warning text (yellow).
pub fn warning() -> Style {
    Style::default().fg(YELLOW)
}

/// Error text (red).
pub fn error() -> Style {
    Style::default().fg(RED)
}

/// Accent text without background.
pub fn accent() -> Style {
    Style::default().fg(ACCENT)
}

/// Mauve / special label.
pub fn mauve() -> Style {
    Style::default().fg(MAUVE)
}

/// Cyan / informational.
pub fn cyan() -> Style {
    Style::default().fg(CYAN)
}

/// Peach / orange highlight.
pub fn peach() -> Style {
    Style::default().fg(PEACH)
}

/// Active tab style.
pub fn tab_active() -> Style {
    Style::default()
        .fg(BG)
        .bg(ACCENT)
        .add_modifier(Modifier::BOLD)
}

/// Inactive tab style.
pub fn tab_inactive() -> Style {
    Style::default().fg(DIM)
}

/// Style for the bottom status bar.
pub fn status_bar() -> Style {
    Style::default().fg(FG).bg(SURFACE)
}

/// Style for the status bar mode indicator.
pub fn status_mode() -> Style {
    Style::default()
        .fg(BG)
        .bg(ACCENT)
        .add_modifier(Modifier::BOLD)
}

/// Style for search / input bar.
pub fn search_bar() -> Style {
    Style::default().fg(FG).bg(SURFACE)
}

/// Latency colour based on delay value (ms).
pub fn latency_style(delay: u64) -> Style {
    match delay {
        0 => Style::default().fg(DIM),
        1..=199 => Style::default().fg(GREEN),
        200..=499 => Style::default().fg(YELLOW),
        _ => Style::default().fg(RED),
    }
}

/// Foreground-only style (no background override).
pub fn fg() -> Style {
    Style::default().fg(FG)
}

/// Mode colour for clash mode indicator.
pub fn mode_style(mode: &crate::api::types::ClashMode) -> Style {
    match mode {
        crate::api::types::ClashMode::Rule => Style::default().fg(GREEN),
        crate::api::types::ClashMode::Global => Style::default().fg(YELLOW),
        crate::api::types::ClashMode::Direct => Style::default().fg(ACCENT),
    }
}

/// Log level colour mapping.
pub fn log_level_style(level: &str) -> Style {
    match level.to_lowercase().as_str() {
        "debug" => Style::default().fg(DIM),
        "info" => Style::default().fg(GREEN),
        "warning" => Style::default().fg(YELLOW),
        "error" => Style::default().fg(RED),
        _ => Style::default().fg(FG),
    }
}
