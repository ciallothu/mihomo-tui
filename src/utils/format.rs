//! Formatting utilities for mihomo-tui.
//!
//! Provides human-readable formatting for:
//! - Byte sizes (B, KB, MB, GB, TB)
//! - Durations (uptime in human-readable form)
//! - Timestamps (formatting Unix timestamps)
//! - String truncation (Unicode/CJK-aware width)

use chrono::{DateTime, Local, TimeZone, Utc};
use unicode_width::UnicodeWidthStr;

// ── Byte formatting ────────────────────────────────────────────────────────

/// Convert a byte count to a human-readable string.
///
/// Uses binary prefixes (1 KB = 1024 B).
///
/// ```
/// use mihomo_tui::utils::format::bytes_to_human;
/// assert_eq!(bytes_to_human(0), "0 B");
/// assert_eq!(bytes_to_human(1024), "1.00 KB");
/// assert_eq!(bytes_to_human(1536), "1.50 KB");
/// assert_eq!(bytes_to_human(1048576), "1.00 MB");
/// ```
pub fn bytes_to_human(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    const TB: u64 = GB * 1024;

    if bytes == 0 {
        return "0 B".to_owned();
    }

    if bytes >= TB {
        format!("{:.2} TB", bytes as f64 / TB as f64)
    } else if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{bytes} B")
    }
}

// ── Duration formatting ────────────────────────────────────────────────────

/// Format a duration (in seconds) into a human-readable uptime string.
///
/// ```
/// use mihomo_tui::utils::format::format_duration;
/// assert_eq!(format_duration(0), "0s");
/// assert_eq!(format_duration(65), "1m 5s");
/// assert_eq!(format_duration(3661), "1h 1m 1s");
/// assert_eq!(format_duration(86400 + 3600), "1d 1h");
/// ```
pub fn format_duration(secs: u64) -> String {
    if secs == 0 {
        return "0s".to_owned();
    }

    let days = secs / 86400;
    let hours = (secs % 86400) / 3600;
    let minutes = (secs % 3600) / 60;
    let seconds = secs % 60;

    let mut parts = Vec::new();

    if days > 0 {
        parts.push(format!("{days}d"));
    }
    if hours > 0 {
        parts.push(format!("{hours}h"));
    }
    if minutes > 0 {
        parts.push(format!("{minutes}m"));
    }
    if seconds > 0 && days == 0 {
        // Only show seconds if less than a day.
        parts.push(format!("{seconds}s"));
    }

    parts.join(" ")
}

// ── Timestamp formatting ───────────────────────────────────────────────────

/// Format a Unix timestamp (seconds) as a local datetime string.
///
/// Returns a string like "2024-01-15 14:30:00".
pub fn format_timestamp(secs: i64) -> String {
    match Local.timestamp_opt(secs, 0) {
        chrono::LocalResult::Single(dt) => dt.format("%Y-%m-%d %H:%M:%S").to_string(),
        _ => "invalid timestamp".to_owned(),
    }
}

/// Format a Unix timestamp (seconds) with timezone info.
pub fn format_timestamp_tz(secs: i64) -> String {
    match Local.timestamp_opt(secs, 0) {
        chrono::LocalResult::Single(dt) => dt.format("%Y-%m-%d %H:%M:%S %:z").to_string(),
        _ => "invalid timestamp".to_owned(),
    }
}

/// Format a `DateTime<Utc>` into a local datetime string.
pub fn format_datetime_utc(dt: &DateTime<Utc>) -> String {
    dt.with_timezone(&Local)
        .format("%Y-%m-%d %H:%M:%S")
        .to_string()
}

/// Format a `DateTime<Utc>` as a relative time string (e.g. "2 hours ago").
pub fn format_time_ago(dt: &DateTime<Utc>) -> String {
    let now = Utc::now();
    let diff = now.signed_duration_since(*dt);

    if diff.num_seconds() < 0 {
        return "just now".to_owned();
    }

    let mins = diff.num_minutes();
    let hours = diff.num_hours();
    let days = diff.num_days();

    if mins < 1 {
        "just now".to_owned()
    } else if mins < 60 {
        format!("{mins} minute{} ago", if mins == 1 { "" } else { "s" })
    } else if hours < 24 {
        format!("{hours} hour{} ago", if hours == 1 { "" } else { "s" })
    } else if days < 30 {
        format!("{days} day{} ago", if days == 1 { "" } else { "s" })
    } else {
        format_datetime_utc(dt)
    }
}

// ── String truncation (CJK-aware) ──────────────────────────────────────────

/// Truncate a string to fit within a given display width, respecting
/// Unicode and CJK character widths.
///
/// If the string's display width exceeds `max_width`, it is truncated and
/// an ellipsis (`…`) is appended. The resulting string will have a display
/// width ≤ `max_width`.
///
/// ```
/// use mihomo_tui::utils::format::truncate_str;
/// assert_eq!(truncate_str("hello world", 20), "hello world");
/// assert_eq!(truncate_str("hello world", 8), "hello…");
/// ```
pub fn truncate_str(s: &str, max_width: usize) -> String {
    if max_width == 0 {
        return String::new();
    }

    let total_width = UnicodeWidthStr::width(s);
    if total_width <= max_width {
        return s.to_owned();
    }

    // We need to leave room for the ellipsis (width = 1).
    let target_width = max_width.saturating_sub(1);
    let mut current_width = 0usize;
    let mut end = 0;

    for (i, ch) in s.char_indices() {
        let ch_width = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        if current_width + ch_width > target_width {
            break;
        }
        current_width += ch_width;
        end = i + ch.len_utf8();
    }

    format!("{}…", &s[..end])
}

