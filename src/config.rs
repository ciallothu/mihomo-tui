use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};

#[derive(Debug, Clone)]
pub struct AppPaths {
    pub root: PathBuf,
    pub configs: PathBuf,
    pub cores: PathBuf,
    pub subscriptions: PathBuf,
}

#[derive(Debug, Clone)]
pub struct ConfigManager {
    paths: AppPaths,
}

impl ConfigManager {
    pub fn new(root_override: Option<PathBuf>) -> Result<Self> {
        let root = root_override.unwrap_or_else(default_root);
        let paths = AppPaths {
            configs: root.join("configs"),
            cores: root.join("cores"),
            subscriptions: root.join("subscriptions"),
            root,
        };

        fs::create_dir_all(&paths.configs).context("create config directory")?;
        fs::create_dir_all(&paths.cores).context("create core directory")?;
        fs::create_dir_all(&paths.subscriptions).context("create subscription directory")?;

        Ok(Self { paths })
    }

    pub fn paths(&self) -> &AppPaths {
        &self.paths
    }

    pub fn import_file(&self, source: &Path) -> Result<PathBuf> {
        if !source.exists() {
            bail!("config file does not exist: {}", source.display());
        }

        let raw = fs::read_to_string(source)
            .with_context(|| format!("read config {}", source.display()))?;
        validate_yaml(&raw)?;

        let file_name = source
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("profile.yaml");
        let target = self.paths.configs.join(file_name);
        fs::write(&target, raw).with_context(|| format!("write config {}", target.display()))?;
        Ok(target)
    }

    pub async fn pull_subscription(&self, url: &str) -> Result<PathBuf> {
        let raw = reqwest::get(url)
            .await
            .with_context(|| format!("request subscription {url}"))?
            .error_for_status()
            .with_context(|| format!("subscription returned an error {url}"))?
            .text()
            .await
            .context("read subscription body")?;

        validate_yaml(&raw)?;

        let target = self.paths.subscriptions.join("default.yaml");
        fs::write(&target, raw)
            .with_context(|| format!("write subscription {}", target.display()))?;
        Ok(target)
    }
}

fn validate_yaml(raw: &str) -> Result<()> {
    serde_yaml::from_str::<serde_yaml::Value>(raw).context("parse mihomo YAML config")?;
    Ok(())
}

fn default_root() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
        .join("mihomo-tui")
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::ConfigManager;

    #[test]
    fn import_file_validates_and_copies_yaml() {
        let root = unique_temp_dir();
        let source = root.join("profile.yaml");
        fs::create_dir_all(&root).unwrap();
        fs::write(&source, "mixed-port: 7890\nmode: rule\n").unwrap();

        let manager = ConfigManager::new(Some(root.clone())).unwrap();
        let target = manager.import_file(&source).unwrap();

        assert!(target.exists());
        assert_eq!(target, root.join("configs").join("profile.yaml"));
    }

    #[test]
    fn import_file_rejects_invalid_yaml() {
        let root = unique_temp_dir();
        let source = root.join("broken.yaml");
        fs::create_dir_all(&root).unwrap();
        fs::write(&source, "mixed-port: [").unwrap();

        let manager = ConfigManager::new(Some(root)).unwrap();
        assert!(manager.import_file(&source).is_err());
    }

    fn unique_temp_dir() -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("mihomo-tui-test-{nanos}"))
    }
}
