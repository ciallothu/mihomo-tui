//! Kernel management – download, list, and switch mihomo kernel binaries.
//!
//! Kernels are stored under `<app_data_dir>/kernels/<version>/mihomo{-os}-{arch}[.exe]`.
//! The module detects the current platform and resolves the correct asset name
//! from GitHub releases published by `MetaCubeX/mihomo`.

use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use flate2::read::GzDecoder;
use serde::{Deserialize, Serialize};

// ── Platform detection ─────────────────────────────────────────────────────

/// Identified platform details used to select the correct kernel asset.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Platform {
    pub os: String,
    pub arch: String,
}

impl Platform {
    /// Detect the current OS and architecture.
    pub fn current() -> Self {
        let os = Self::detect_os();
        let arch = Self::detect_arch();
        Self { os, arch }
    }

    fn detect_os() -> String {
        if cfg!(target_os = "linux") {
            "linux".to_owned()
        } else if cfg!(target_os = "macos") {
            "darwin".to_owned()
        } else if cfg!(target_os = "windows") {
            "windows".to_owned()
        } else {
            std::env::consts::OS.to_owned()
        }
    }

    fn detect_arch() -> String {
        if cfg!(target_arch = "x86_64") {
            "amd64".to_owned()
        } else if cfg!(target_arch = "aarch64") {
            "arm64".to_owned()
        } else if cfg!(target_arch = "arm") {
            "armv7".to_owned()
        } else {
            std::env::consts::ARCH.to_owned()
        }
    }

    /// Binary filename for this platform (e.g. `mihomo-linux-amd64`, `mihomo-windows-amd64.exe`).
    pub fn binary_name(&self) -> String {
        if self.os == "windows" {
            format!("mihomo-{}-{}.exe", self.os, self.arch)
        } else {
            format!("mihomo-{}-{}", self.os, self.arch)
        }
    }

    /// Return the asset-name glob pattern used to find the correct download
    /// file from a GitHub release.
    ///
    /// For linux/darwin the assets are `.gz` files; for windows they are `.zip`.
    pub fn asset_pattern(&self) -> String {
        let ext = if self.os == "windows" { "zip" } else { "gz" };
        format!("mihomo-{}-{}*.{}", self.os, self.arch, ext)
    }
}

// ── GitHub release types ───────────────────────────────────────────────────

/// A simplified GitHub release object (only the fields we need).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GithubRelease {
    pub tag_name: String,
    pub name: Option<String>,
    pub prerelease: bool,
    pub published_at: String,
    pub assets: Vec<GithubAsset>,
}

/// A single asset within a GitHub release.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GithubAsset {
    pub name: String,
    pub size: u64,
    pub browser_download_url: String,
}

// ── Kernel manager ─────────────────────────────────────────────────────────

/// Manages mihomo kernel binaries on the local filesystem.
pub struct KernelManager {
    /// Base directory where kernel versions are stored.
    kernels_dir: PathBuf,
    /// Detected current platform.
    platform: Platform,
    /// HTTP client for GitHub API / downloads.
    http: reqwest::Client,
}

impl KernelManager {
    /// Create a new manager. `app_data_dir` is the application data directory
    /// (typically from `dirs::data_dir()`).
    pub fn new(app_data_dir: &Path) -> Self {
        let kernels_dir = app_data_dir.join("kernels");
        let http = reqwest::Client::builder()
            .user_agent("mihomo-tui")
            .build()
            .expect("failed to build HTTP client");
        Self {
            kernels_dir,
            platform: Platform::current(),
            http,
        }
    }

    /// Return the detected platform.
    pub fn platform(&self) -> &Platform {
        &self.platform
    }

    /// Return the kernels directory path.
    pub fn kernels_dir(&self) -> &Path {
        &self.kernels_dir
    }

    // ── GitHub API ─────────────────────────────────────────────────────────