/// Truncate from the beginning of the string (useful for file paths).
///
/// If the string is too wide, it is prefixed with `…`.
pub fn truncate_str_left(s: &str, max_width: usize) -> String {
    if max_width == 0 {
        return String::new();
    }

    let total_width = UnicodeWidthStr::width(s);
    if total_width <= max_width {
        return s.to_owned();
    }

    let target_width = max_width.saturating_sub(1);
    let mut current_width = 0usize;
    let chars: Vec<char> = s.chars().collect();

    // Walk backwards from the end.
    for (i, &ch) in chars.iter().enumerate().rev() {
        let ch_width = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        if current_width + ch_width > target_width {
            // Skip this char and everything before it.
            let start_idx = i + 1;
            let remaining: String = chars[start_idx..].iter().collect();
            return format!("…{remaining}");
        }
        current_width += ch_width;
    }

    s.to_owned()
}

/// Pad or truncate a string to exactly `max_width` display columns.
///
/// If shorter, pads with spaces on the right. If longer, truncates with `…`.
pub fn pad_or_truncate(s: &str, max_width: usize) -> String {
    let truncated = truncate_str(s, max_width);
    let width = UnicodeWidthStr::width(truncated.as_str());
    if width < max_width {
        format!("{}{}", truncated, " ".repeat(max_width - width))
    } else {
        truncated
    }
}

// ── Speed formatting ───────────────────────────────────────────────────────

/// Format bytes per second as a human-readable speed string.
///
/// ```
/// use mihomo_tui::utils::format::format_speed;
/// assert_eq!(format_speed(1024), "1.00 KB/s");
/// ```
pub fn format_speed(bytes_per_sec: u64) -> String {
    format!("{}/s", bytes_to_human(bytes_per_sec))
}

// ── Percentage formatting ──────────────────────────────────────────────────

/// Format a ratio (0.0 – 1.0) as a percentage string.
pub fn format_percent(ratio: f64) -> String {
    format!("{:.1}%", (ratio * 100.0).clamp(0.0, 100.0))
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bytes_to_human() {
        assert_eq!(bytes_to_human(0), "0 B");
        assert_eq!(bytes_to_human(512), "512 B");
        assert_eq!(bytes_to_human(1024), "1.00 KB");
        assert_eq!(bytes_to_human(1536), "1.50 KB");
        assert_eq!(bytes_to_human(1048576), "1.00 MB");
        assert_eq!(bytes_to_human(1073741824), "1.00 GB");
        assert_eq!(bytes_to_human(1099511627776), "1.00 TB");
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(0), "0s");
        assert_eq!(format_duration(1), "1s");
        assert_eq!(format_duration(59), "59s");
        assert_eq!(format_duration(60), "1m");
        assert_eq!(format_duration(65), "1m 5s");
        assert_eq!(format_duration(3600), "1h");
        assert_eq!(format_duration(3661), "1h 1m 1s");
        assert_eq!(format_duration(86400), "1d");
        assert_eq!(format_duration(90061), "1d 1h 1m");
    }

    #[test]
    fn test_truncate_str_ascii() {
        assert_eq!(truncate_str("hello", 10), "hello");
        assert_eq!(truncate_str("hello world", 8), "hello…");
        assert_eq!(truncate_str("hello", 5), "hello");
        assert_eq!(truncate_str("hello", 4), "hel…");
    }

    #[test]
    fn test_truncate_str_cjk() {
        // Each CJK character has width 2.
        let s = "你好世界";
        assert_eq!(truncate_str(s, 8), s);
        assert_eq!(truncate_str(s, 5), "你好…");
        assert_eq!(truncate_str(s, 3), "你…");
    }

    #[test]
    fn test_truncate_str_mixed() {
        let s = "hello你好";
        // h(1)e(1)l(1)l(1)o(1)你(2)好(2) = 8
        assert_eq!(truncate_str(s, 8), s);
        assert_eq!(truncate_str(s, 6), "hello…");
        assert_eq!(truncate_str(s, 5), "hello");
    }

    #[test]
    fn test_truncate_str_left() {
        assert_eq!(truncate_str_left("hello world", 20), "hello world");
        assert_eq!(truncate_str_left("hello world", 8), "…lo world");
    }

    #[test]
    fn test_pad_or_truncate() {
        let result = pad_or_truncate("hi", 5);
        assert_eq!(result, "hi   ");
        assert_eq!(result.len(), 5);

        let result = pad_or_truncate("hello world", 5);
        assert_eq!(result, "hell…");
    }

    #[test]
    fn test_format_speed() {
        assert_eq!(format_speed(0), "0 B/s");
        assert_eq!(format_speed(1024), "1.00 KB/s");
    }

    #[test]
    fn test_format_percent() {
        assert_eq!(format_percent(0.0), "0.0%");
        assert_eq!(format_percent(0.5), "50.0%");
        assert_eq!(format_percent(1.0), "100.0%");
        assert_eq!(format_percent(1.5), "100.0%"); // clamped
    }

    #[test]
    fn test_truncate_empty() {
        assert_eq!(truncate_str("", 5), "");
        assert_eq!(truncate_str("hello", 0), "");
    }

    #[test]
    fn test_bytes_to_human_large() {
        let tb: u64 = 2 * 1024 * 1024 * 1024 * 1024;
        assert_eq!(bytes_to_human(tb), "2.00 TB");
    }
}
