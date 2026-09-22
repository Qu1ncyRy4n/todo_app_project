use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};
use uuid::Uuid;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppPaths {
    pub config_dir: PathBuf,
    pub data_dir: PathBuf,
}

impl AppPaths {
    pub fn discover() -> Result<Self> {
        let config_dir = match std::env::var_os("TODO_CONFIG_HOME") {
            Some(path) => PathBuf::from(path),
            None => dirs::config_dir()
                .context("cannot determine configuration directory")?
                .join("todo-ledger"),
        };
        let data_dir = match std::env::var_os("TODO_DATA_HOME") {
            Some(path) => PathBuf::from(path),
            None => dirs::data_dir()
                .context("cannot determine data directory")?
                .join("todo-ledger"),
        };
        Ok(Self {
            config_dir,
            data_dir,
        })
    }

    pub fn database(&self) -> PathBuf {
        self.data_dir.join("ledger.db")
    }

    pub fn config(&self) -> PathBuf {
        self.config_dir.join("config.toml")
    }

    pub fn create_dirs(&self) -> Result<()> {
        fs::create_dir_all(&self.config_dir)
            .with_context(|| format!("create {}", self.config_dir.display()))?;
        fs::create_dir_all(&self.data_dir)
            .with_context(|| format!("create {}", self.data_dir.display()))?;
        Ok(())
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub actor_id: Option<String>,
    #[serde(default)]
    pub sync_repo: Option<PathBuf>,
    pub default_project: Option<String>,
    #[serde(default)]
    pub project_paths: BTreeMap<PathBuf, String>,
}

impl AppConfig {
    pub fn ensure_actor_id(&mut self) -> &str {
        if self.actor_id.is_none() {
            self.actor_id = Some(Uuid::now_v7().to_string());
        }
        self.actor_id.as_deref().expect("actor ID was initialized")
    }

    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let contents =
            fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
        toml::from_str(&contents).with_context(|| format!("parse {}", path.display()))
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let parent = path.parent().context("configuration path has no parent")?;
        fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
        let encoded = toml::to_string_pretty(self).context("encode configuration")?;
        let temporary = path.with_extension("toml.tmp");
        fs::write(&temporary, encoded).with_context(|| format!("write {}", temporary.display()))?;
        fs::rename(&temporary, path).with_context(|| format!("replace {}", path.display()))?;
        Ok(())
    }

    pub fn attach_project(&mut self, root: &Path, project: &str) -> Result<PathBuf> {
        let canonical = root
            .canonicalize()
            .with_context(|| format!("resolve project root {}", root.display()))?;
        if !canonical.is_dir() {
            bail!("project root {} is not a directory", canonical.display());
        }
        self.project_paths
            .insert(canonical.clone(), project.to_owned());
        Ok(canonical)
    }

    /// Resolve by longest matching directory root so nested projects override
    /// broader workspace mappings. Source: docs/architecture.md.
    pub fn project_for(&self, cwd: &Path) -> Result<Option<&str>> {
        let canonical = cwd
            .canonicalize()
            .with_context(|| format!("resolve current directory {}", cwd.display()))?;
        Ok(self
            .project_paths
            .iter()
            .filter(|(root, _)| canonical.starts_with(root))
            .max_by_key(|(root, _)| root.components().count())
            .map(|(_, project)| project.as_str())
            .or(self.default_project.as_deref()))
    }
}

#[cfg(test)]
mod tests {
    use super::AppConfig;
    use tempfile::tempdir;

    #[test]
    fn nested_project_mapping_wins() {
        let root = tempdir().unwrap();
        let nested = root.path().join("nested");
        std::fs::create_dir(&nested).unwrap();
        let mut config = AppConfig::default();
        config.attach_project(root.path(), "project/root").unwrap();
        config.attach_project(&nested, "project/nested").unwrap();
        assert_eq!(config.project_for(&nested).unwrap(), Some("project/nested"));
    }
}
