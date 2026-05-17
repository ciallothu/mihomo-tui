# mihomo-tui

`mihomo-tui` is a cross-platform terminal workspace for managing a local
mihomo runtime. Release packages bundle the latest matching mihomo core for the
target platform next to the TUI binary. The TUI talks to mihomo's
`external-controller` API, so proxy
groups, mode switching, provider refreshes, listener ports, connections,
runtime metrics, and logs are real operations against a running mihomo core.

## Project shape

- `src/tui.rs`: ratatui/crossterm terminal UI and keyboard handling.
- `src/app.rs`: application state, tab navigation, and TUI actions.
- `src/api.rs`: mihomo external-controller REST and log WebSocket client.
- `src/core.rs`: mihomo core metadata, mode model, and version detection.
- `src/config.rs`: local config import and remote subscription pull.
- `src/panel.rs`: zashboard-like panel models used by the TUI.
- `scripts/download_mihomo_core.py`: CI helper that bundles the latest mihomo
  release asset for each target platform.
- `.github/workflows`: CI, review, build, PR pre-release, and stable release.

## Current controls

- `q` / `Esc`: quit.
- `Tab`, `h`, `l`, arrows: switch tabs.
- `j`, `k`: move selection.
- `Enter` / `Space`: apply the current selection.
- `m`: cycle `Rule -> Global -> Direct` through `/configs`.
- `r`: refresh live data from mihomo.
- `x`: close all active connections through `/connections`.
- `+` / `-`: adjust the selected listener port on the Ports tab.

On the Proxies tab, `Enter` switches the selected proxy group to the next
available node through `/proxies/{group}`. On the Providers tab, `Enter`
refreshes the selected proxy or rule provider. On the Connections tab, `Enter`
closes the selected connection.

## Run locally

```bash
cargo run
```

Connect to a non-default controller:

```bash
cargo run -- --controller http://127.0.0.1:9090 --secret "$MIHOMO_SECRET"
```

Import a local mihomo config before opening the TUI:

```bash
cargo run -- --config ./config.yaml
```

Pull a remote subscription before opening the TUI:

```bash
cargo run -- --subscribe "https://example.com/sub.yaml"
```

Use a specific mihomo core binary:

```bash
cargo run -- --core /path/to/mihomo
```

When no `--core` is supplied, `mihomo-tui` looks for a bundled `mihomo` or
`mihomo.exe` next to the current executable first, then falls back to the app
data directory. Release archives are packaged in that layout by default.

Install a mihomo core release into the app data directory:

```bash
cargo run -- --install-core latest
cargo run -- --install-core v1.19.24
```

Install or select a core, import a config, start mihomo, then open the TUI:

```bash
cargo run -- --install-core latest --config ./config.yaml --start-core
```

If `--start-core` is used without `--core` and no bundled/default core is found,
the app installs the latest mihomo release into the data directory before
starting it.

## CI and releases

All distributable builds are intended to be produced by GitHub Actions:

- `CI`: formatting, clippy review, and tests.
- `Review`: dependency audit with `cargo audit`.
- `Build`: release binaries for Linux, Windows, and macOS.
- `PR Pre-release`: builds PR previews and publishes a rolling pre-release.
- `Release`: publishes stable releases for tags matching `v*`.

Build, PR pre-release, and release artifacts include:

- `mihomo-tui` or `mihomo-tui.exe`
- latest platform-matched `mihomo` or `mihomo.exe`
- `mihomo-core-version.txt`
- `README.md`

For PR previews, the workflow uses `pr-<number>` as the short release tag. On
each PR update, it deletes the previous pre-release and matching tag first, then
pushes a fresh tag at the PR head commit and creates a new GitHub pre-release.

Preview publishing is enabled only for PR branches in the same repository,
because GitHub does not grant write tokens to untrusted fork PRs.

## Roadmap

- Add config editing and subscription refresh intervals inside the TUI.
- Add provider enable/disable by editing config YAML and restarting/reloading
  mihomo, because mihomo's runtime API refreshes providers but does not expose
  a generic provider disable switch.
- Add integration tests around API adapters once the backend layer is wired.
