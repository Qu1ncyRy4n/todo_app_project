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
    /// The operator's actor name, so typed entry can say `by:me`.
    #[serde(default)]
    pub me: Option<String>,
    /// Accept `#tag`, `@actor`, and `+project` in typed entry. Default on.
    #[serde(default)]
    pub entry_ascii: Option<bool>,
    /// Environment variables whose presence means an agent is running `todo`.
    /// Default: [`DEFAULT_AGENT_ENV`].
    #[serde(default)]
    pub agent_env: Option<Vec<String>>,
    /// Whether listings `show`, `hide`, or `only` show agent-created items.
    /// Default `show`.
    #[serde(default)]
    pub agent_items: Option<String>,
}

/// Variables agent harnesses set in the shells they run.
// ponytail: a fixed guess at today's harnesses; `agent_env` overrides it and
// `TODO_AGENT` always wins.
pub const DEFAULT_AGENT_ENV: [&str; 3] = ["CLAUDECODE", "GEMINI_CLI", "CODEX_SANDBOX"];

impl AppConfig {
    /// The agent this process writes as, read through `var`.
    ///
    /// `TODO_AGENT=<name>` names it and `TODO_AGENT=` (empty) means the
    /// operator; otherwise any `agent_env` variable signs as `agent`.
    pub fn agent(&self, var: impl Fn(&str) -> Option<String>) -> Option<String> {
        if let Some(name) = var("TODO_AGENT") {
            return (!name.is_empty()).then_some(name);
        }
        let detected = match &self.agent_env {
            Some(names) => names.iter().any(|name| var(name).is_some()),
            None => DEFAULT_AGENT_ENV.iter().any(|name| var(name).is_some()),
        };
        detected.then(|| "agent".to_owned())
    }

    pub fn ensure_actor_id(&mut self) -> &str {
        self.actor_id
            .get_or_insert_with(|| Uuid::now_v7().to_string())
            .as_str()
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
    /// broader workspace mappings. Source: docs/architecture.md. The caller
    /// tries the Git remote and then `default_project` when this is `None`.
    pub fn project_for(&self, cwd: &Path) -> Result<Option<&str>> {
        let canonical = cwd
            .canonicalize()
            .with_context(|| format!("resolve current directory {}", cwd.display()))?;
        Ok(self
            .project_paths
            .iter()
            .filter(|(root, _)| canonical.starts_with(root))
            .max_by_key(|(root, _)| root.components().count())
            .map(|(_, project)| project.as_str()))
    }
}

#[cfg(test)]
#[allow(
    clippy::indexing_slicing,
    clippy::unwrap_used,
    reason = "test fixtures use direct setup and assertion values"
)]
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

    #[test]
    fn todo_agent_wins_and_agent_env_detects() {
        let config = AppConfig::default();
        let env = |pairs: &'static [(&'static str, &'static str)]| {
            move |name: &str| {
                pairs
                    .iter()
                    .find(|(key, _)| *key == name)
                    .map(|(_, value)| (*value).to_owned())
            }
        };
        assert_eq!(config.agent(env(&[])), None);
        assert_eq!(
            config.agent(env(&[("CLAUDECODE", "1")])).as_deref(),
            Some("agent")
        );
        assert_eq!(
            config.agent(env(&[("CLAUDECODE", "1"), ("TODO_AGENT", "")])),
            None
        );
        assert_eq!(
            config.agent(env(&[("TODO_AGENT", "bot")])).as_deref(),
            Some("bot")
        );
        let custom = AppConfig {
            agent_env: Some(vec!["MY_AGENT".to_owned()]),
            ..AppConfig::default()
        };
        assert_eq!(custom.agent(env(&[("CLAUDECODE", "1")])), None);
        assert_eq!(
            custom.agent(env(&[("MY_AGENT", "x")])).as_deref(),
            Some("agent")
        );
    }
}
