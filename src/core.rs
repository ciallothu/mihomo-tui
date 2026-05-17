use std::{
    fmt,
    fs::{self, File},
    io::Cursor,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
};

use anyhow::{Context, Result, bail};
use flate2::read::GzDecoder;
use reqwest::Client;
use serde::{Deserialize, Deserializer};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MihomoMode {
    Rule,
    Global,
    Direct,
}

impl MihomoMode {
    pub fn next(self) -> Self {
        match self {
            Self::Rule => Self::Global,
            Self::Global => Self::Direct,
            Self::Direct => Self::Rule,
        }
    }

    pub fn api_value(self) -> &'static str {
        match self {
            Self::Rule => "rule",
            Self::Global => "global",
            Self::Direct => "direct",
        }
    }
}

impl fmt::Display for MihomoMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rule => write!(f, "Rule"),
            Self::Global => write!(f, "Global"),
            Self::Direct => write!(f, "Direct"),
        }
    }
}

impl<'de> Deserialize<'de> for MihomoMode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        match value.to_ascii_lowercase().as_str() {
            "rule" => Ok(Self::Rule),
            "global" => Ok(Self::Global),
            "direct" => Ok(Self::Direct),
            other => Err(serde::de::Error::custom(format!(
                "unsupported mihomo mode {other}"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoreStatus {
    Stopped,
    Running,
    Missing,
}

impl fmt::Display for CoreStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Stopped => write!(f, "Stopped"),
            Self::Running => write!(f, "Running"),
            Self::Missing => write!(f, "Missing"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct MihomoCore {
    pub binary_path: Option<PathBuf>,
    pub version: String,
    pub status: CoreStatus,
}

impl MihomoCore {
    pub fn new(binary_path: Option<PathBuf>) -> Self {
        let version = detect_version(binary_path.as_ref()).unwrap_or_else(|| "unknown".to_string());
        let status = if binary_path.as_ref().is_some_and(|path| path.exists()) {
            CoreStatus::Stopped
        } else {
            CoreStatus::Missing
        };

        Self {
            binary_path,
            version,
            status,
        }
    }
}

pub fn find_default_core(cores_dir: &Path) -> Option<PathBuf> {
    let binary = binary_name();
    let mut candidates = Vec::new();

    if let Ok(current_exe) = std::env::current_exe()
        && let Some(exe_dir) = current_exe.parent()
    {
        candidates.push(exe_dir.join(binary));
        candidates.push(exe_dir.join("core").join(binary));
    }

    candidates.push(cores_dir.join(binary));
    candidates.extend(versioned_core_candidates(cores_dir, binary));
    candidates.into_iter().find(|path| path.is_file())
}

pub fn start_core(binary_path: &Path, config_path: &Path) -> Result<Child> {
    if !binary_path.exists() {
        bail!("mihomo binary does not exist: {}", binary_path.display());
    }
    if !config_path.exists() {
        bail!("mihomo config does not exist: {}", config_path.display());
    }

    Command::new(binary_path)
        .arg("-f")
        .arg(config_path)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .with_context(|| format!("start mihomo core {}", binary_path.display()))
}

pub async fn install_core(version: &str, cores_dir: &Path) -> Result<PathBuf> {
    fs::create_dir_all(cores_dir).context("create mihomo core directory")?;
    let client = Client::new();
    let release = fetch_release(&client, version).await?;
    let asset = choose_asset(&release.assets).with_context(|| {
        format!(
            "no mihomo release asset for this platform in {}",
            release.tag_name
        )
    })?;

    let bytes = client
        .get(&asset.browser_download_url)
        .header("User-Agent", "mihomo-tui")
        .send()
        .await
        .context("download mihomo release asset")?
        .error_for_status()
        .context("mihomo asset download failed")?
        .bytes()
        .await
        .context("read mihomo asset")?;

    let binary_name = binary_name();
    let target = cores_dir.join(format!("{}-{}", release.tag_name, binary_name));

    if asset.name.ends_with(".gz") {
        let mut decoder = GzDecoder::new(Cursor::new(bytes));
        let mut file =
            File::create(&target).with_context(|| format!("write {}", target.display()))?;
        std::io::copy(&mut decoder, &mut file).context("decompress mihomo gzip asset")?;
    } else if asset.name.ends_with(".zip") {
        let cursor = Cursor::new(bytes);
        let mut archive = zip::ZipArchive::new(cursor).context("open mihomo zip asset")?;
        let mut extracted = false;
        for index in 0..archive.len() {
            let mut entry = archive.by_index(index).context("read zip entry")?;
            let name = entry.name().to_ascii_lowercase();
            if name.ends_with("mihomo.exe") || name.ends_with("mihomo") {
                let mut file =
                    File::create(&target).with_context(|| format!("write {}", target.display()))?;
                std::io::copy(&mut entry, &mut file).context("extract mihomo zip asset")?;
                extracted = true;
                break;
            }
        }
        if !extracted {
            bail!("mihomo executable was not found in {}", asset.name);
        }
    } else {
        fs::write(&target, bytes).with_context(|| format!("write {}", target.display()))?;
    }

    make_executable(&target)?;
    let default_target = cores_dir.join(binary_name);
    fs::copy(&target, &default_target)
        .with_context(|| format!("update default core {}", default_target.display()))?;
    make_executable(&default_target)?;
    Ok(default_target)
}

async fn fetch_release(client: &Client, version: &str) -> Result<Release> {
    let url = if version == "latest" {
        "https://api.github.com/repos/MetaCubeX/mihomo/releases/latest".to_string()
    } else {
        let tag = if version.starts_with('v') {
            version.to_string()
        } else {
            format!("v{version}")
        };
        format!("https://api.github.com/repos/MetaCubeX/mihomo/releases/tags/{tag}")
    };

    client
        .get(url)
        .header("User-Agent", "mihomo-tui")
        .send()
        .await
        .context("request mihomo release metadata")?
        .error_for_status()
        .context("mihomo release metadata request failed")?
        .json::<Release>()
        .await
        .context("decode mihomo release metadata")
}

#[derive(Debug, Deserialize)]
struct Release {
    tag_name: String,
    assets: Vec<ReleaseAsset>,
}

#[derive(Debug, Deserialize)]
struct ReleaseAsset {
    name: String,
    browser_download_url: String,
}

fn choose_asset(assets: &[ReleaseAsset]) -> Option<&ReleaseAsset> {
    let os = if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "darwin"
    } else {
        "linux"
    };
    let arch = match std::env::consts::ARCH {
        "x86_64" => "amd64",
        "aarch64" => "arm64",
        "arm" => "arm",
        "x86" => "386",
        other => other,
    };
    let extension = if cfg!(windows) { ".zip" } else { ".gz" };

    assets
        .iter()
        .filter(|asset| {
            let name = asset.name.to_ascii_lowercase();
            name.contains(os)
                && name.contains(arch)
                && name.ends_with(extension)
                && !name.contains("android")
        })
        .max_by_key(|asset| asset_score(&asset.name))
}

fn asset_score(name: &str) -> i32 {
    let lower = name.to_ascii_lowercase();
    let mut score = 0;
    if lower.contains("compatible") {
        score += 30;
    }
    if lower.contains("-v1-") {
        score += 20;
    }
    if lower.contains("go124") {
        score += 3;
    } else if lower.contains("go122") {
        score += 2;
    } else if lower.contains("go120") {
        score += 1;
    }
    score
}

fn binary_name() -> &'static str {
    if cfg!(windows) {
        "mihomo.exe"
    } else {
        "mihomo"
    }
}

fn versioned_core_candidates(cores_dir: &Path, binary: &str) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(cores_dir) else {
        return Vec::new();
    };
    let mut paths = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with(binary))
        })
        .collect::<Vec<_>>();
    paths.sort();
    paths.reverse();
    paths
}

#[cfg(unix)]
fn make_executable(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = fs::metadata(path)?.permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions)?;
    Ok(())
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) -> Result<()> {
    Ok(())
}

fn detect_version(binary_path: Option<&PathBuf>) -> Option<String> {
    let candidate = binary_path?;
    let output = Command::new(candidate).arg("-v").output().ok()?;
    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let text = if stdout.trim().is_empty() {
        stderr.trim()
    } else {
        stdout.trim()
    };

    if text.is_empty() {
        None
    } else {
        Some(text.lines().next().unwrap_or(text).to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::MihomoMode;

    #[test]
    fn mode_cycles_through_mihomo_modes() {
        assert_eq!(MihomoMode::Rule.next(), MihomoMode::Global);
        assert_eq!(MihomoMode::Global.next(), MihomoMode::Direct);
        assert_eq!(MihomoMode::Direct.next(), MihomoMode::Rule);
    }
}