    /// List available mihomo releases from GitHub.
    pub async fn list_remote_versions(&self) -> Result<Vec<GithubRelease>> {
        let url = "https://api.github.com/repos/MetaCubeX/mihomo/releases?per_page=30";
        let resp = self
            .http
            .get(url)
            .send()
            .await
            .context("failed to fetch releases from GitHub")?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            bail!("GitHub API error (HTTP {status}): {body}");
        }
        let releases: Vec<GithubRelease> = resp
            .json()
            .await
            .context("failed to parse GitHub releases")?;
        Ok(releases)
    }

    /// Find the best matching asset for the current platform within a release.
    pub fn find_matching_asset<'a>(&self, release: &'a GithubRelease) -> Option<&'a GithubAsset> {
        let pattern = self.platform.asset_pattern();
        release.assets.iter().find(|a| {
            // Simple glob: check if the asset name starts with the prefix
            // before the wildcard.
            let prefix = pattern.split('*').next().unwrap_or("");
            a.name.starts_with(prefix) && (a.name.ends_with(".gz") || a.name.ends_with(".zip"))
        })
    }

    // ── Download & install ─────────────────────────────────────────────────

    /// Download and install a kernel version.
    ///
    /// Returns the path to the extracted binary.
    pub async fn download_version(&self, release: &GithubRelease) -> Result<PathBuf> {
        let asset = self
            .find_matching_asset(release)
            .context("no matching asset found for current platform")?;

        let version_dir = self.kernels_dir.join(&release.tag_name);
        fs::create_dir_all(&version_dir)
            .with_context(|| format!("cannot create {}", version_dir.display()))?;

        // Download the compressed file.
        let resp = self
            .http
            .get(&asset.browser_download_url)
            .send()
            .await
            .context("download failed")?;
        if !resp.status().is_success() {
            bail!(
                "download failed (HTTP {}): {}",
                resp.status(),
                asset.browser_download_url
            );
        }
        let compressed_bytes = resp.bytes().await.context("failed to read download body")?;

        let binary_name = self.platform.binary_name();
        let binary_path = version_dir.join(&binary_name);

        // Decompress based on extension.
        if asset.name.ends_with(".gz") {
            self.decompress_gz(&compressed_bytes, &binary_path)
                .with_context(|| "gz decompression failed")?;
        } else if asset.name.ends_with(".zip") {
            self.decompress_zip(&compressed_bytes, &binary_path)
                .with_context(|| "zip decompression failed")?;
        } else {
            bail!("unknown archive format: {}", asset.name);
        }

        // Make executable on unix.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&binary_path)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&binary_path, perms)?;
        }

        Ok(binary_path)
    }

    /// Decompress a gzip archive, extracting the first entry to `out`.
    fn decompress_gz(&self, data: &[u8], out: &Path) -> Result<()> {
        let mut decoder = GzDecoder::new(data);
        let mut buf = Vec::with_capacity(data.len() * 2);
        decoder
            .read_to_end(&mut buf)
            .context("gzip decode failed")?;
        let mut file = fs::File::create(out)?;
        file.write_all(&buf)?;
        Ok(())
    }

    /// Decompress a zip archive, extracting the first file matching the
    /// expected binary name (or the first entry if no match).
    fn decompress_zip(&self, data: &[u8], out: &Path) -> Result<()> {
        let reader = io::Cursor::new(data);
        let mut archive = zip::ZipArchive::new(reader).context("zip open failed")?;

        // Try to find an entry that looks like the mihomo binary.
        let target = self.platform.binary_name();
        let mut found_idx: Option<usize> = None;

        for i in 0..archive.len() {
            let entry = archive.by_index(i).ok();
            if let Some(e) = entry {
                let name = e.name().to_string();
                // Match by contained binary name or just pick the first file.
                if name.contains(&target.replace(".exe", ""))
                    || name.ends_with(".exe") && target.ends_with(".exe")
                {
                    found_idx = Some(i);
                    break;
                }
            }
        }

        let idx = found_idx.unwrap_or(0);
        let mut entry = archive.by_index(idx).context("zip entry missing")?;
        let mut buf = Vec::with_capacity(entry.size() as usize);
        entry.read_to_end(&mut buf)?;
        let mut file = fs::File::create(out)?;
        file.write_all(&buf)?;
        Ok(())
    }

    // ── Local kernel management ────────────────────────────────────────────

    /// List locally installed kernel versions.
    ///
    /// Returns a sorted list of version tags that have a valid binary.
    pub fn list_installed_versions(&self) -> Result<Vec<String>> {
        if !self.kernels_dir.exists() {
            return Ok(Vec::new());
        }
        let mut versions = Vec::new();
        let binary_name = self.platform.binary_name();
        for entry in fs::read_dir(&self.kernels_dir)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                let version_dir = entry.path();
                if version_dir.join(&binary_name).exists()
                    && let Some(tag) = entry.file_name().to_str()
                {
                    versions.push(tag.to_owned());
                }
            }
        }
        versions.sort();
        Ok(versions)
    }

    /// Get the binary path for a specific version.
    pub fn get_binary_path(&self, version: &str) -> Result<PathBuf> {
        let path = self
            .kernels_dir
            .join(version)
            .join(self.platform.binary_name());
        if path.exists() {
            Ok(path)
        } else {
            bail!(
                "kernel binary not found for version {} at {}",
                version,
                path.display()
            );
        }
    }

    /// Remove an installed kernel version.
    pub fn remove_version(&self, version: &str) -> Result<()> {
        let dir = self.kernels_dir.join(version);
        if dir.exists() {
            fs::remove_dir_all(&dir)
                .with_context(|| format!("failed to remove {}", dir.display()))?;
        }
        Ok(())
    }

    /// Get the path to the "active" kernel symlink / pointer.
    ///
    /// The active version is stored in a simple text file `active` inside the
    /// kernels directory.
    pub fn active_version_path(&self) -> PathBuf {
        self.kernels_dir.join("active")
    }

    /// Read the currently active kernel version, if any.
    pub fn get_active_version(&self) -> Option<String> {
        fs::read_to_string(self.active_version_path())
            .ok()
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty())
    }

    /// Set the active kernel version.
    pub fn set_active_version(&self, version: &str) -> Result<()> {
        // Verify it's installed.
        self.get_binary_path(version)?;
        fs::write(self.active_version_path(), version)
            .context("failed to write active version file")?;
        Ok(())
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platform_current() {
        let p = Platform::current();
        // Should always produce a non-empty os and arch.
        assert!(!p.os.is_empty());
        assert!(!p.arch.is_empty());
    }

    #[test]
    fn binary_name_format() {
        let linux_amd64 = Platform {
            os: "linux".into(),
            arch: "amd64".into(),
        };
        assert_eq!(linux_amd64.binary_name(), "mihomo-linux-amd64");

        let win = Platform {
            os: "windows".into(),
            arch: "amd64".into(),
        };
        assert_eq!(win.binary_name(), "mihomo-windows-amd64.exe");
    }

    #[test]
    fn asset_pattern_format() {
        let p = Platform {
            os: "linux".into(),
            arch: "amd64".into(),
        };
        assert!(p.asset_pattern().contains(".gz"));

        let pw = Platform {
            os: "windows".into(),
            arch: "amd64".into(),
        };
        assert!(pw.asset_pattern().contains(".zip"));
    }
}
