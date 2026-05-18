//! Command-line argument definitions.
//!
//! Uses [`clap`] derive macros to declare every flag and subcommand the TUI
//! understands. The parsed result ([`CliArgs`]) is consumed early in `main`
//! and folded into the application configuration.

use clap::Parser;

// ── Top-level CLI ───────────────────────────────────────────────────────────

/// **mihomo-tui** – A cross-platform terminal UI for the mihomo (Clash.Meta)
/// proxy kernel.
#[derive(Debug, Parser)]
#[command(
    name = "mihomo-tui",
    version,
    about = "Terminal UI for mihomo (Clash.Meta) proxy management",
    long_about = "An interactive terminal interface for managing mihomo proxy.\n\
                  Connect to a running mihomo instance via its RESTful API\n\
                  and control proxies, rules, connections, and more."
)]
pub struct CliArgs {
    // ── API connection ──────────────────────────────────────────────────────
    /// mihomo external-controller address (e.g. `127.0.0.1:9090`).
    #[arg(
        short,
        long = "api-addr",
        env = "MIHOMO_API_ADDR",
        default_value = "127.0.0.1:9090",
        value_name = "HOST:PORT"
    )]
    pub api_addr: String,

    /// mihomo API secret (`secret` field in config.yaml).
    #[arg(
        short,
        long = "secret",
        env = "MIHOMO_SECRET",
        default_value = "",
        value_name = "SECRET"
    )]
    pub secret: String,

    /// Use HTTPS instead of HTTP for the API connection.
    #[arg(long = "use-https", default_value_t = false)]
    pub use_https: bool,

    // ── Display ─────────────────────────────────────────────────────────────
    /// Use a light color theme.
    #[arg(long = "light-theme", default_value_t = false)]
    pub light_theme: bool,

    /// Request a specific tick rate in milliseconds (controls UI refresh).
    #[arg(long = "tick-rate", default_value_t = 100, value_name = "MS")]
    pub tick_rate: u64,

    // ── Paths ───────────────────────────────────────────────────────────────
    /// Path to an extra mihomo config directory (used for locating profiles).
    #[arg(long = "config-dir", value_name = "DIR")]
    pub config_dir: Option<String>,

    // ── Logging ─────────────────────────────────────────────────────────────
    /// Log level: off, error, warn, info, debug, trace.
    #[arg(long = "log-level", default_value = "warn", value_name = "LEVEL")]
    pub log_level: String,

    /// Write logs to a file instead of stderr.
    #[arg(long = "log-file", value_name = "PATH")]
    pub log_file: Option<String>,

    // ── Subcommands ─────────────────────────────────────────────────────────
    #[command(subcommand)]
    pub command: Option<CliCommand>,
}

// ── Subcommands ─────────────────────────────────────────────────────────────

/// Optional subcommand (when omitted the full TUI is launched).
#[derive(Debug, clap::Subcommand)]
pub enum CliCommand {
    /// Print mihomo version info and exit.
    Version,

    /// Run a quick connectivity check against the API.
    Check,

    /// Dump the parsed configuration and exit (useful for debugging).
    DumpConfig,
}

// ── Unit tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn verify_cli() {
        // Ensure the CLI definition parses without errors.
        use clap::CommandFactory;
        CliArgs::command().debug_assert();
    }

    #[test]
    fn default_values() {
        let args = CliArgs::try_parse_from(["mihomo-tui"]).unwrap();
        assert_eq!(args.api_addr, "127.0.0.1:9090");
        assert_eq!(args.secret, "");
        assert!(!args.use_https);
        assert!(!args.light_theme);
        assert_eq!(args.tick_rate, 100);
        assert!(args.command.is_none());
    }
}
