use anyhow::{Context, Result, anyhow, bail};
use chrono::{DateTime, Local, NaiveDate, Utc};
use cid::Cid;
use serde::Serialize;
use std::{
    cmp::Ordering,
    collections::{BTreeMap, HashSet},
    path::Path,
    str::FromStr,
};
use todo_core::{
    AGENT_FIELD, Event, EventBuilder, EventKind, GRID_CBOR_CODEC, GridEnvelope, Item, ItemId,
    ItemState, NamespaceId, NamespaceKind, NamespacePath, Priority, RelationKind, decode_envelope,
    event_protocol_v5_cid, event_protocol_v6_cid, event_protocol_v7_cid, event_protocol_v8_cid,
    event_protocol_v9_cid, event_protocol_v10_cid, event_protocol_v11_cid, event_protocol_v12_cid,
    event_protocol_v13_cid, event_protocol_v14_cid, event_protocol_v15_cid, validate_segment,
};
use turso::{Builder, Connection, Value};
use uuid::Uuid;

/// The one namespace relation protocol v14 defines: an actor belongs to another.
pub const MEMBER_OF: &str = "member-of";

/// Reduces a Git remote to `host/owner/name` so that
/// `git@github.com:o/n.git` and `https://github.com/o/n` compare equal.
///
/// ponytail: ignores ports and lowercases the whole path, which suits GitHub-style
/// hosts; keep the raw value if a case-sensitive host ever matters.
pub fn repo_key(remote: &str) -> String {
    let mut rest = remote.trim();
    let scheme = rest.split_once("://");
    if let Some((_, after)) = scheme {
        rest = after;
    }
    if let Some((_, after)) = rest.split_once('@') {
        rest = after;
    }
    let rest = rest.trim_end_matches('/');
    let rest = rest.strip_suffix(".git").unwrap_or(rest);
    let rest = if scheme.is_none() {
        rest.replacen(':', "/", 1)
    } else {
        rest.to_owned()
    };
    rest.to_lowercase()
}

const OPEN_RETRY_ATTEMPTS: usize = 50;
const OPEN_RETRY_DELAY: std::time::Duration = std::time::Duration::from_millis(20);

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS meta (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS events (
  cid TEXT PRIMARY KEY,
  parent_cid TEXT,
  event_id TEXT NOT NULL UNIQUE,
  kind TEXT NOT NULL,
  actor_id TEXT NOT NULL,
  subject_id TEXT NOT NULL,
  project_id TEXT,
  recorded_at TEXT NOT NULL,
  envelope BLOB NOT NULL
);
CREATE INDEX IF NOT EXISTS events_subject_time ON events(subject_id, recorded_at);
CREATE TABLE IF NOT EXISTS namespaces (
  id TEXT PRIMARY KEY,
  kind TEXT NOT NULL,
  canonical_path TEXT NOT NULL UNIQUE
);
CREATE TABLE IF NOT EXISTS namespace_aliases (
  namespace_id TEXT NOT NULL,
  path TEXT NOT NULL UNIQUE
);
CREATE TABLE IF NOT EXISTS items (
  id TEXT PRIMARY KEY,
  title TEXT NOT NULL,
  state TEXT NOT NULL,
  project_id TEXT,
  parent_id TEXT,
  priority TEXT NOT NULL DEFAULT 'normal',
  due_on TEXT,
  defer_until TEXT,
  scheduled_on TEXT,
  difficulty INTEGER,
  estimate_minutes INTEGER
);
CREATE INDEX IF NOT EXISTS items_project_state ON items(project_id, state);
CREATE TABLE IF NOT EXISTS notes (
  event_cid TEXT PRIMARY KEY,
  item_id TEXT NOT NULL,
  body TEXT NOT NULL,
  recorded_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS citations (
  event_cid TEXT PRIMARY KEY,
  item_id TEXT NOT NULL,
  target TEXT NOT NULL,
  label TEXT,
  recorded_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS item_namespaces (
  item_id TEXT NOT NULL,
  namespace_id TEXT NOT NULL,
  PRIMARY KEY(item_id, namespace_id)
);
CREATE TABLE IF NOT EXISTS item_dependencies (
  item_id TEXT NOT NULL,
  blocker_id TEXT NOT NULL,
  PRIMARY KEY(item_id, blocker_id)
);
CREATE INDEX IF NOT EXISTS dependencies_blocker ON item_dependencies(blocker_id);
CREATE TABLE IF NOT EXISTS item_blocks (
  item_id TEXT NOT NULL,
  blocker_id TEXT NOT NULL,
  PRIMARY KEY(item_id, blocker_id)
);
CREATE INDEX IF NOT EXISTS blocks_blocker ON item_blocks(blocker_id);
CREATE TABLE IF NOT EXISTS item_relations (
  item_id TEXT NOT NULL,
  relation TEXT NOT NULL,
  target_id TEXT NOT NULL,
  PRIMARY KEY(item_id, relation, target_id)
);
CREATE INDEX IF NOT EXISTS relations_target ON item_relations(target_id);
CREATE TABLE IF NOT EXISTS namespace_fields (
  namespace_id TEXT NOT NULL,
  key TEXT NOT NULL,
  value TEXT NOT NULL,
  PRIMARY KEY(namespace_id, key)
);
CREATE TABLE IF NOT EXISTS namespace_links (
  namespace_id TEXT NOT NULL,
  relation TEXT NOT NULL,
  target_id TEXT NOT NULL,
  PRIMARY KEY(namespace_id, relation, target_id)
);
CREATE INDEX IF NOT EXISTS namespace_links_target ON namespace_links(target_id);
CREATE TABLE IF NOT EXISTS item_roles (
  item_id TEXT NOT NULL,
  role TEXT NOT NULL,
  namespace_id TEXT NOT NULL,
  PRIMARY KEY(item_id, role, namespace_id)
);
CREATE INDEX IF NOT EXISTS item_roles_namespace ON item_roles(namespace_id);
CREATE TABLE IF NOT EXISTS focus_sequences (
  id TEXT PRIMARY KEY,
  title TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS focus_sequence_entries (
  sequence_id TEXT NOT NULL,
  position INTEGER NOT NULL,
  item_id TEXT NOT NULL,
  PRIMARY KEY(sequence_id, position)
);
CREATE TABLE IF NOT EXISTS item_sessions (
  session_id TEXT PRIMARY KEY,
  item_id TEXT NOT NULL,
  started_at TEXT NOT NULL,
  finished_at TEXT
);
"#;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct StoredItem {
    pub item: Item,
    pub namespaces: Vec<NamespacePath>,
    /// Planned prerequisites. Historical `item_dependencies` rows populate this.
    pub dependencies: Vec<ItemId>,
    /// Current impediments. The inverse `blocks` view is derived from this edge.
    pub blocked_by: Vec<ItemId>,
    /// Typed relations that may cross projects and never affect readiness.
    pub relations: Vec<ItemRelation>,
    /// The agent that created the item, if one did. Source: protocol v15.
    pub agent: Option<String>,
}

/// One typed relation from an item to another item.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct ItemRelation {
    pub relation: RelationKind,
    pub target: ItemId,
}

/// A task hidden by deletion, as reported by [`Ledger::deleted_items`].
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DeletedItem {
    pub id: ItemId,
    pub title: String,
    /// RFC 3339 timestamp of the deletion.
    pub deleted_at: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BatchItem {
    pub title: String,
    pub state: ItemState,
    pub parent_index: Option<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Note {
    pub body: String,
    pub recorded_at: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Citation {
    pub target: String,
    pub label: Option<String>,
    pub recorded_at: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct FocusSequence {
    pub id: String,
    pub title: String,
    pub items: Vec<ItemId>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct WorkSession {
    pub id: String,
    pub item_id: ItemId,
    pub started_at: String,
    pub finished_at: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ItemPatch {
    pub title: Option<String>,
    pub priority: Option<Priority>,
    pub state: Option<ItemState>,
    pub due_on: Option<Option<NaiveDate>>,
    pub defer_until: Option<Option<NaiveDate>>,
    pub scheduled_on: Option<Option<NaiveDate>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HistoryEntry {
    pub cid: String,
    pub kind: String,
    pub recorded_at: String,
    /// The agent that wrote the event, if one did.
    pub agent: Option<String>,
}

/// An immutable event as it was accepted by this ledger. The stored CID is
/// carried separately because historical protocols need not use v5's codec.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EventRecord {
    pub cid: Cid,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NamespaceEntry {
    pub id: NamespaceId,
    pub kind: NamespaceKind,
    pub path: NamespacePath,
}

#[derive(Clone, Copy, Debug)]
pub enum ItemScope<'a> {
    All,
    Unscoped,
    Project(&'a NamespacePath),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NextItem {
    pub item: StoredItem,
    pub reasons: Vec<String>,
}

pub struct Ledger {
    connection: Connection,
    actor_id: String,
    agent: Option<String>,
}

impl Ledger {
    pub async fn open(path: &Path) -> Result<Self> {
        Self::open_with_actor(path, "local").await
    }

    pub async fn open_with_actor(path: &Path, actor_id: impl Into<String>) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create database directory {}", parent.display()))?;
        }
        let path = path
            .to_str()
            .ok_or_else(|| anyhow!("database path is not valid UTF-8: {}", path.display()))?;
        // Stage-one dogfooding includes short-lived CLI and agent processes. Turso
        // can transiently reject a second process while opening its local WAL, so
        // wait briefly rather than exposing an implementation-level race. Source: DI-0007.
        let mut attempt = 0;
        let database = loop {
            match Builder::new_local(path).build().await {
                Ok(database) => break database,
                Err(error) if attempt + 1 < OPEN_RETRY_ATTEMPTS && is_lock_error(&error) => {
                    attempt += 1;
                    tokio::time::sleep(OPEN_RETRY_DELAY).await;
                }
                Err(error) => return Err(error).context("open Turso database"),
            }
        };
        let connection = database.connect().context("connect Turso database")?;
        let ledger = Self {
            connection,
            actor_id: actor_id.into(),
            agent: None,
        };
        ledger.migrate().await?;
        Ok(ledger)
    }

    /// Signs every event this ledger writes as the named agent.
    pub fn with_agent(mut self, agent: Option<String>) -> Self {
        self.agent = agent;
        self
    }

    async fn migrate(&self) -> Result<()> {
        // Turso's Rust API currently exposes single-statement execution. Keep
        // migrations simple and fail before application writes. Source: DI-0002.
        for statement in SCHEMA
            .split(';')
            .map(str::trim)
            .filter(|sql| !sql.is_empty())
        {
            self.connection
                .execute(statement, ())
                .await
                .with_context(|| format!("apply schema statement {statement}"))?;
        }
        self.add_column_if_missing("priority", "priority TEXT NOT NULL DEFAULT 'normal'")
            .await?;
        self.add_column_if_missing("due_on", "due_on TEXT").await?;
        self.add_column_if_missing("defer_until", "defer_until TEXT")
            .await?;
        self.add_column_if_missing("scheduled_on", "scheduled_on TEXT")
            .await?;
        self.add_column_if_missing("deleted_at", "deleted_at TEXT")
            .await?;
        self.add_column_if_missing("difficulty", "difficulty INTEGER")
            .await?;
        self.add_column_if_missing("estimate_minutes", "estimate_minutes INTEGER")
            .await?;
        self.add_column_if_missing("created_by_agent", "created_by_agent TEXT")
            .await?;
        self.connection
            .execute(
                "INSERT INTO meta(key, value) VALUES ('schema_version', '2') ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                (),
            )
            .await?;
        Ok(())
    }

    async fn add_column_if_missing(&self, name: &str, definition: &str) -> Result<()> {
        let mut rows = self
            .connection
            .query("PRAGMA table_info(items)", ())
            .await?;
        while let Some(row) = rows.next().await? {
            if text(&row, 1)? == name {
                return Ok(());
            }
        }
        self.connection
            .execute(&format!("ALTER TABLE items ADD COLUMN {definition}"), ())
            .await?;
        Ok(())
    }

    pub async fn ensure_namespace(&self, path: &NamespacePath) -> Result<NamespaceEntry> {
        if let Some(existing) = self.namespace_by_path(path).await? {
            return Ok(existing);
        }
        let id = NamespaceId::new();
        let kind = path.kind()?;
        let event = self
            .event(
                EventBuilder::new(EventKind::NamespaceCreated, id.to_string())
                    .data("kind", kind.to_string())
                    .data("path", path.to_string()),
            )
            .await?;
        self.transaction_begin().await?;
        let result = async {
            self.insert_event(&event).await?;
            self.connection
                .execute(
                    "INSERT INTO namespaces(id, kind, canonical_path) VALUES (?1, ?2, ?3)",
                    values([id.to_string(), kind.to_string(), path.to_string()]),
                )
                .await?;
            self.set_head(&event.cid).await
        }
        .await;
        self.transaction_finish(result).await?;
        Ok(NamespaceEntry {
            id,
            kind,
            path: path.clone(),
        })
    }

    pub async fn rename_namespace(
        &self,
        current: &NamespacePath,
        next: &NamespacePath,
    ) -> Result<()> {
        let namespace = self
            .namespace_by_path(current)
            .await?
            .ok_or_else(|| anyhow!("unknown namespace {current}"))?;
        let next_kind = next.kind()?;
        if namespace.kind != next_kind {
            bail!(
                "cannot change namespace kind from {} to {}",
                namespace.kind,
                next_kind
            );
        }
        if self.namespace_by_path(next).await?.is_some() {
            bail!("namespace path {next} already exists");
        }
        let event = self
            .event(
                EventBuilder::new(EventKind::NamespaceRenamed, namespace.id.to_string())
                    .data("from", current.to_string())
                    .data("to", next.to_string()),
            )
            .await?;
        self.transaction_begin().await?;
        let result = async {
            self.insert_event(&event).await?;
            self.connection
                .execute(
                    "INSERT INTO namespace_aliases(namespace_id, path) VALUES (?1, ?2)",
                    values([namespace.id.to_string(), current.to_string()]),
                )
                .await?;
            self.connection
                .execute(
                    "UPDATE namespaces SET canonical_path = ?1 WHERE id = ?2",
                    values([next.to_string(), namespace.id.to_string()]),
                )
                .await?;
            self.set_head(&event.cid).await
        }
        .await;
        self.transaction_finish(result).await
    }

    pub async fn add_item(
        &self,
        title: &str,
        project: Option<&NamespacePath>,
    ) -> Result<StoredItem> {
        self.add_item_with(title, project, &[], None).await
    }

    /// Create the item and all of its initial relationships in one transaction.
    /// Namespace definitions may be created beforehand, but a failed request
    /// never leaves a partially configured item behind.
    pub async fn add_item_with(
        &self,
        title: &str,
        project: Option<&NamespacePath>,
        namespaces: &[NamespacePath],
        parent_reference: Option<&str>,
    ) -> Result<StoredItem> {
        let title = title.trim();
        if title.is_empty() {
            bail!("item title cannot be empty");
        }
        let project = match project {
            Some(path) => Some(self.ensure_namespace(path).await?),
            None => None,
        };
        let mut namespace_entries = Vec::new();
        for path in namespaces {
            let entry = self.ensure_namespace(path).await?;
            if !namespace_entries
                .iter()
                .any(|existing: &NamespaceEntry| existing.id == entry.id)
            {
                namespace_entries.push(entry);
            }
        }
        let parent = match parent_reference {
            Some(reference) => Some(self.get_item(reference).await?),
            None => None,
        };
        if let Some(parent) = &parent
            && parent.item.project_id != project.as_ref().map(|entry| entry.id)
        {
            bail!("an item and its structural parent must be in the same project scope");
        }

        let id = ItemId::new();
        let project_id = project.as_ref().map(|entry| entry.id);
        let mut previous = self.head().await?;
        let mut events = Vec::new();
        let created = self.sign(
            EventBuilder::for_item(EventKind::ItemCreated, id)
                .project(project_id)
                .parent(previous)
                .data("title", title)
                .data("state", ItemState::Inbox.to_string()),
        )?;
        previous = Some(created.cid);
        let mut head = created.cid;
        events.push(created);
        for namespace in &namespace_entries {
            let event = self.sign(
                EventBuilder::for_item(EventKind::NamespaceAssigned, id)
                    .project(project_id)
                    .parent(previous)
                    .link("namespace", namespace.id.to_string()),
            )?;
            previous = Some(event.cid);
            head = event.cid;
            events.push(event);
        }
        if let Some(parent) = &parent {
            let event = self.sign(
                EventBuilder::for_item(EventKind::ItemParentSet, id)
                    .project(project_id)
                    .parent(previous)
                    .link("parent", parent.item.id.to_string()),
            )?;
            head = event.cid;
            events.push(event);
        }

        self.transaction_begin().await?;
        let result = async {
            for event in &events {
                self.insert_event(event).await?;
            }
            self.connection
                .execute(
                    "INSERT INTO items(id, title, state, project_id, parent_id, created_by_agent) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    vec![
                        Value::Text(id.to_string()),
                        Value::Text(title.to_owned()),
                        Value::Text(ItemState::Inbox.to_string()),
                        project.as_ref().map_or(Value::Null, |entry| Value::Text(entry.id.to_string())),
                        parent.as_ref().map_or(Value::Null, |stored| Value::Text(stored.item.id.to_string())),
                        self.agent.clone().map_or(Value::Null, Value::Text),
                    ],
                )
                .await?;
            for namespace in &namespace_entries {
                self.connection
                    .execute(
                        "INSERT INTO item_namespaces(item_id, namespace_id) VALUES (?1, ?2)",
                        values([id.to_string(), namespace.id.to_string()]),
                    )
                    .await?;
            }
            self.set_head(&head).await
        }
        .await;
        self.transaction_finish(result).await?;
        Ok(StoredItem {
            item: Item {
                id,
                title: title.to_owned(),
                state: ItemState::Inbox,
                project_id,
                parent_id: parent.as_ref().map(|stored| stored.item.id),
                priority: Priority::Normal,
                due_on: None,
                defer_until: None,
                scheduled_on: None,
                difficulty: None,
                estimate_minutes: None,
            },
            relations: Vec::new(),
            agent: self.agent.clone(),
            namespaces: namespace_entries
                .into_iter()
                .map(|entry| entry.path)
                .collect(),
            dependencies: Vec::new(),
            blocked_by: Vec::new(),
        })
    }

    pub async fn import_batch(
        &self,
        project: Option<&NamespacePath>,
        items: &[BatchItem],
        source_citation: Option<&str>,
    ) -> Result<Vec<StoredItem>> {
        if items.is_empty() {
            bail!("import contains no todo items");
        }
        for (index, item) in items.iter().enumerate() {
            if item.title.trim().is_empty() {
                bail!("import item {} has an empty title", index + 1);
            }
            if item.parent_index.is_some_and(|parent| parent >= index) {
                bail!("import item {} has an invalid parent", index + 1);
            }
        }
        let source_citation = source_citation
            .map(str::trim)
            .filter(|source| !source.is_empty());
        let project = match project {
            Some(path) => Some(self.ensure_namespace(path).await?),
            None => None,
        };
        let project_id = project.as_ref().map(|entry| entry.id);
        let ids: Vec<_> = items.iter().map(|_| ItemId::new()).collect();
        let mut previous = self.head().await?;
        let mut built = Vec::new();

        for (index, item) in items.iter().enumerate() {
            let id = *ids
                .get(index)
                .ok_or_else(|| anyhow!("import item ID is missing"))?;
            let mut events = Vec::new();
            let created = self.sign(
                EventBuilder::for_item(EventKind::ItemCreated, id)
                    .project(project_id)
                    .parent(previous)
                    .data("title", item.title.trim())
                    .data("state", item.state.to_string())
                    .data("imported", "true"),
            )?;
            previous = Some(created.cid);
            events.push(created);
            if let Some(parent_index) = item.parent_index {
                let parent_id = ids
                    .get(parent_index)
                    .ok_or_else(|| anyhow!("import item parent ID is missing"))?;
                let parent = self.sign(
                    EventBuilder::for_item(EventKind::ItemParentSet, id)
                        .project(project_id)
                        .parent(previous)
                        .link("parent", parent_id.to_string()),
                )?;
                previous = Some(parent.cid);
                events.push(parent);
            }
            if let Some(source) = source_citation {
                let citation = self.sign(
                    EventBuilder::for_item(EventKind::ItemCitationAdded, id)
                        .project(project_id)
                        .parent(previous)
                        .data("target", source)
                        .data("label", "import source"),
                )?;
                previous = Some(citation.cid);
                events.push(citation);
            }
            built.push((item, events));
        }
        let head = previous.ok_or_else(|| anyhow!("non-empty import did not emit events"))?;

        self.transaction_begin().await?;
        let result = async {
            for ((item, events), id) in built.iter().zip(&ids) {
                for event in events {
                    self.insert_event(event).await?;
                }
                let parent = item
                    .parent_index
                    .map(|parent| {
                        ids.get(parent)
                            .map(|id| Value::Text(id.to_string()))
                            .ok_or_else(|| anyhow!("import item parent ID is missing"))
                    })
                    .transpose()?
                    .unwrap_or(Value::Null);
                self.connection
                    .execute(
                        "INSERT INTO items(id, title, state, project_id, parent_id, created_by_agent) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                        vec![
                            Value::Text(id.to_string()),
                            Value::Text(item.title.trim().to_owned()),
                            Value::Text(item.state.to_string()),
                            project_id.map_or(Value::Null, |id| Value::Text(id.to_string())),
                            parent,
                            self.agent.clone().map_or(Value::Null, Value::Text),
                        ],
                    )
                    .await?;
                if let Some(source) = source_citation {
                    let citation = events
                        .iter()
                        .find(|event| event.payload.kind == EventKind::ItemCitationAdded)
                        .ok_or_else(|| anyhow!("import item did not create its source citation"))?;
                    self.connection
                        .execute(
                            "INSERT INTO citations(event_cid, item_id, target, label, recorded_at) VALUES (?1, ?2, ?3, 'import source', ?4)",
                            values([
                                citation.cid.to_string(),
                                id.to_string(),
                                source.to_owned(),
                                citation.payload.recorded_at.to_rfc3339(),
                            ]),
                        )
                        .await?;
                }
            }
            self.set_head(&head).await
        }
        .await;
        self.transaction_finish(result).await?;

        let mut imported = Vec::new();
        for id in ids {
            imported.push(self.get_item(&id.to_string()).await?);
        }
        Ok(imported)
    }

    pub async fn set_state(&self, reference: &str, next: ItemState) -> Result<StoredItem> {
        let current = self.get_item(reference).await?;
        if next == ItemState::Active
            && let Some(defer_until) = current.item.defer_until
            && defer_until > Local::now().date_naive()
        {
            bail!("item is deferred until {defer_until}; clear the defer date before starting it");
        }
        if !current.item.state.can_transition_to(next) {
            bail!("cannot move item from {} to {next}", current.item.state);
        }
        let event = self
            .event(
                EventBuilder::for_item(EventKind::ItemStateChanged, current.item.id)
                    .project(current.item.project_id)
                    .data("from", current.item.state.to_string())
                    .data("to", next.to_string()),
            )
            .await?;
        self.transaction_begin().await?;
        let result = async {
            self.insert_event(&event).await?;
            self.connection
                .execute(
                    "UPDATE items SET state = ?1 WHERE id = ?2",
                    values([next.to_string(), current.item.id.to_string()]),
                )
                .await?;
            self.set_head(&event.cid).await
        }
        .await;
        self.transaction_finish(result).await?;
        self.get_item(&current.item.id.to_string()).await
    }

    /// Sets or clears an item's difficulty, from 1 to 5.
    ///
    /// Difficulty is descriptive: nothing in the ledger derives readiness or
    /// ordering from it, so a reader is free to weigh it however it likes.
    pub async fn set_difficulty(
        &self,
        reference: &str,
        difficulty: Option<u8>,
    ) -> Result<StoredItem> {
        if let Some(value) = difficulty
            && !(1..=5).contains(&value)
        {
            bail!("difficulty must be between 1 and 5");
        }
        let current = self.get_item(reference).await?;
        let rendered = difficulty
            .map(|value| value.to_string())
            .unwrap_or_default();
        let event = self
            .event(
                EventBuilder::for_item(EventKind::ItemDifficultyChanged, current.item.id)
                    .project(current.item.project_id)
                    .data("difficulty", rendered.clone()),
            )
            .await?;
        self.transaction_begin().await?;
        let result = async {
            self.insert_event(&event).await?;
            self.connection
                .execute(
                    "UPDATE items SET difficulty = ?1 WHERE id = ?2",
                    vec![
                        parse_optional_number(&rendered)?,
                        Value::Text(current.item.id.to_string()),
                    ],
                )
                .await?;
            self.set_head(&event.cid).await
        }
        .await;
        self.transaction_finish(result).await?;
        self.get_item(&current.item.id.to_string()).await
    }

    /// Sets or clears an item's effort estimate, in minutes.
    pub async fn set_estimate(&self, reference: &str, minutes: Option<u32>) -> Result<StoredItem> {
        let current = self.get_item(reference).await?;
        let rendered = minutes.map(|value| value.to_string()).unwrap_or_default();
        let event = self
            .event(
                EventBuilder::for_item(EventKind::ItemEstimateChanged, current.item.id)
                    .project(current.item.project_id)
                    .data("estimate_minutes", rendered.clone()),
            )
            .await?;
        self.transaction_begin().await?;
        let result = async {
            self.insert_event(&event).await?;
            self.connection
                .execute(
                    "UPDATE items SET estimate_minutes = ?1 WHERE id = ?2",
                    vec![
                        parse_optional_number(&rendered)?,
                        Value::Text(current.item.id.to_string()),
                    ],
                )
                .await?;
            self.set_head(&event.cid).await
        }
        .await;
        self.transaction_finish(result).await?;
        self.get_item(&current.item.id.to_string()).await
    }

    /// Marks an item deleted, hiding it from every ordinary read.
    ///
    /// The item's rows are retained and the log stays append-only, so the deletion
    /// can be reversed with [`Ledger::restore_item`]. A deleted item keeps its
    /// relationships, but they are hidden with it.
    ///
    /// Refuses while other visible items still point at this one as a parent,
    /// dependency, or blocker, so a delete cannot silently orphan work.
    pub async fn delete_item(&self, reference: &str) -> Result<DeletedItem> {
        let current = self.get_item(reference).await?;
        let blockers = self.deletion_blockers(current.item.id).await?;
        if !blockers.is_empty() {
            bail!(
                "item {} is still referenced by {}; detach those references first",
                current.item.id,
                blockers.join(", ")
            );
        }
        let deleted_at = Utc::now();
        let event = self
            .event(
                EventBuilder::for_item(EventKind::ItemDeleted, current.item.id)
                    .project(current.item.project_id)
                    .data("deleted_at", deleted_at.to_rfc3339())
                    .data("title", current.item.title.clone()),
            )
            .await?;
        self.transaction_begin().await?;
        let result = async {
            self.insert_event(&event).await?;
            self.connection
                .execute(
                    "UPDATE items SET deleted_at = ?1 WHERE id = ?2",
                    values([deleted_at.to_rfc3339(), current.item.id.to_string()]),
                )
                .await?;
            self.set_head(&event.cid).await
        }
        .await;
        self.transaction_finish(result).await?;
        Ok(DeletedItem {
            id: current.item.id,
            title: current.item.title,
            deleted_at: deleted_at.to_rfc3339(),
        })
    }

    /// Clears an item's deletion mark, returning it to ordinary reads.
    pub async fn restore_item(&self, reference: &str) -> Result<StoredItem> {
        let deleted = self.deleted_items().await?;
        let reference = reference.trim();
        // Compared without hyphens so a full identifier, a trailing fragment, and a
        // decoded proquint all match the same stored id.
        let matched =
            todo_core::proquint_to_hex(reference).unwrap_or_else(|| reference.replace('-', ""));
        let matches = deleted
            .into_iter()
            .filter(|entry| {
                let id = entry.id.to_string().replace('-', "");
                id == matched || id.ends_with(&matched)
            })
            .collect::<Vec<_>>();
        let found = match matches.len() {
            0 => bail!("no deleted item matches {reference}"),
            1 => matches.into_iter().next(),
            _ => bail!("deleted item reference {reference} is ambiguous; use the full ID"),
        };
        let Some(found) = found else {
            bail!("no deleted item matches {reference}");
        };
        let event = self
            .event(EventBuilder::for_item(EventKind::ItemRestored, found.id))
            .await?;
        self.transaction_begin().await?;
        let result = async {
            self.insert_event(&event).await?;
            self.connection
                .execute(
                    "UPDATE items SET deleted_at = NULL WHERE id = ?1",
                    values([found.id.to_string()]),
                )
                .await?;
            self.set_head(&event.cid).await
        }
        .await;
        self.transaction_finish(result).await?;
        self.get_item(&found.id.to_string()).await
    }

    /// Returns items marked void, which every other listing leaves out.
    ///
    /// Void items are noise by definition, so nothing surfaces them by accident;
    /// this is how one is found again when it was voided by mistake.
    pub async fn void_items(&self) -> Result<Vec<StoredItem>> {
        let mut rows = self
            .connection
            .query(
                "SELECT id, title, state, project_id, parent_id, priority, due_on, defer_until, scheduled_on, difficulty, estimate_minutes FROM items WHERE deleted_at IS NULL AND state = 'void' ORDER BY id",
                (),
            )
            .await?;
        let mut items = Vec::new();
        while let Some(row) = rows.next().await? {
            let item = item_from_row(&row)?;
            let namespaces = self.item_namespaces(item.id).await?;
            let dependencies = self.item_dependencies(item.id).await?;
            let blocked_by = self.item_active_blockers(item.id).await?;
            let relations = self.item_relations(item.id).await?;
            let agent = self.item_agent(item.id).await?;
            items.push(StoredItem {
                item,
                namespaces,
                dependencies,
                blocked_by,
                relations,
                agent,
            });
        }
        Ok(items)
    }

    /// Returns every deleted item, newest deletion first.
    pub async fn deleted_items(&self) -> Result<Vec<DeletedItem>> {
        let mut rows = self
            .connection
            .query(
                "SELECT id, title, deleted_at FROM items WHERE deleted_at IS NOT NULL ORDER BY deleted_at DESC, id",
                (),
            )
            .await?;
        let mut deleted = Vec::new();
        while let Some(row) = rows.next().await? {
            deleted.push(DeletedItem {
                id: ItemId::parse(&text(&row, 0)?)?,
                title: text(&row, 1)?,
                deleted_at: text(&row, 2)?,
            });
        }
        Ok(deleted)
    }

    /// Returns descriptions of visible items that still reference this one.
    async fn deletion_blockers(&self, id: ItemId) -> Result<Vec<String>> {
        let mut blockers = Vec::new();
        let checks = [
            (
                "SELECT 1 FROM items WHERE parent_id = ?1 AND deleted_at IS NULL LIMIT 1",
                "child tasks",
            ),
            (
                "SELECT 1 FROM item_dependencies WHERE blocker_id = ?1 LIMIT 1",
                "dependencies",
            ),
            (
                "SELECT 1 FROM item_blocks WHERE blocker_id = ?1 LIMIT 1",
                "active blockers",
            ),
        ];
        for (sql, label) in checks {
            let mut rows = self.connection.query(sql, values([id.to_string()])).await?;
            if rows.next().await?.is_some() {
                blockers.push(label.to_owned());
            }
        }
        Ok(blockers)
    }

    pub async fn edit_title(&self, reference: &str, title: &str) -> Result<StoredItem> {
        let current = self.get_item(reference).await?;
        let title = title.trim();
        if title.is_empty() {
            bail!("item title cannot be empty");
        }
        if current.item.title == title {
            return Ok(current);
        }
        let event = self
            .event(
                EventBuilder::for_item(EventKind::ItemTitleChanged, current.item.id)
                    .project(current.item.project_id)
                    .data("from", &current.item.title)
                    .data("to", title),
            )
            .await?;
        self.transaction_begin().await?;
        let result = async {
            self.insert_event(&event).await?;
            self.connection
                .execute(
                    "UPDATE items SET title = ?1 WHERE id = ?2",
                    values([title.to_owned(), current.item.id.to_string()]),
                )
                .await?;
            self.set_head(&event.cid).await
        }
        .await;
        self.transaction_finish(result).await?;
        self.get_item(&current.item.id.to_string()).await
    }

    pub async fn set_priority(&self, reference: &str, next: Priority) -> Result<StoredItem> {
        let current = self.get_item(reference).await?;
        if current.item.priority == next {
            return Ok(current);
        }
        let event = self
            .event(
                EventBuilder::for_item(EventKind::ItemPriorityChanged, current.item.id)
                    .project(current.item.project_id)
                    .data("from", current.item.priority.to_string())
                    .data("to", next.to_string()),
            )
            .await?;
        self.transaction_begin().await?;
        let result = async {
            self.insert_event(&event).await?;
            self.connection
                .execute(
                    "UPDATE items SET priority = ?1 WHERE id = ?2",
                    values([next.to_string(), current.item.id.to_string()]),
                )
                .await?;
            self.set_head(&event.cid).await
        }
        .await;
        self.transaction_finish(result).await?;
        self.get_item(&current.item.id.to_string()).await
    }

    pub async fn apply_patch(&self, reference: &str, patch: ItemPatch) -> Result<StoredItem> {
        let current = self.get_item(reference).await?;
        if patch
            .title
            .as_deref()
            .is_some_and(|title| title.trim().is_empty())
        {
            bail!("title cannot be empty");
        }
        if patch
            .state
            .is_some_and(|state| !current.item.state.can_transition_to(state))
        {
            bail!("invalid lifecycle transition");
        }
        let mut builder = EventBuilder::for_item(EventKind::ItemPatched, current.item.id)
            .project(current.item.project_id);
        if let Some(title) = &patch.title {
            builder = builder.data("title", title.trim());
        }
        if let Some(priority) = patch.priority {
            builder = builder.data("priority", priority.to_string());
        }
        if let Some(state) = patch.state {
            builder = builder.data("state", state.to_string());
        }
        if let Some(date) = patch.due_on {
            builder = builder.data(
                "due_on",
                date.map(|value| value.to_string()).unwrap_or_default(),
            );
        }
        if let Some(date) = patch.defer_until {
            builder = builder.data(
                "defer_until",
                date.map(|value| value.to_string()).unwrap_or_default(),
            );
        }
        if let Some(date) = patch.scheduled_on {
            builder = builder.data(
                "scheduled_on",
                date.map(|value| value.to_string()).unwrap_or_default(),
            );
        }
        let event = self.event(builder).await?;
        self.transaction_begin().await?;
        let result = async {
            self.insert_event(&event).await?;
            self.apply_event(&event).await?;
            self.set_head(&event.cid).await
        }
        .await;
        self.transaction_finish(result).await?;
        self.get_item(&current.item.id.to_string()).await
    }

    pub async fn set_due_on(&self, reference: &str, next: Option<NaiveDate>) -> Result<StoredItem> {
        self.set_date(reference, next, "due_on", EventKind::ItemDueDateChanged)
            .await
    }

    pub async fn set_defer_until(
        &self,
        reference: &str,
        next: Option<NaiveDate>,
    ) -> Result<StoredItem> {
        let current = self.get_item(reference).await?;
        if current.item.state == ItemState::Active
            && next.is_some_and(|date| date > Local::now().date_naive())
        {
            bail!(
                "active items cannot be deferred into the future; complete, cancel, or model a separate follow-up item"
            );
        }
        self.set_date(
            reference,
            next,
            "defer_until",
            EventKind::ItemDeferDateChanged,
        )
        .await
    }

    async fn set_date(
        &self,
        reference: &str,
        next: Option<NaiveDate>,
        column: &str,
        kind: EventKind,
    ) -> Result<StoredItem> {
        let current = self.get_item(reference).await?;
        let previous = match column {
            "due_on" => current.item.due_on,
            "defer_until" => current.item.defer_until,
            _ => bail!("invalid internal date field {column}"),
        };
        if previous == next {
            return Ok(current);
        }
        let event = self
            .event(
                EventBuilder::for_item(kind, current.item.id)
                    .project(current.item.project_id)
                    .data("field", column)
                    .data(
                        "from",
                        previous.map(|date| date.to_string()).unwrap_or_default(),
                    )
                    .data("to", next.map(|date| date.to_string()).unwrap_or_default()),
            )
            .await?;
        self.transaction_begin().await?;
        let result = async {
            self.insert_event(&event).await?;
            self.connection
                .execute(
                    &format!("UPDATE items SET {column} = ?1 WHERE id = ?2"),
                    vec![
                        next.map_or(Value::Null, |date| Value::Text(date.to_string())),
                        Value::Text(current.item.id.to_string()),
                    ],
                )
                .await?;
            self.set_head(&event.cid).await
        }
        .await;
        self.transaction_finish(result).await?;
        self.get_item(&current.item.id.to_string()).await
    }

    pub async fn add_note(&self, reference: &str, body: &str) -> Result<()> {
        let item = self.get_item(reference).await?;
        self.add_text_record(EventKind::ItemNoteAdded, &item, "note", body, "notes")
            .await
    }

    pub async fn add_citation(
        &self,
        reference: &str,
        target: &str,
        label: Option<&str>,
    ) -> Result<()> {
        let item = self.get_item(reference).await?;
        let mut builder = EventBuilder::for_item(EventKind::ItemCitationAdded, item.item.id)
            .project(item.item.project_id)
            .data("target", target.trim());
        if let Some(label) = label.filter(|label| !label.trim().is_empty()) {
            builder = builder.data("label", label.trim());
        }
        let event = self.event(builder).await?;
        self.transaction_begin().await?;
        let result = async {
            self.insert_event(&event).await?;
            self.connection
                .execute(
                    "INSERT INTO citations(event_cid, item_id, target, label, recorded_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                    vec![
                        Value::Text(event.cid.to_string()),
                        Value::Text(item.item.id.to_string()),
                        Value::Text(target.trim().to_owned()),
                        label.map_or(Value::Null, |value| Value::Text(value.trim().to_owned())),
                        Value::Text(event.payload.recorded_at.to_rfc3339()),
                    ],
                )
                .await?;
            self.set_head(&event.cid).await
        }
        .await;
        self.transaction_finish(result).await
    }

    async fn add_text_record(
        &self,
        kind: EventKind,
        item: &StoredItem,
        key: &str,
        body: &str,
        table: &str,
    ) -> Result<()> {
        let body = body.trim();
        if body.is_empty() {
            bail!("{key} cannot be empty");
        }
        let event = self
            .event(
                EventBuilder::for_item(kind, item.item.id)
                    .project(item.item.project_id)
                    .data(key, body),
            )
            .await?;
        self.transaction_begin().await?;
        let result = async {
            self.insert_event(&event).await?;
            let sql = format!(
                "INSERT INTO {table}(event_cid, item_id, body, recorded_at) VALUES (?1, ?2, ?3, ?4)"
            );
            self.connection
                .execute(
                    &sql,
                    values([
                        event.cid.to_string(),
                        item.item.id.to_string(),
                        body.to_owned(),
                        event.payload.recorded_at.to_rfc3339(),
                    ]),
                )
                .await?;
            self.set_head(&event.cid).await
        }
        .await;
        self.transaction_finish(result).await
    }

    pub async fn set_parent(&self, child_reference: &str, parent_reference: &str) -> Result<()> {
        let child = self.get_item(child_reference).await?;
        let parent = self.get_item(parent_reference).await?;
        if child.item.id == parent.item.id {
            bail!("an item cannot be its own parent");
        }
        if child.item.project_id != parent.item.project_id {
            bail!("an item and its structural parent must be in the same project scope");
        }
        if self
            .would_create_cycle(child.item.id, parent.item.id)
            .await?
        {
            bail!("parent relation would create a cycle");
        }
        let event = self
            .event(
                EventBuilder::for_item(EventKind::ItemParentSet, child.item.id)
                    .project(child.item.project_id)
                    .link("parent", parent.item.id.to_string()),
            )
            .await?;
        self.transaction_begin().await?;
        let result = async {
            self.insert_event(&event).await?;
            self.connection
                .execute(
                    "UPDATE items SET parent_id = ?1 WHERE id = ?2",
                    values([parent.item.id.to_string(), child.item.id.to_string()]),
                )
                .await?;
            self.set_head(&event.cid).await
        }
        .await;
        self.transaction_finish(result).await
    }

    pub async fn clear_parent(&self, reference: &str) -> Result<()> {
        let item = self.get_item(reference).await?;
        let Some(parent) = item.item.parent_id else {
            return Ok(());
        };
        let event = self
            .event(
                EventBuilder::for_item(EventKind::ItemParentCleared, item.item.id)
                    .project(item.item.project_id)
                    .link("former-parent", parent.to_string()),
            )
            .await?;
        self.transaction_begin().await?;
        let result = async {
            self.insert_event(&event).await?;
            self.connection
                .execute(
                    "UPDATE items SET parent_id = NULL WHERE id = ?1",
                    values([item.item.id.to_string()]),
                )
                .await?;
            self.set_head(&event.cid).await
        }
        .await;
        self.transaction_finish(result).await
    }

    pub async fn move_project(
        &self,
        reference: &str,
        target: Option<&NamespacePath>,
    ) -> Result<StoredItem> {
        let item = self.get_item(reference).await?;
        let target = match target {
            Some(path) => Some(self.ensure_namespace(path).await?),
            None => None,
        };
        let target_id = target.as_ref().map(|entry| entry.id);
        if item.item.project_id == target_id {
            return Ok(item);
        }
        if item.item.parent_id.is_some()
            || !item.dependencies.is_empty()
            || !item.blocked_by.is_empty()
            || self.has_children(item.item.id).await?
            || self.has_dependents(item.item.id).await?
        {
            bail!(
                "cannot move an item with parent, children, dependencies, blockers, or dependents; clear those relationships first"
            );
        }
        let event = self
            .event(
                EventBuilder::for_item(EventKind::ItemProjectChanged, item.item.id)
                    .project(target_id)
                    .data(
                        "from",
                        item.item
                            .project_id
                            .map(|id| id.to_string())
                            .unwrap_or_default(),
                    )
                    .data("to", target_id.map(|id| id.to_string()).unwrap_or_default()),
            )
            .await?;
        self.transaction_begin().await?;
        let result = async {
            self.insert_event(&event).await?;
            self.connection
                .execute(
                    "UPDATE items SET project_id = ?1 WHERE id = ?2",
                    vec![
                        target_id.map_or(Value::Null, |id| Value::Text(id.to_string())),
                        Value::Text(item.item.id.to_string()),
                    ],
                )
                .await?;
            self.set_head(&event.cid).await
        }
        .await;
        self.transaction_finish(result).await?;
        self.get_item(&item.item.id.to_string()).await
    }

    pub async fn add_dependency(
        &self,
        item_reference: &str,
        blocker_reference: &str,
    ) -> Result<()> {
        let item = self.get_item(item_reference).await?;
        let blocker = self.get_item(blocker_reference).await?;
        if item.item.id == blocker.item.id {
            bail!("an item cannot block itself");
        }
        if item.item.project_id != blocker.item.project_id {
            bail!("an item and its blocker must be in the same project scope");
        }
        if item.dependencies.contains(&blocker.item.id) {
            return Ok(());
        }
        if self
            .would_create_dependency_cycle(item.item.id, blocker.item.id)
            .await?
        {
            bail!("blocking relationship would create a dependency cycle");
        }
        let event = self
            .event(
                EventBuilder::for_item(EventKind::ItemDependencyAdded, item.item.id)
                    .project(item.item.project_id)
                    .link("depends-on", blocker.item.id.to_string()),
            )
            .await?;
        self.transaction_begin().await?;
        let result = async {
            self.insert_event(&event).await?;
            self.connection
                .execute(
                    "INSERT INTO item_dependencies(item_id, blocker_id) VALUES (?1, ?2)",
                    values([item.item.id.to_string(), blocker.item.id.to_string()]),
                )
                .await?;
            self.set_head(&event.cid).await
        }
        .await;
        self.transaction_finish(result).await
    }

    pub async fn remove_dependency(
        &self,
        item_reference: &str,
        blocker_reference: &str,
    ) -> Result<()> {
        let item = self.get_item(item_reference).await?;
        let blocker = self.get_item(blocker_reference).await?;
        if !item.dependencies.contains(&blocker.item.id) {
            bail!("{} does not depend on {}", item.item.id, blocker.item.id);
        }
        let event = self
            .event(
                EventBuilder::for_item(EventKind::ItemDependencyRemoved, item.item.id)
                    .project(item.item.project_id)
                    .link("no-longer-depends-on", blocker.item.id.to_string()),
            )
            .await?;
        self.transaction_begin().await?;
        let result = async {
            self.insert_event(&event).await?;
            self.connection
                .execute(
                    "DELETE FROM item_dependencies WHERE item_id = ?1 AND blocker_id = ?2",
                    values([item.item.id.to_string(), blocker.item.id.to_string()]),
                )
                .await?;
            self.set_head(&event.cid).await
        }
        .await;
        self.transaction_finish(result).await
    }

    /// Add a current impediment. Unlike a planned dependency, this stays active
    /// until explicitly removed, even if the referenced task is completed.
    pub async fn add_blocker(&self, item_reference: &str, blocker_reference: &str) -> Result<()> {
        let item = self.get_item(item_reference).await?;
        let blocker = self.get_item(blocker_reference).await?;
        if item.item.id == blocker.item.id {
            bail!("an item cannot block itself");
        }
        if item.item.project_id != blocker.item.project_id {
            bail!("an item and its blocker must be in the same project scope");
        }
        if item.blocked_by.contains(&blocker.item.id) {
            return Ok(());
        }
        let event = self
            .event(
                EventBuilder::for_item(EventKind::ItemBlockAdded, item.item.id)
                    .project(item.item.project_id)
                    .link("blocked-by", blocker.item.id.to_string()),
            )
            .await?;
        self.transaction_begin().await?;
        let result = async {
            self.insert_event(&event).await?;
            self.connection
                .execute(
                    "INSERT INTO item_blocks(item_id, blocker_id) VALUES (?1, ?2)",
                    values([item.item.id.to_string(), blocker.item.id.to_string()]),
                )
                .await?;
            self.set_head(&event.cid).await
        }
        .await;
        self.transaction_finish(result).await
    }

    pub async fn remove_blocker(
        &self,
        item_reference: &str,
        blocker_reference: &str,
    ) -> Result<()> {
        let item = self.get_item(item_reference).await?;
        let blocker = self.get_item(blocker_reference).await?;
        if !item.blocked_by.contains(&blocker.item.id) {
            bail!(
                "{} is not actively blocked by {}",
                item.item.id,
                blocker.item.id
            );
        }
        let event = self
            .event(
                EventBuilder::for_item(EventKind::ItemBlockRemoved, item.item.id)
                    .project(item.item.project_id)
                    .link("no-longer-blocked-by", blocker.item.id.to_string()),
            )
            .await?;
        self.transaction_begin().await?;
        let result = async {
            self.insert_event(&event).await?;
            self.connection
                .execute(
                    "DELETE FROM item_blocks WHERE item_id = ?1 AND blocker_id = ?2",
                    values([item.item.id.to_string(), blocker.item.id.to_string()]),
                )
                .await?;
            self.set_head(&event.cid).await
        }
        .await;
        self.transaction_finish(result).await
    }

    pub async fn assign_namespace(&self, item_reference: &str, path: &NamespacePath) -> Result<()> {
        let item = self.get_item(item_reference).await?;
        let namespace = self.ensure_namespace(path).await?;
        let event = self
            .event(
                EventBuilder::for_item(EventKind::NamespaceAssigned, item.item.id)
                    .project(item.item.project_id)
                    .link("namespace", namespace.id.to_string()),
            )
            .await?;
        self.transaction_begin().await?;
        let result = async {
            self.insert_event(&event).await?;
            self.connection
                .execute(
                    "INSERT OR IGNORE INTO item_namespaces(item_id, namespace_id) VALUES (?1, ?2)",
                    values([item.item.id.to_string(), namespace.id.to_string()]),
                )
                .await?;
            self.set_head(&event.cid).await
        }
        .await;
        self.transaction_finish(result).await
    }

    pub async fn remove_namespace(&self, item_reference: &str, path: &NamespacePath) -> Result<()> {
        let item = self.get_item(item_reference).await?;
        let namespace = self
            .namespace_by_path(path)
            .await?
            .ok_or_else(|| anyhow!("unknown namespace {path}"))?;
        if !item
            .namespaces
            .iter()
            .any(|assigned| assigned == &namespace.path)
        {
            bail!(
                "item {} does not have namespace {}",
                item.item.id,
                namespace.path
            );
        }
        let event = self
            .event(
                EventBuilder::for_item(EventKind::NamespaceRemoved, item.item.id)
                    .project(item.item.project_id)
                    .link("namespace", namespace.id.to_string()),
            )
            .await?;
        self.transaction_begin().await?;
        let result = async {
            self.insert_event(&event).await?;
            self.connection
                .execute(
                    "DELETE FROM item_namespaces WHERE item_id = ?1 AND namespace_id = ?2",
                    values([item.item.id.to_string(), namespace.id.to_string()]),
                )
                .await?;
            self.set_head(&event.cid).await
        }
        .await;
        self.transaction_finish(result).await
    }

    pub async fn list_items(
        &self,
        scope: ItemScope<'_>,
        all_states: bool,
    ) -> Result<Vec<StoredItem>> {
        let project_id = match scope {
            ItemScope::Project(path) => match self.namespace_by_path(path).await? {
                Some(entry) => Some(entry.id),
                None => return Ok(Vec::new()),
            },
            ItemScope::All | ItemScope::Unscoped => None,
        };
        let (sql, params) = match (scope, project_id, all_states) {
            (ItemScope::Project(_), Some(id), true) => (
                "SELECT id, title, state, project_id, parent_id, priority, due_on, defer_until, scheduled_on, difficulty, estimate_minutes FROM items WHERE deleted_at IS NULL AND state != 'void' AND project_id = ?1 ORDER BY id",
                values([id.to_string()]),
            ),
            (ItemScope::Project(_), Some(id), false) => (
                "SELECT id, title, state, project_id, parent_id, priority, due_on, defer_until, scheduled_on, difficulty, estimate_minutes FROM items WHERE deleted_at IS NULL AND project_id = ?1 AND state NOT IN ('done', 'canceled', 'void') ORDER BY id",
                values([id.to_string()]),
            ),
            (ItemScope::Unscoped, None, true) => (
                "SELECT id, title, state, project_id, parent_id, priority, due_on, defer_until, scheduled_on, difficulty, estimate_minutes FROM items WHERE deleted_at IS NULL AND state != 'void' AND project_id IS NULL ORDER BY id",
                Vec::<Value>::new(),
            ),
            (ItemScope::Unscoped, None, false) => (
                "SELECT id, title, state, project_id, parent_id, priority, due_on, defer_until, scheduled_on, difficulty, estimate_minutes FROM items WHERE deleted_at IS NULL AND project_id IS NULL AND state NOT IN ('done', 'canceled', 'void') ORDER BY id",
                Vec::<Value>::new(),
            ),
            (ItemScope::All, None, true) => (
                "SELECT id, title, state, project_id, parent_id, priority, due_on, defer_until, scheduled_on, difficulty, estimate_minutes FROM items WHERE deleted_at IS NULL AND state != 'void' ORDER BY id",
                Vec::<Value>::new(),
            ),
            (ItemScope::All, None, false) => (
                "SELECT id, title, state, project_id, parent_id, priority, due_on, defer_until, scheduled_on, difficulty, estimate_minutes FROM items WHERE deleted_at IS NULL AND state NOT IN ('done', 'canceled', 'void') ORDER BY id",
                Vec::<Value>::new(),
            ),
            _ => bail!("invalid resolved item scope"),
        };
        let mut rows = self.connection.query(sql, params).await?;
        let mut items = Vec::new();
        while let Some(row) = rows.next().await? {
            let item = item_from_row(&row)?;
            let namespaces = self.item_namespaces(item.id).await?;
            let dependencies = self.item_dependencies(item.id).await?;
            let blocked_by = self.item_active_blockers(item.id).await?;
            let relations = self.item_relations(item.id).await?;
            let agent = self.item_agent(item.id).await?;
            items.push(StoredItem {
                item,
                namespaces,
                dependencies,
                blocked_by,
                relations,
                agent,
            });
        }
        Ok(items)
    }

    pub async fn list_ready(
        &self,
        scope: ItemScope<'_>,
        today: NaiveDate,
    ) -> Result<Vec<StoredItem>> {
        let candidates = self.list_items(scope, false).await?;
        let mut ready = Vec::new();
        for item in candidates {
            if item.item.defer_until.is_some_and(|date| date > today) {
                continue;
            }
            if !self.has_incomplete_dependency(item.item.id).await?
                && !self.has_active_blocker(item.item.id).await?
            {
                ready.push(item);
            }
        }
        Ok(ready)
    }

    /// The highest-ranked ready item among those `keep` accepts.
    pub async fn next_item(
        &self,
        scope: ItemScope<'_>,
        today: NaiveDate,
        keep: impl Fn(&StoredItem) -> bool,
    ) -> Result<Option<NextItem>> {
        let mut ready = self.list_ready(scope, today).await?;
        ready.retain(|item| keep(item));
        ready.sort_by(compare_focus);
        let Some(item) = ready.into_iter().next() else {
            return Ok(None);
        };
        let mut reasons = vec![
            "ready: no incomplete dependencies, no active blockers, and defer date reached"
                .to_owned(),
            format!("state: {}", item.item.state),
            format!("priority: {}", item.item.priority),
        ];
        reasons.push(match item.item.due_on {
            Some(date) => format!("due: {date}"),
            None => "due: none".to_owned(),
        });
        Ok(Some(NextItem { item, reasons }))
    }

    pub async fn get_item(&self, reference: &str) -> Result<StoredItem> {
        let reference = reference.trim();
        // An identifier printed as proquints must be accepted back: it is decoded to
        // the hexadecimal it stands for, and anything that is not a valid quint is
        // matched as given.
        let decoded = todo_core::proquint_to_hex(reference);
        let matched = decoded.as_deref().unwrap_or(reference);
        let pattern = format!("%{matched}");
        let mut rows = self
            .connection
            .query(
                "SELECT id, title, state, project_id, parent_id, priority, due_on, defer_until, scheduled_on, difficulty, estimate_minutes FROM items WHERE deleted_at IS NULL AND (id = ?1 OR id LIKE ?2) ORDER BY id LIMIT 2",
                values([matched.to_owned(), pattern]),
            )
            .await?;
        let first = rows
            .next()
            .await?
            .ok_or_else(|| anyhow!("unknown item {reference}"))?;
        let item = item_from_row(&first)?;
        if rows.next().await?.is_some() {
            bail!("item reference {reference} is ambiguous; use the full ID");
        }
        let namespaces = self.item_namespaces(item.id).await?;
        let dependencies = self.item_dependencies(item.id).await?;
        let blocked_by = self.item_active_blockers(item.id).await?;
        let relations = self.item_relations(item.id).await?;
        let agent = self.item_agent(item.id).await?;
        Ok(StoredItem {
            item,
            namespaces,
            dependencies,
            blocked_by,
            relations,
            agent,
        })
    }

    pub async fn notes(&self, item: ItemId) -> Result<Vec<Note>> {
        let mut rows = self
            .connection
            .query(
                "SELECT body, recorded_at FROM notes WHERE item_id = ?1 ORDER BY recorded_at",
                values([item.to_string()]),
            )
            .await?;
        let mut result = Vec::new();
        while let Some(row) = rows.next().await? {
            result.push(Note {
                body: text(&row, 0)?,
                recorded_at: text(&row, 1)?,
            });
        }
        Ok(result)
    }

    pub async fn citations(&self, item: ItemId) -> Result<Vec<Citation>> {
        let mut rows = self.connection.query(
            "SELECT target, label, recorded_at FROM citations WHERE item_id = ?1 ORDER BY recorded_at",
            values([item.to_string()]),
        ).await?;
        let mut result = Vec::new();
        while let Some(row) = rows.next().await? {
            result.push(Citation {
                target: text(&row, 0)?,
                label: optional_text(&row, 1)?,
                recorded_at: text(&row, 2)?,
            });
        }
        Ok(result)
    }

    pub async fn create_focus_sequence(&self, title: &str) -> Result<FocusSequence> {
        let title = title.trim();
        if title.is_empty() {
            bail!("focus sequence title cannot be empty");
        }
        let id = Uuid::now_v7().to_string();
        let event = self
            .event(EventBuilder::new(EventKind::FocusSequenceCreated, &id).data("title", title))
            .await?;
        self.transaction_begin().await?;
        let result = async {
            self.insert_event(&event).await?;
            self.connection
                .execute(
                    "INSERT INTO focus_sequences(id, title) VALUES (?1, ?2)",
                    values([id.clone(), title.to_owned()]),
                )
                .await?;
            self.set_head(&event.cid).await
        }
        .await;
        self.transaction_finish(result).await?;
        Ok(FocusSequence {
            id,
            title: title.to_owned(),
            items: Vec::new(),
        })
    }

    pub async fn add_focus_sequence_item(
        &self,
        sequence_id: &str,
        item_reference: &str,
    ) -> Result<()> {
        let item = self.get_item(item_reference).await?;
        let position = self.next_focus_sequence_position(sequence_id).await?;
        let event = self
            .event(
                EventBuilder::new(EventKind::FocusSequenceEntryAdded, sequence_id)
                    .data("position", position.to_string())
                    .link("item", item.item.id.to_string()),
            )
            .await?;
        self.transaction_begin().await?;
        let result = async {
            self.insert_event(&event).await?;
            self.connection.execute("INSERT INTO focus_sequence_entries(sequence_id, position, item_id) VALUES (?1, ?2, ?3)", values([sequence_id.to_owned(), position.to_string(), item.item.id.to_string()])).await?;
            self.set_head(&event.cid).await
        }.await;
        self.transaction_finish(result).await
    }

    pub async fn focus_sequences(&self) -> Result<Vec<FocusSequence>> {
        let mut rows = self
            .connection
            .query("SELECT id, title FROM focus_sequences ORDER BY title", ())
            .await?;
        let mut sequences = Vec::new();
        while let Some(row) = rows.next().await? {
            let id = text(&row, 0)?;
            let title = text(&row, 1)?;
            let mut item_rows = self.connection.query("SELECT item_id FROM focus_sequence_entries WHERE sequence_id = ?1 ORDER BY position", values([id.clone()])).await?;
            let mut items = Vec::new();
            while let Some(item_row) = item_rows.next().await? {
                items.push(ItemId::parse(&text(&item_row, 0)?)?);
            }
            sequences.push(FocusSequence { id, title, items });
        }
        Ok(sequences)
    }

    pub async fn start_session(
        &self,
        reference: &str,
        started_at: DateTime<Utc>,
    ) -> Result<WorkSession> {
        let item = self.get_item(reference).await?;
        if self.active_session(item.item.id).await?.is_some() {
            bail!("item already has an active session");
        }
        let id = Uuid::now_v7().to_string();
        let started_at = started_at.to_rfc3339();
        let event = self
            .event(
                EventBuilder::for_item(EventKind::ItemSessionStarted, item.item.id)
                    .project(item.item.project_id)
                    .data("session_id", &id)
                    .data("started_at", &started_at),
            )
            .await?;
        self.transaction_begin().await?;
        let result = async {
            self.insert_event(&event).await?;
            self.connection.execute("INSERT INTO item_sessions(session_id, item_id, started_at, finished_at) VALUES (?1, ?2, ?3, NULL)", values([id.clone(), item.item.id.to_string(), started_at.clone()])).await?;
            self.set_head(&event.cid).await
        }.await;
        self.transaction_finish(result).await?;
        Ok(WorkSession {
            id,
            item_id: item.item.id,
            started_at,
            finished_at: None,
        })
    }

    pub async fn finish_session(
        &self,
        reference: &str,
        finished_at: DateTime<Utc>,
    ) -> Result<WorkSession> {
        let item = self.get_item(reference).await?;
        let session = self
            .active_session(item.item.id)
            .await?
            .ok_or_else(|| anyhow!("item has no active session"))?;
        let started_at = DateTime::parse_from_rfc3339(&session.started_at)?.to_utc();
        if finished_at < started_at {
            bail!("session finish cannot precede its start");
        }
        let finished_at = finished_at.to_rfc3339();
        let event = self
            .event(
                EventBuilder::for_item(EventKind::ItemSessionFinished, item.item.id)
                    .project(item.item.project_id)
                    .data("session_id", &session.id)
                    .data("finished_at", &finished_at),
            )
            .await?;
        self.transaction_begin().await?;
        let result = async {
            self.insert_event(&event).await?;
            self.connection
                .execute(
                    "UPDATE item_sessions SET finished_at = ?1 WHERE session_id = ?2",
                    values([finished_at.clone(), session.id.clone()]),
                )
                .await?;
            self.set_head(&event.cid).await
        }
        .await;
        self.transaction_finish(result).await?;
        Ok(WorkSession {
            finished_at: Some(finished_at),
            ..session
        })
    }

    pub async fn sessions(&self, item: ItemId) -> Result<Vec<WorkSession>> {
        let mut rows = self.connection.query("SELECT session_id, started_at, finished_at FROM item_sessions WHERE item_id = ?1 ORDER BY started_at", values([item.to_string()])).await?;
        let mut sessions = Vec::new();
        while let Some(row) = rows.next().await? {
            sessions.push(WorkSession {
                id: text(&row, 0)?,
                item_id: item,
                started_at: text(&row, 1)?,
                finished_at: optional_text(&row, 2)?,
            });
        }
        Ok(sessions)
    }

    pub async fn history(&self, item: ItemId) -> Result<Vec<HistoryEntry>> {
        let mut rows = self.connection.query(
            "SELECT cid, kind, recorded_at, envelope FROM events WHERE subject_id = ?1 ORDER BY recorded_at, event_id",
            values([item.to_string()]),
        ).await?;
        let mut result = Vec::new();
        while let Some(row) = rows.next().await? {
            // Only v5+ envelopes decode here; older ones never carry an agent.
            let agent = match row.get_value(3)? {
                Value::Blob(bytes) => decode_envelope(&bytes)
                    .ok()
                    .and_then(|event| event.payload.data.get(AGENT_FIELD).cloned()),
                _ => None,
            };
            result.push(HistoryEntry {
                cid: text(&row, 0)?,
                kind: text(&row, 1)?,
                recorded_at: text(&row, 2)?,
                agent,
            });
        }
        Ok(result)
    }

    /// The signing agent of the item's creation, or else an author actor whose
    /// `kind` is `agent`, which is how items from before v15 are labeled.
    async fn item_agent(&self, item: ItemId) -> Result<Option<String>> {
        let mut rows = self
            .connection
            .query(
                "SELECT COALESCE(i.created_by_agent, (SELECT substr(n.canonical_path, 7) FROM item_roles r JOIN namespaces n ON n.id = r.namespace_id JOIN namespace_fields f ON f.namespace_id = n.id AND f.key = 'kind' AND f.value = 'agent' WHERE r.item_id = i.id AND r.role = 'author' ORDER BY n.canonical_path LIMIT 1)) FROM items i WHERE i.id = ?1",
                values([item.to_string()]),
            )
            .await?;
        match rows.next().await? {
            Some(row) => optional_text(&row, 0),
            None => Ok(None),
        }
    }

    pub async fn event_records(&self) -> Result<Vec<EventRecord>> {
        let mut rows = self
            .connection
            .query(
                "SELECT cid, envelope FROM events ORDER BY recorded_at, event_id",
                (),
            )
            .await?;
        let mut result = Vec::new();
        while let Some(row) = rows.next().await? {
            match row.get_value(1)? {
                Value::Blob(bytes) => result.push(EventRecord {
                    cid: Cid::from_str(&text(&row, 0)?)?,
                    bytes,
                }),
                _ => bail!("event envelope column is not a blob"),
            }
        }
        Ok(result)
    }

    pub async fn event_envelopes(&self) -> Result<Vec<Vec<u8>>> {
        Ok(self
            .event_records()
            .await?
            .into_iter()
            .map(|record| record.bytes)
            .collect())
    }

    /// Admit v5 and v6 events exactly as received, then derive all views from the
    /// canonical event order. This is the boundary used by local interchange
    /// and later transport adapters.
    pub async fn import_events(&self, records: &[EventRecord]) -> Result<usize> {
        let mut events = Vec::new();
        for record in records {
            if record.cid.codec() != GRID_CBOR_CODEC {
                bail!("event {} is not replayable by the v5 ledger", record.cid);
            }
            let envelope = GridEnvelope::parse(&record.bytes).context("validate Grid envelope")?;
            if envelope.protocol_cid() != &event_protocol_v5_cid()
                && envelope.protocol_cid() != &event_protocol_v6_cid()
                && envelope.protocol_cid() != &event_protocol_v7_cid()
                && envelope.protocol_cid() != &event_protocol_v8_cid()
                && envelope.protocol_cid() != &event_protocol_v9_cid()
                && envelope.protocol_cid() != &event_protocol_v10_cid()
                && envelope.protocol_cid() != &event_protocol_v11_cid()
                && envelope.protocol_cid() != &event_protocol_v12_cid()
                && envelope.protocol_cid() != &event_protocol_v13_cid()
                && envelope.protocol_cid() != &event_protocol_v14_cid()
                && envelope.protocol_cid() != &event_protocol_v15_cid()
            {
                bail!("event {} uses an unknown protocol", record.cid);
            }
            let event = decode_envelope(&record.bytes).context("decode ledger event")?;
            if event.cid != record.cid {
                bail!("event bytes do not match declared CID {}", record.cid);
            }
            events.push(event);
        }
        events.sort_by(|left, right| {
            left.payload
                .recorded_at
                .cmp(&right.payload.recorded_at)
                .then_with(|| left.payload.event_id.cmp(&right.payload.event_id))
        });
        self.transaction_begin().await?;
        let mut inserted = 0;
        let result = async {
            for event in &events {
                if !self.event_exists(&event.cid).await? {
                    self.insert_event(event).await?;
                    inserted += 1;
                }
            }
            self.rebuild_views_in_transaction().await?;
            self.connection
                .execute(
                    "INSERT INTO meta(key, value) VALUES ('schema_version', '2') ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                    (),
                )
                .await?;
            Ok(())
        }
        .await;
        self.transaction_finish(result).await?;
        Ok(inserted)
    }

    pub async fn rebuild_views(&self) -> Result<()> {
        self.transaction_begin().await?;
        let result = self.rebuild_views_in_transaction().await;
        self.transaction_finish(result).await
    }

    pub async fn namespaces(&self) -> Result<Vec<NamespaceEntry>> {
        let mut rows = self
            .connection
            .query(
                "SELECT id, kind, canonical_path FROM namespaces ORDER BY canonical_path",
                (),
            )
            .await?;
        let mut result = Vec::new();
        while let Some(row) = rows.next().await? {
            result.push(NamespaceEntry {
                id: NamespaceId::parse(&text(&row, 0)?)?,
                kind: text(&row, 1)?.parse()?,
                path: text(&row, 2)?.parse()?,
            });
        }
        Ok(result)
    }

    async fn item_namespaces(&self, item: ItemId) -> Result<Vec<NamespacePath>> {
        let mut rows = self.connection.query(
            "SELECT n.canonical_path FROM namespaces n JOIN item_namespaces i ON i.namespace_id = n.id WHERE i.item_id = ?1 ORDER BY n.canonical_path",
            values([item.to_string()]),
        ).await?;
        let mut result = Vec::new();
        while let Some(row) = rows.next().await? {
            result.push(text(&row, 0)?.parse()?);
        }
        Ok(result)
    }

    async fn item_dependencies(&self, item: ItemId) -> Result<Vec<ItemId>> {
        let mut rows = self
            .connection
            .query(
                "SELECT blocker_id FROM item_dependencies WHERE item_id = ?1 ORDER BY blocker_id",
                values([item.to_string()]),
            )
            .await?;
        let mut result = Vec::new();
        while let Some(row) = rows.next().await? {
            result.push(ItemId::parse(&text(&row, 0)?)?);
        }
        Ok(result)
    }

    /// Returns an item's typed relations, ordered for stable output.
    async fn item_relations(&self, item: ItemId) -> Result<Vec<ItemRelation>> {
        let mut rows = self
            .connection
            .query(
                "SELECT relation, target_id FROM item_relations WHERE item_id = ?1 ORDER BY relation, target_id",
                values([item.to_string()]),
            )
            .await?;
        let mut result = Vec::new();
        while let Some(row) = rows.next().await? {
            result.push(ItemRelation {
                relation: text(&row, 0)?.parse()?,
                target: ItemId::parse(&text(&row, 1)?)?,
            });
        }
        Ok(result)
    }

    /// Returns the items that hold a typed relation pointing at this one.
    ///
    /// The inverse direction is derived on read rather than stored twice, exactly as
    /// the dependency and blocker views are.
    pub async fn related_from(&self, reference: &str) -> Result<Vec<(RelationKind, ItemId)>> {
        let target = self.get_item(reference).await?;
        let mut rows = self
            .connection
            .query(
                "SELECT relation, item_id FROM item_relations WHERE target_id = ?1 ORDER BY relation, item_id",
                values([target.item.id.to_string()]),
            )
            .await?;
        let mut result = Vec::new();
        while let Some(row) = rows.next().await? {
            result.push((text(&row, 0)?.parse()?, ItemId::parse(&text(&row, 1)?)?));
        }
        Ok(result)
    }

    /// Adds a typed relation from one item to another, across projects if needed.
    ///
    /// Relations are deliberately unconstrained by project: an item in one project
    /// may contribute to or implement work in another. Nothing about readiness
    /// changes, and a duplicate relation is a no-op rather than an error.
    pub async fn add_relation(
        &self,
        reference: &str,
        relation: RelationKind,
        target_reference: &str,
    ) -> Result<StoredItem> {
        let item = self.get_item(reference).await?;
        let target = self.get_item(target_reference).await?;
        if item.item.id == target.item.id {
            bail!("an item cannot relate to itself");
        }
        let holds = |from: &StoredItem, to: ItemId| {
            from.relations
                .iter()
                .any(|existing| existing.relation == relation && existing.target == to)
        };
        // A symmetric relation already stored the other way round is the same
        // relation, so it is never written twice. Source: protocol v14.
        if holds(&item, target.item.id) || relation.is_symmetric() && holds(&target, item.item.id) {
            return Ok(item);
        }
        let event = self
            .event(
                EventBuilder::for_item(EventKind::ItemRelationAdded, item.item.id)
                    .project(item.item.project_id)
                    .data("relation", relation.to_string())
                    .link("target", target.item.id.to_string()),
            )
            .await?;
        self.transaction_begin().await?;
        let result = async {
            self.insert_event(&event).await?;
            self.connection
                .execute(
                    "INSERT OR IGNORE INTO item_relations(item_id, relation, target_id) VALUES (?1, ?2, ?3)",
                    values([
                        item.item.id.to_string(),
                        relation.to_string(),
                        target.item.id.to_string(),
                    ]),
                )
                .await?;
            self.set_head(&event.cid).await
        }
        .await;
        self.transaction_finish(result).await?;
        self.get_item(&item.item.id.to_string()).await
    }

    /// Withdraws a typed relation.
    pub async fn remove_relation(
        &self,
        reference: &str,
        relation: RelationKind,
        target_reference: &str,
    ) -> Result<StoredItem> {
        let mut item = self.get_item(reference).await?;
        let mut target = self.get_item(target_reference).await?;
        let holds = |from: &StoredItem, to: ItemId| {
            from.relations
                .iter()
                .any(|existing| existing.relation == relation && existing.target == to)
        };
        // A symmetric relation may have been written from the other end.
        if relation.is_symmetric() && !holds(&item, target.item.id) && holds(&target, item.item.id)
        {
            std::mem::swap(&mut item, &mut target);
        }
        if !holds(&item, target.item.id) {
            bail!(
                "item {} does not {relation} {}",
                item.item.id,
                target.item.id
            );
        }
        let event = self
            .event(
                EventBuilder::for_item(EventKind::ItemRelationRemoved, item.item.id)
                    .project(item.item.project_id)
                    .data("relation", relation.to_string())
                    .link("target", target.item.id.to_string()),
            )
            .await?;
        self.transaction_begin().await?;
        let result = async {
            self.insert_event(&event).await?;
            self.connection
                .execute(
                    "DELETE FROM item_relations WHERE item_id = ?1 AND relation = ?2 AND target_id = ?3",
                    values([
                        item.item.id.to_string(),
                        relation.to_string(),
                        target.item.id.to_string(),
                    ]),
                )
                .await?;
            self.set_head(&event.cid).await
        }
        .await;
        self.transaction_finish(result).await?;
        self.get_item(reference).await
    }

    /// Sets one field on a namespace node, or clears it with `None`.
    ///
    /// Setting creates the node on first use, so `todo set quincy email=…` needs no
    /// separate create step. Writing the value already held is a no-op.
    pub async fn set_field(
        &self,
        path: &NamespacePath,
        key: &str,
        value: Option<&str>,
    ) -> Result<()> {
        validate_segment(key).with_context(|| format!("field key {key:?}"))?;
        let value = value.unwrap_or("");
        let namespace = match self.namespace_by_path(path).await? {
            Some(namespace) => namespace,
            None if value.is_empty() => bail!("unknown namespace {path}"),
            None => self.ensure_namespace(path).await?,
        };
        let current = self.fields(path).await?;
        if current.get(key).map_or("", String::as_str) == value {
            return Ok(());
        }
        self.record(
            EventBuilder::new(EventKind::NamespaceFieldSet, namespace.id.to_string())
                .data("key", key)
                .data("value", value),
        )
        .await
    }

    /// Returns a namespace node's fields, empty for an unknown node.
    pub async fn fields(&self, path: &NamespacePath) -> Result<BTreeMap<String, String>> {
        let mut rows = self.connection.query(
            "SELECT f.key, f.value FROM namespace_fields f JOIN namespaces n ON n.id = f.namespace_id LEFT JOIN namespace_aliases a ON a.namespace_id = n.id WHERE n.canonical_path = ?1 OR a.path = ?1",
            values([path.to_string()]),
        ).await?;
        let mut result = BTreeMap::new();
        while let Some(row) = rows.next().await? {
            result.insert(text(&row, 0)?, text(&row, 1)?);
        }
        Ok(result)
    }

    /// Returns the projects whose `repo` field names the same repository as
    /// `remote`, compared by [`repo_key`] so SSH and HTTPS forms agree.
    pub async fn projects_for_repo(&self, remote: &str) -> Result<Vec<NamespacePath>> {
        let wanted = repo_key(remote);
        let mut rows = self
            .connection
            .query(
                "SELECT n.canonical_path, f.value FROM namespace_fields f JOIN namespaces n ON n.id = f.namespace_id WHERE f.key = 'repo' AND n.kind = 'project' ORDER BY n.canonical_path",
                (),
            )
            .await?;
        let mut result = Vec::new();
        while let Some(row) = rows.next().await? {
            if repo_key(&text(&row, 1)?) == wanted {
                result.push(text(&row, 0)?.parse()?);
            }
        }
        Ok(result)
    }

    /// Links one namespace to another, such as a person `member-of` an org.
    pub async fn link_namespace(
        &self,
        path: &NamespacePath,
        relation: &str,
        target: &NamespacePath,
    ) -> Result<()> {
        if relation != MEMBER_OF {
            bail!("unknown namespace relation {relation:?}; expected {MEMBER_OF}");
        }
        let namespace = self.ensure_namespace(path).await?;
        let target = self.ensure_namespace(target).await?;
        if namespace.id == target.id {
            bail!("a namespace cannot link to itself");
        }
        if self
            .namespace_links(&namespace.path)
            .await?
            .contains(&(relation.to_owned(), target.path.clone()))
        {
            return Ok(());
        }
        self.record(
            EventBuilder::new(EventKind::NamespaceLinked, namespace.id.to_string())
                .data("relation", relation)
                .link("target", target.id.to_string()),
        )
        .await
    }

    /// Withdraws a namespace link.
    pub async fn unlink_namespace(
        &self,
        path: &NamespacePath,
        relation: &str,
        target: &NamespacePath,
    ) -> Result<()> {
        let unknown = || anyhow!("{path} is not {relation} {target}");
        let namespace = self.namespace_by_path(path).await?.ok_or_else(unknown)?;
        let target = self.namespace_by_path(target).await?.ok_or_else(unknown)?;
        if !self
            .namespace_links(&namespace.path)
            .await?
            .contains(&(relation.to_owned(), target.path.clone()))
        {
            return Err(unknown());
        }
        self.record(
            EventBuilder::new(EventKind::NamespaceUnlinked, namespace.id.to_string())
                .data("relation", relation)
                .link("target", target.id.to_string()),
        )
        .await
    }

    /// Returns the links a namespace holds, as (relation, target).
    pub async fn namespace_links(
        &self,
        path: &NamespacePath,
    ) -> Result<Vec<(String, NamespacePath)>> {
        self.links_query(
            "SELECT l.relation, t.canonical_path FROM namespace_links l JOIN namespaces s ON s.id = l.namespace_id JOIN namespaces t ON t.id = l.target_id WHERE s.canonical_path = ?1 ORDER BY l.relation, t.canonical_path",
            path,
        )
        .await
    }

    /// Returns the links pointing at a namespace, as (relation, source): an org's
    /// members. Derived on read, never stored twice.
    pub async fn namespace_linked_from(
        &self,
        path: &NamespacePath,
    ) -> Result<Vec<(String, NamespacePath)>> {
        self.links_query(
            "SELECT l.relation, s.canonical_path FROM namespace_links l JOIN namespaces s ON s.id = l.namespace_id JOIN namespaces t ON t.id = l.target_id WHERE t.canonical_path = ?1 ORDER BY l.relation, s.canonical_path",
            path,
        )
        .await
    }

    async fn links_query(
        &self,
        sql: &str,
        path: &NamespacePath,
    ) -> Result<Vec<(String, NamespacePath)>> {
        let mut rows = self
            .connection
            .query(sql, values([path.to_string()]))
            .await?;
        let mut result = Vec::new();
        while let Some(row) = rows.next().await? {
            result.push((text(&row, 0)?, text(&row, 1)?.parse()?));
        }
        Ok(result)
    }

    /// Attaches an item to a namespace in a role, such as `author` of `actor/quincy`.
    ///
    /// Independent of plain tagging: the same namespace may be both a tag and a
    /// role holder on one item. A duplicate is a no-op.
    pub async fn assign_role(
        &self,
        item_reference: &str,
        role: &str,
        path: &NamespacePath,
    ) -> Result<()> {
        validate_segment(role).with_context(|| format!("role {role:?}"))?;
        let item = self.get_item(item_reference).await?;
        let namespace = self.ensure_namespace(path).await?;
        if self
            .roles(item.item.id)
            .await?
            .contains(&(role.to_owned(), namespace.path.clone()))
        {
            return Ok(());
        }
        self.record(
            EventBuilder::for_item(EventKind::NamespaceAssigned, item.item.id)
                .project(item.item.project_id)
                .data("role", role)
                .link("namespace", namespace.id.to_string()),
        )
        .await
    }

    /// Withdraws one role assignment.
    pub async fn remove_role(
        &self,
        item_reference: &str,
        role: &str,
        path: &NamespacePath,
    ) -> Result<()> {
        let item = self.get_item(item_reference).await?;
        let missing = || anyhow!("item {} has no {role} {path}", item.item.id);
        let namespace = self.namespace_by_path(path).await?.ok_or_else(missing)?;
        if !self
            .roles(item.item.id)
            .await?
            .contains(&(role.to_owned(), namespace.path.clone()))
        {
            return Err(missing());
        }
        self.record(
            EventBuilder::for_item(EventKind::NamespaceRemoved, item.item.id)
                .project(item.item.project_id)
                .data("role", role)
                .link("namespace", namespace.id.to_string()),
        )
        .await
    }

    /// Returns an item's roles, as (role, namespace).
    pub async fn roles(&self, item: ItemId) -> Result<Vec<(String, NamespacePath)>> {
        let mut rows = self.connection.query(
            "SELECT r.role, n.canonical_path FROM item_roles r JOIN namespaces n ON n.id = r.namespace_id WHERE r.item_id = ?1 ORDER BY r.role, n.canonical_path",
            values([item.to_string()]),
        ).await?;
        let mut result = Vec::new();
        while let Some(row) = rows.next().await? {
            result.push((text(&row, 0)?, text(&row, 1)?.parse()?));
        }
        Ok(result)
    }

    /// Returns the live items holding a role on a namespace, as (role, item): what
    /// an actor authored, was assigned, or reviewed.
    pub async fn role_items(&self, path: &NamespacePath) -> Result<Vec<(String, ItemId)>> {
        let mut rows = self.connection.query(
            "SELECT r.role, r.item_id FROM item_roles r JOIN namespaces n ON n.id = r.namespace_id JOIN items i ON i.id = r.item_id WHERE n.canonical_path = ?1 AND i.deleted_at IS NULL ORDER BY r.role, r.item_id",
            values([path.to_string()]),
        ).await?;
        let mut result = Vec::new();
        while let Some(row) = rows.next().await? {
            result.push((text(&row, 0)?, ItemId::parse(&text(&row, 1)?)?));
        }
        Ok(result)
    }

    /// Writes one event and applies it through the same code a rebuild replays,
    /// so a live write and a rebuilt view cannot disagree.
    async fn record(&self, builder: EventBuilder) -> Result<()> {
        let event = self.event(builder).await?;
        self.transaction_begin().await?;
        let result = async {
            self.insert_event(&event).await?;
            self.apply_event(&event).await?;
            self.set_head(&event.cid).await
        }
        .await;
        self.transaction_finish(result).await
    }

    async fn write_field(&self, namespace_id: &str, key: &str, value: &str) -> Result<u64> {
        if value.is_empty() {
            return Ok(self
                .connection
                .execute(
                    "DELETE FROM namespace_fields WHERE namespace_id = ?1 AND key = ?2",
                    values([namespace_id.to_owned(), key.to_owned()]),
                )
                .await?);
        }
        Ok(self.connection.execute(
            "INSERT INTO namespace_fields(namespace_id, key, value) VALUES (?1, ?2, ?3) ON CONFLICT(namespace_id, key) DO UPDATE SET value = excluded.value",
            values([namespace_id.to_owned(), key.to_owned(), value.to_owned()]),
        ).await?)
    }

    async fn item_active_blockers(&self, item: ItemId) -> Result<Vec<ItemId>> {
        let mut rows = self
            .connection
            .query(
                "SELECT blocker_id FROM item_blocks WHERE item_id = ?1 ORDER BY blocker_id",
                values([item.to_string()]),
            )
            .await?;
        let mut result = Vec::new();
        while let Some(row) = rows.next().await? {
            result.push(ItemId::parse(&text(&row, 0)?)?);
        }
        Ok(result)
    }

    async fn next_focus_sequence_position(&self, sequence_id: &str) -> Result<i64> {
        let mut rows = self.connection.query(
            "SELECT COALESCE(MAX(position), -1) + 1 FROM focus_sequence_entries WHERE sequence_id = ?1",
            values([sequence_id.to_owned()]),
        ).await?;
        let row = rows
            .next()
            .await?
            .ok_or_else(|| anyhow!("cannot allocate focus sequence position"))?;
        match row.get_value(0)? {
            Value::Integer(position) => Ok(position),
            _ => bail!("focus sequence position is not an integer"),
        }
    }

    async fn active_session(&self, item: ItemId) -> Result<Option<WorkSession>> {
        let mut rows = self.connection.query("SELECT session_id, started_at FROM item_sessions WHERE item_id = ?1 AND finished_at IS NULL LIMIT 1", values([item.to_string()])).await?;
        let Some(row) = rows.next().await? else {
            return Ok(None);
        };
        Ok(Some(WorkSession {
            id: text(&row, 0)?,
            item_id: item,
            started_at: text(&row, 1)?,
            finished_at: None,
        }))
    }

    /// Looks a namespace up by its current path or any former one.
    pub async fn namespace_by_path(&self, path: &NamespacePath) -> Result<Option<NamespaceEntry>> {
        let mut rows = self.connection.query(
            "SELECT n.id, n.kind, n.canonical_path FROM namespaces n LEFT JOIN namespace_aliases a ON a.namespace_id = n.id WHERE n.canonical_path = ?1 OR a.path = ?1 LIMIT 1",
            values([path.to_string()]),
        ).await?;
        let Some(row) = rows.next().await? else {
            return Ok(None);
        };
        Ok(Some(NamespaceEntry {
            id: NamespaceId::parse(&text(&row, 0)?)?,
            kind: text(&row, 1)?.parse()?,
            path: text(&row, 2)?.parse()?,
        }))
    }

    async fn would_create_cycle(&self, child: ItemId, mut candidate: ItemId) -> Result<bool> {
        loop {
            if candidate == child {
                return Ok(true);
            }
            let mut rows = self
                .connection
                .query(
                    "SELECT parent_id FROM items WHERE id = ?1",
                    values([candidate.to_string()]),
                )
                .await?;
            let Some(row) = rows.next().await? else {
                return Ok(false);
            };
            let Some(parent) = optional_text(&row, 0)? else {
                return Ok(false);
            };
            candidate = ItemId::parse(&parent)?;
        }
    }

    async fn would_create_dependency_cycle(&self, item: ItemId, blocker: ItemId) -> Result<bool> {
        let mut pending = vec![blocker];
        let mut visited = HashSet::new();
        while let Some(candidate) = pending.pop() {
            if candidate == item {
                return Ok(true);
            }
            if !visited.insert(candidate) {
                continue;
            }
            let mut rows = self
                .connection
                .query(
                    "SELECT blocker_id FROM item_dependencies WHERE item_id = ?1",
                    values([candidate.to_string()]),
                )
                .await?;
            while let Some(row) = rows.next().await? {
                pending.push(ItemId::parse(&text(&row, 0)?)?);
            }
        }
        Ok(false)
    }

    async fn has_incomplete_dependency(&self, item: ItemId) -> Result<bool> {
        let mut rows = self
            .connection
            .query(
                "SELECT 1 FROM item_dependencies d JOIN items b ON b.id = d.blocker_id WHERE d.item_id = ?1 AND b.state NOT IN ('done', 'canceled') LIMIT 1",
                values([item.to_string()]),
            )
            .await?;
        Ok(rows.next().await?.is_some())
    }

    async fn has_active_blocker(&self, item: ItemId) -> Result<bool> {
        let mut rows = self
            .connection
            .query(
                "SELECT 1 FROM item_blocks WHERE item_id = ?1 LIMIT 1",
                values([item.to_string()]),
            )
            .await?;
        Ok(rows.next().await?.is_some())
    }

    async fn has_children(&self, item: ItemId) -> Result<bool> {
        let mut rows = self
            .connection
            .query(
                "SELECT 1 FROM items WHERE parent_id = ?1 LIMIT 1",
                values([item.to_string()]),
            )
            .await?;
        Ok(rows.next().await?.is_some())
    }

    async fn has_dependents(&self, item: ItemId) -> Result<bool> {
        let mut rows = self
            .connection
            .query(
                "SELECT 1 FROM item_dependencies WHERE blocker_id = ?1 UNION SELECT 1 FROM item_blocks WHERE blocker_id = ?1 LIMIT 1",
                values([item.to_string()]),
            )
            .await?;
        Ok(rows.next().await?.is_some())
    }

    async fn event(&self, builder: EventBuilder) -> Result<Event> {
        let parent = self.head().await?;
        self.sign(builder.parent(parent))
    }

    /// Stamps this ledger's actor and any agent, then builds the event.
    fn sign(&self, builder: EventBuilder) -> Result<Event> {
        let builder = match &self.agent {
            Some(agent) => builder.data(AGENT_FIELD, agent),
            None => builder,
        };
        builder.actor(&self.actor_id).build().context("build event")
    }

    async fn head(&self) -> Result<Option<Cid>> {
        let mut rows = self
            .connection
            .query("SELECT value FROM meta WHERE key = 'head'", ())
            .await?;
        rows.next()
            .await?
            .map(|row| Cid::from_str(&text(&row, 0)?).map_err(Into::into))
            .transpose()
    }

    async fn set_head(&self, cid: &Cid) -> Result<()> {
        self.connection.execute(
            "INSERT INTO meta(key, value) VALUES ('head', ?1) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            values([cid.to_string()]),
        ).await?;
        Ok(())
    }

    async fn insert_event(&self, event: &Event) -> Result<()> {
        self.connection.execute(
            "INSERT INTO events(cid, parent_cid, event_id, kind, actor_id, subject_id, project_id, recorded_at, envelope) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            vec![
                Value::Text(event.cid.to_string()),
                event.parent.as_ref().map_or(Value::Null, |cid| Value::Text(cid.to_string())),
                Value::Text(event.payload.event_id.to_string()),
                Value::Text(event.payload.kind.to_string()),
                Value::Text(event.payload.actor_id.clone()),
                Value::Text(event.payload.subject_id.clone()),
                event.payload.project_id.map_or(Value::Null, |id| Value::Text(id.to_string())),
                Value::Text(event.payload.recorded_at.to_rfc3339()),
                Value::Blob(event.bytes.clone()),
            ],
        ).await?;
        Ok(())
    }

    async fn event_exists(&self, cid: &Cid) -> Result<bool> {
        let mut rows = self
            .connection
            .query(
                "SELECT 1 FROM events WHERE cid = ?1 LIMIT 1",
                values([cid.to_string()]),
            )
            .await?;
        Ok(rows.next().await?.is_some())
    }

    async fn rebuild_views_in_transaction(&self) -> Result<()> {
        for table in [
            "notes",
            "citations",
            "item_namespaces",
            "item_roles",
            "item_relations",
            "namespace_fields",
            "namespace_links",
            "item_dependencies",
            "item_blocks",
            "focus_sequence_entries",
            "focus_sequences",
            "item_sessions",
            "namespace_aliases",
            "items",
            "namespaces",
        ] {
            self.connection
                .execute(&format!("DELETE FROM {table}"), ())
                .await?;
        }
        self.connection
            .execute("DELETE FROM meta WHERE key = 'head'", ())
            .await?;
        let records = self.event_records().await?;
        for record in records {
            if record.cid.codec() != GRID_CBOR_CODEC {
                bail!(
                    "cannot rebuild views containing legacy event {}",
                    record.cid
                );
            }
            let event = decode_envelope(&record.bytes)?;
            if event.cid != record.cid {
                bail!("stored event bytes do not match CID {}", record.cid);
            }
            self.apply_event(&event).await?;
            self.set_head(&event.cid).await?;
        }
        Ok(())
    }

    async fn apply_event(&self, event: &Event) -> Result<()> {
        let value = |name: &str| {
            event
                .payload
                .data
                .get(name)
                .map(String::as_str)
                .ok_or_else(|| anyhow!("event {} is missing data field {name}", event.cid))
        };
        let link = |relation: &str| {
            event
                .payload
                .links
                .iter()
                .find(|link| link.relation == relation)
                .map(|link| link.target.as_str())
                .ok_or_else(|| anyhow!("event {} is missing {relation} link", event.cid))
        };
        match &event.payload.kind {
            EventKind::NamespaceCreated => {
                let id = NamespaceId::parse(&event.payload.subject_id)?;
                let kind: NamespaceKind = value("kind")?.parse()?;
                let path: NamespacePath = value("path")?.parse()?;
                self.connection.execute(
                    "INSERT INTO namespaces(id, kind, canonical_path) VALUES (?1, ?2, ?3)",
                    values([id.to_string(), kind.to_string(), path.to_string()]),
                ).await?
            }
            EventKind::NamespaceRenamed => {
                let id = NamespaceId::parse(&event.payload.subject_id)?;
                self.connection.execute(
                    "INSERT INTO namespace_aliases(namespace_id, path) VALUES (?1, ?2)",
                    values([id.to_string(), value("from")?.to_owned()]),
                ).await?;
                self.connection.execute(
                    "UPDATE namespaces SET canonical_path = ?1 WHERE id = ?2",
                    values([value("to")?.to_owned(), id.to_string()]),
                ).await?
            }
            EventKind::ItemCreated => {
                let id = ItemId::parse(&event.payload.subject_id)?;
                let state: ItemState = value("state")?.parse()?;
                self.connection.execute(
                    "INSERT INTO items(id, title, state, project_id, parent_id, created_by_agent) VALUES (?1, ?2, ?3, ?4, NULL, ?5)",
                    vec![
                        Value::Text(id.to_string()),
                        Value::Text(value("title")?.to_owned()),
                        Value::Text(state.to_string()),
                        event.payload.project_id.map_or(Value::Null, |id| Value::Text(id.to_string())),
                        event.payload.data.get(AGENT_FIELD).map_or(Value::Null, |agent| Value::Text(agent.clone())),
                    ],
                ).await?
            }
            EventKind::ItemDeleted => self.connection.execute(
                "UPDATE items SET deleted_at = ?1 WHERE id = ?2",
                values([value("deleted_at")?.to_owned(), event.payload.subject_id.clone()]),
            ).await?,
            EventKind::ItemRelationAdded => self.connection.execute(
                "INSERT OR IGNORE INTO item_relations(item_id, relation, target_id) VALUES (?1, ?2, ?3)",
                values([event.payload.subject_id.clone(), value("relation")?.to_owned(), link("target")?.to_owned()]),
            ).await?,
            EventKind::ItemRelationRemoved => self.connection.execute(
                "DELETE FROM item_relations WHERE item_id = ?1 AND relation = ?2 AND target_id = ?3",
                values([event.payload.subject_id.clone(), value("relation")?.to_owned(), link("target")?.to_owned()]),
            ).await?,
            EventKind::ItemDifficultyChanged => self.connection.execute(
                "UPDATE items SET difficulty = ?1 WHERE id = ?2",
                vec![
                    parse_optional_number(value("difficulty")?)?,
                    Value::Text(event.payload.subject_id.clone()),
                ],
            ).await?,
            EventKind::ItemEstimateChanged => self.connection.execute(
                "UPDATE items SET estimate_minutes = ?1 WHERE id = ?2",
                vec![
                    parse_optional_number(value("estimate_minutes")?)?,
                    Value::Text(event.payload.subject_id.clone()),
                ],
            ).await?,
            EventKind::ItemRestored => self.connection.execute(
                "UPDATE items SET deleted_at = NULL WHERE id = ?1",
                values([event.payload.subject_id.clone()]),
            ).await?,
            EventKind::ItemStateChanged => self.connection.execute(
                "UPDATE items SET state = ?1 WHERE id = ?2",
                values([value("to")?.to_owned(), event.payload.subject_id.clone()]),
            ).await?,
            EventKind::ItemNoteAdded => self.connection.execute(
                "INSERT INTO notes(event_cid, item_id, body, recorded_at) VALUES (?1, ?2, ?3, ?4)",
                values([event.cid.to_string(), event.payload.subject_id.clone(), value("note")?.to_owned(), event.payload.recorded_at.to_rfc3339()]),
            ).await?,
            EventKind::ItemCitationAdded => self.connection.execute(
                "INSERT INTO citations(event_cid, item_id, target, label, recorded_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                vec![
                    Value::Text(event.cid.to_string()), Value::Text(event.payload.subject_id.clone()),
                    Value::Text(value("target")?.to_owned()),
                    event.payload.data.get("label").map_or(Value::Null, |label| Value::Text(label.clone())),
                    Value::Text(event.payload.recorded_at.to_rfc3339()),
                ],
            ).await?,
            EventKind::ItemParentSet => self.connection.execute(
                "UPDATE items SET parent_id = ?1 WHERE id = ?2",
                values([link("parent")?.to_owned(), event.payload.subject_id.clone()]),
            ).await?,
            EventKind::ItemTitleChanged => self.connection.execute(
                "UPDATE items SET title = ?1 WHERE id = ?2",
                values([value("to")?.to_owned(), event.payload.subject_id.clone()]),
            ).await?,
            EventKind::ItemParentCleared => self.connection.execute(
                "UPDATE items SET parent_id = NULL WHERE id = ?1",
                values([event.payload.subject_id.clone()]),
            ).await?,
            EventKind::ItemProjectChanged => self.connection.execute(
                "UPDATE items SET project_id = ?1 WHERE id = ?2",
                vec![event.payload.project_id.map_or(Value::Null, |id| Value::Text(id.to_string())), Value::Text(event.payload.subject_id.clone())],
            ).await?,
            EventKind::ItemPriorityChanged => self.connection.execute(
                "UPDATE items SET priority = ?1 WHERE id = ?2",
                values([value("to")?.to_owned(), event.payload.subject_id.clone()]),
            ).await?,
            EventKind::ItemDueDateChanged | EventKind::ItemDeferDateChanged => {
                let column = if matches!(&event.payload.kind, EventKind::ItemDueDateChanged) { "due_on" } else { "defer_until" };
                self.connection.execute(
                    &format!("UPDATE items SET {column} = ?1 WHERE id = ?2"),
                    vec![if value("to")?.is_empty() { Value::Null } else { Value::Text(value("to")?.to_owned()) }, Value::Text(event.payload.subject_id.clone())],
                ).await?
            }
            EventKind::ItemDependencyAdded => self.connection.execute(
                "INSERT INTO item_dependencies(item_id, blocker_id) VALUES (?1, ?2)",
                values([event.payload.subject_id.clone(), link("depends-on").or_else(|_| link("blocked-by"))?.to_owned()]),
            ).await?,
            EventKind::ItemDependencyRemoved => self.connection.execute(
                "DELETE FROM item_dependencies WHERE item_id = ?1 AND blocker_id = ?2",
                values([event.payload.subject_id.clone(), link("no-longer-depends-on").or_else(|_| link("unblocked-by"))?.to_owned()]),
            ).await?,
            EventKind::ItemBlockAdded => self.connection.execute(
                "INSERT INTO item_blocks(item_id, blocker_id) VALUES (?1, ?2)",
                values([event.payload.subject_id.clone(), link("blocked-by")?.to_owned()]),
            ).await?,
            EventKind::ItemBlockRemoved => self.connection.execute(
                "DELETE FROM item_blocks WHERE item_id = ?1 AND blocker_id = ?2",
                values([event.payload.subject_id.clone(), link("no-longer-blocked-by")?.to_owned()]),
            ).await?,
            EventKind::FocusSequenceCreated => self.connection.execute(
                "INSERT INTO focus_sequences(id, title) VALUES (?1, ?2)",
                values([event.payload.subject_id.clone(), value("title")?.to_owned()]),
            ).await?,
            EventKind::FocusSequenceEntryAdded => self.connection.execute(
                "INSERT INTO focus_sequence_entries(sequence_id, position, item_id) VALUES (?1, ?2, ?3)",
                values([event.payload.subject_id.clone(), value("position")?.to_owned(), link("item")?.to_owned()]),
            ).await?,
            EventKind::FocusSequenceEntryRemoved => self.connection.execute(
                "DELETE FROM focus_sequence_entries WHERE sequence_id = ?1 AND position = ?2",
                values([event.payload.subject_id.clone(), value("position")?.to_owned()]),
            ).await?,
            EventKind::ItemSessionStarted => self.connection.execute(
                "INSERT INTO item_sessions(session_id, item_id, started_at, finished_at) VALUES (?1, ?2, ?3, NULL)",
                values([value("session_id")?.to_owned(), event.payload.subject_id.clone(), value("started_at")?.to_owned()]),
            ).await?,
            EventKind::ItemSessionFinished => self.connection.execute(
                "UPDATE item_sessions SET finished_at = ?1 WHERE session_id = ?2",
                values([value("finished_at")?.to_owned(), value("session_id")?.to_owned()]),
            ).await?,
            EventKind::ItemPatched => {
                let title = event.payload.data.get("title");
                let priority = event.payload.data.get("priority");
                let state = event.payload.data.get("state");
                let due_on = event.payload.data.get("due_on");
                let defer_until = event.payload.data.get("defer_until");
                let scheduled_on = event.payload.data.get("scheduled_on");
                if let Some(title) = title { self.connection.execute("UPDATE items SET title = ?1 WHERE id = ?2", values([title.clone(), event.payload.subject_id.clone()])).await?; }
                if let Some(priority) = priority { self.connection.execute("UPDATE items SET priority = ?1 WHERE id = ?2", values([priority.clone(), event.payload.subject_id.clone()])).await?; }
                if let Some(state) = state { self.connection.execute("UPDATE items SET state = ?1 WHERE id = ?2", values([state.clone(), event.payload.subject_id.clone()])).await?; }
                for (column, date) in [("due_on", due_on), ("defer_until", defer_until), ("scheduled_on", scheduled_on)] {
                    if let Some(date) = date { self.connection.execute(&format!("UPDATE items SET {column} = ?1 WHERE id = ?2"), vec![if date.is_empty() { Value::Null } else { Value::Text(date.clone()) }, Value::Text(event.payload.subject_id.clone())]).await?; }
                }
                0
            }
            EventKind::NamespaceAssigned => match event.payload.data.get("role") {
                Some(role) => self.connection.execute(
                    "INSERT OR IGNORE INTO item_roles(item_id, role, namespace_id) VALUES (?1, ?2, ?3)",
                    values([event.payload.subject_id.clone(), role.clone(), link("namespace")?.to_owned()]),
                ).await?,
                None => self.connection.execute(
                    "INSERT INTO item_namespaces(item_id, namespace_id) VALUES (?1, ?2)",
                    values([event.payload.subject_id.clone(), link("namespace")?.to_owned()]),
                ).await?,
            },
            EventKind::NamespaceRemoved => match event.payload.data.get("role") {
                Some(role) => self.connection.execute(
                    "DELETE FROM item_roles WHERE item_id = ?1 AND role = ?2 AND namespace_id = ?3",
                    values([event.payload.subject_id.clone(), role.clone(), link("namespace")?.to_owned()]),
                ).await?,
                None => self.connection.execute(
                    "DELETE FROM item_namespaces WHERE item_id = ?1 AND namespace_id = ?2",
                    values([event.payload.subject_id.clone(), link("namespace")?.to_owned()]),
                ).await?,
            },
            EventKind::NamespaceFieldSet => self.write_field(&event.payload.subject_id, value("key")?, value("value")?).await?,
            EventKind::NamespaceLinked => self.connection.execute(
                "INSERT OR IGNORE INTO namespace_links(namespace_id, relation, target_id) VALUES (?1, ?2, ?3)",
                values([event.payload.subject_id.clone(), value("relation")?.to_owned(), link("target")?.to_owned()]),
            ).await?,
            EventKind::NamespaceUnlinked => self.connection.execute(
                "DELETE FROM namespace_links WHERE namespace_id = ?1 AND relation = ?2 AND target_id = ?3",
                values([event.payload.subject_id.clone(), value("relation")?.to_owned(), link("target")?.to_owned()]),
            ).await?,
        };
        Ok(())
    }

    async fn transaction_begin(&self) -> Result<()> {
        self.connection.execute("BEGIN IMMEDIATE", ()).await?;
        Ok(())
    }

    async fn transaction_finish(&self, result: Result<()>) -> Result<()> {
        match result {
            Ok(()) => {
                self.connection.execute("COMMIT", ()).await?;
                Ok(())
            }
            Err(error) => {
                let rollback = self.connection.execute("ROLLBACK", ()).await;
                if let Err(rollback_error) = rollback {
                    return Err(error.context(format!("rollback also failed: {rollback_error}")));
                }
                Err(error)
            }
        }
    }
}

fn is_lock_error(error: &turso::Error) -> bool {
    matches!(error, turso::Error::Busy(_) | turso::Error::BusySnapshot(_))
        || error.to_string().contains("locked by another process")
}

fn values<const N: usize>(items: [String; N]) -> Vec<Value> {
    items.into_iter().map(Value::Text).collect()
}

fn compare_focus(left: &StoredItem, right: &StoredItem) -> Ordering {
    let state_rank = |state| match state {
        ItemState::Active => 0,
        ItemState::Inbox => 1,
        ItemState::Done | ItemState::Canceled => 2,
        // Void items are filtered out long before this, but ranking them last
        // keeps the comparison total rather than relying on that.
        ItemState::Void => 3,
    };
    state_rank(left.item.state)
        .cmp(&state_rank(right.item.state))
        .then_with(|| right.item.priority.rank().cmp(&left.item.priority.rank()))
        .then_with(|| match (left.item.due_on, right.item.due_on) {
            (Some(left), Some(right)) => left.cmp(&right),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => Ordering::Equal,
        })
        .then_with(|| left.item.id.to_string().cmp(&right.item.id.to_string()))
}

fn text(row: &turso::Row, index: usize) -> Result<String> {
    row.get_value(index)?
        .as_text()
        .cloned()
        .ok_or_else(|| anyhow!("column {index} is not text"))
}

fn optional_text(row: &turso::Row, index: usize) -> Result<Option<String>> {
    let value = row.get_value(index)?;
    match value {
        Value::Null => Ok(None),
        Value::Text(value) => Ok(Some(value)),
        _ => bail!("column {index} is neither text nor null"),
    }
}

fn item_from_row(row: &turso::Row) -> Result<Item> {
    Ok(Item {
        id: ItemId::parse(&text(row, 0)?)?,
        title: text(row, 1)?,
        state: text(row, 2)?.parse()?,
        project_id: optional_text(row, 3)?
            .map(|value| NamespaceId::parse(&value))
            .transpose()?,
        parent_id: optional_text(row, 4)?
            .map(|value| ItemId::parse(&value))
            .transpose()?,
        priority: text(row, 5)?.parse()?,
        due_on: optional_text(row, 6)?
            .map(|value| NaiveDate::parse_from_str(&value, "%Y-%m-%d"))
            .transpose()?,
        defer_until: optional_text(row, 7)?
            .map(|value| NaiveDate::parse_from_str(&value, "%Y-%m-%d"))
            .transpose()?,
        scheduled_on: optional_text(row, 8)?
            .map(|value| NaiveDate::parse_from_str(&value, "%Y-%m-%d"))
            .transpose()?,
        difficulty: optional_integer(row, 9)?
            .map(u8::try_from)
            .transpose()
            .context("difficulty is out of range")?,
        estimate_minutes: optional_integer(row, 10)?
            .map(u32::try_from)
            .transpose()
            .context("estimate is out of range")?,
    })
}

/// Parses an event data field holding a number, where empty clears the value.
fn parse_optional_number(raw: &str) -> Result<Value> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Ok(Value::Null);
    }
    Ok(Value::Integer(raw.parse::<i64>()?))
}

/// Reads an optional integer column, rejecting any other stored type.
fn optional_integer(row: &turso::Row, index: usize) -> Result<Option<i64>> {
    match row.get_value(index)? {
        Value::Null => Ok(None),
        Value::Integer(value) => Ok(Some(value)),
        _ => bail!("column {index} is neither an integer nor null"),
    }
}

#[cfg(test)]
#[allow(
    clippy::indexing_slicing,
    clippy::unwrap_used,
    reason = "test fixtures use direct setup and assertion values"
)]
mod tests {
    use super::{BatchItem, ItemPatch, ItemScope, Ledger, MEMBER_OF, repo_key};
    use chrono::{DateTime, NaiveDate};
    use tempfile::tempdir;
    use todo_core::RelationKind;
    use todo_core::{ItemState, NamespacePath, Priority};

    async fn ledger() -> Ledger {
        let directory = tempdir().unwrap().keep();
        Ledger::open(&directory.join("ledger.db")).await.unwrap()
    }

    #[tokio::test]
    async fn a_void_item_leaves_every_listing_but_can_be_brought_back() {
        let ledger = ledger().await;
        let keeper = ledger.add_item("Real work", None).await.unwrap();
        let mistake = ledger.add_item("Mistyped capture", None).await.unwrap();
        let id = mistake.item.id.to_string();
        ledger.set_state(&id, ItemState::Void).await.unwrap();

        // Even an all-states listing leaves it out: all states means all the work.
        let listed = ledger.list_items(ItemScope::All, true).await.unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].item.id, keeper.item.id);

        let voided = ledger.void_items().await.unwrap();
        assert_eq!(voided.len(), 1);
        assert_eq!(voided[0].item.title, "Mistyped capture");

        ledger.set_state(&id, ItemState::Inbox).await.unwrap();
        assert_eq!(
            ledger.list_items(ItemScope::All, true).await.unwrap().len(),
            2
        );
        assert!(ledger.void_items().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn voiding_survives_a_view_rebuild() {
        let ledger = ledger().await;
        let mistake = ledger.add_item("Duplicate", None).await.unwrap();
        ledger
            .set_state(&mistake.item.id.to_string(), ItemState::Void)
            .await
            .unwrap();
        ledger.rebuild_views().await.unwrap();
        assert!(
            ledger
                .list_items(ItemScope::All, true)
                .await
                .unwrap()
                .is_empty()
        );
        assert_eq!(ledger.void_items().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn a_typed_relation_crosses_projects_without_implying_containment() {
        let ledger = ledger().await;
        let design: NamespacePath = "project/design".parse().unwrap();
        let build: NamespacePath = "project/build".parse().unwrap();
        let spec = ledger.add_item("Graph spec", Some(&design)).await.unwrap();
        let work = ledger
            .add_item("Draw the graph", Some(&build))
            .await
            .unwrap();
        let related = ledger
            .add_relation(
                &work.item.id.to_string(),
                RelationKind::Implements,
                &spec.item.id.to_string(),
            )
            .await
            .unwrap();
        assert_eq!(related.relations.len(), 1);
        // A relation is not parenthood and does not block readiness.
        assert!(related.item.parent_id.is_none());
        assert!(related.blocked_by.is_empty());

        let inverse = ledger
            .related_from(&spec.item.id.to_string())
            .await
            .unwrap();
        assert_eq!(inverse, vec![(RelationKind::Implements, work.item.id)]);

        let repeated = ledger
            .add_relation(
                &work.item.id.to_string(),
                RelationKind::Implements,
                &spec.item.id.to_string(),
            )
            .await
            .unwrap();
        assert_eq!(repeated.relations.len(), 1);

        let removed = ledger
            .remove_relation(
                &work.item.id.to_string(),
                RelationKind::Implements,
                &spec.item.id.to_string(),
            )
            .await
            .unwrap();
        assert!(removed.relations.is_empty());
    }

    #[tokio::test]
    async fn an_item_cannot_relate_to_itself() {
        let ledger = ledger().await;
        let item = ledger.add_item("Only one", None).await.unwrap();
        let id = item.item.id.to_string();
        assert!(
            ledger
                .add_relation(&id, RelationKind::ContributesTo, &id)
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn relations_survive_a_view_rebuild() {
        let ledger = ledger().await;
        let goal = ledger.add_item("Ship the editor", None).await.unwrap();
        let task = ledger.add_item("Tune the colors", None).await.unwrap();
        ledger
            .add_relation(
                &task.item.id.to_string(),
                RelationKind::ContributesTo,
                &goal.item.id.to_string(),
            )
            .await
            .unwrap();
        ledger.rebuild_views().await.unwrap();
        let reloaded = ledger.get_item(&task.item.id.to_string()).await.unwrap();
        assert_eq!(reloaded.relations.len(), 1);
        assert_eq!(reloaded.relations[0].target, goal.item.id);
    }

    #[tokio::test]
    async fn a_deleted_item_disappears_from_reads_and_can_be_restored() {
        let ledger = ledger().await;
        let item = ledger.add_item("Duplicate task", None).await.unwrap();
        let id = item.item.id.to_string();
        let deleted = ledger.delete_item(&id).await.unwrap();
        assert_eq!(deleted.title, "Duplicate task");
        assert!(
            ledger
                .list_items(ItemScope::All, true)
                .await
                .unwrap()
                .is_empty()
        );
        assert!(ledger.get_item(&id).await.is_err());
        let listed = ledger.deleted_items().await.unwrap();
        assert_eq!(listed.len(), 1);
        let restored = ledger.restore_item(&id).await.unwrap();
        assert_eq!(restored.item.title, "Duplicate task");
        assert_eq!(
            ledger.list_items(ItemScope::All, true).await.unwrap().len(),
            1
        );
        assert!(ledger.deleted_items().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn deleting_a_referenced_item_is_refused() {
        let ledger = ledger().await;
        let parent = ledger.add_item("Parent", None).await.unwrap();
        let child = ledger.add_item("Child", None).await.unwrap();
        ledger
            .set_parent(&child.item.id.to_string(), &parent.item.id.to_string())
            .await
            .unwrap();
        let refused = ledger.delete_item(&parent.item.id.to_string()).await;
        assert!(refused.is_err());
        assert_eq!(
            ledger.list_items(ItemScope::All, true).await.unwrap().len(),
            2
        );
    }

    #[tokio::test]
    async fn a_deletion_survives_a_view_rebuild() {
        let ledger = ledger().await;
        let item = ledger.add_item("Gone", None).await.unwrap();
        let keeper = ledger.add_item("Kept", None).await.unwrap();
        ledger.delete_item(&item.item.id.to_string()).await.unwrap();
        ledger.rebuild_views().await.unwrap();
        let remaining = ledger.list_items(ItemScope::All, true).await.unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].item.id, keeper.item.id);
        assert_eq!(ledger.deleted_items().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn item_workflow_preserves_history_notes_and_citations() {
        let ledger = ledger().await;
        let project: NamespacePath = "project/todo".parse().unwrap();
        let item = ledger
            .add_item("Design global scope", Some(&project))
            .await
            .unwrap();
        ledger
            .set_state(&item.item.id.to_string(), ItemState::Active)
            .await
            .unwrap();
        ledger
            .add_note(&item.item.id.to_string(), "Prefer one database")
            .await
            .unwrap();
        ledger
            .add_citation(
                &item.item.id.to_string(),
                "docs/architecture.md",
                Some("Architecture"),
            )
            .await
            .unwrap();
        ledger
            .set_state(&item.item.id.to_string(), ItemState::Done)
            .await
            .unwrap();

        let stored = ledger.get_item(&item.item.id.to_string()).await.unwrap();
        assert_eq!(stored.item.state, ItemState::Done);
        assert_eq!(ledger.notes(stored.item.id).await.unwrap().len(), 1);
        assert_eq!(ledger.citations(stored.item.id).await.unwrap().len(), 1);
        assert_eq!(ledger.history(stored.item.id).await.unwrap().len(), 5);
    }

    #[tokio::test]
    async fn tree_cycles_are_rejected() {
        let ledger = ledger().await;
        let parent = ledger.add_item("Parent", None).await.unwrap();
        let child = ledger.add_item("Child", None).await.unwrap();
        ledger
            .set_parent(&child.item.id.to_string(), &parent.item.id.to_string())
            .await
            .unwrap();
        assert!(
            ledger
                .set_parent(&parent.item.id.to_string(), &child.item.id.to_string())
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn namespace_renames_preserve_assignments() {
        let ledger = ledger().await;
        let old: NamespacePath = "topic/rust/sqlite".parse().unwrap();
        let new: NamespacePath = "topic/rust/turso".parse().unwrap();
        let item = ledger.add_item("Evaluate database", None).await.unwrap();
        ledger
            .assign_namespace(&item.item.id.to_string(), &old)
            .await
            .unwrap();
        ledger.rename_namespace(&old, &new).await.unwrap();
        let stored = ledger.get_item(&item.item.id.to_string()).await.unwrap();
        assert_eq!(stored.namespaces, vec![new]);
    }

    #[tokio::test]
    async fn unscoped_inbox_is_distinct_from_the_all_projects_view() {
        let ledger = ledger().await;
        let project: NamespacePath = "project/todo".parse().unwrap();
        ledger.add_item("Unscoped", None).await.unwrap();
        ledger
            .add_item("Project item", Some(&project))
            .await
            .unwrap();

        let unscoped = ledger.list_items(ItemScope::Unscoped, false).await.unwrap();
        assert_eq!(unscoped.len(), 1);
        assert_eq!(unscoped[0].item.title, "Unscoped");
        assert_eq!(
            ledger
                .list_items(ItemScope::All, false)
                .await
                .unwrap()
                .len(),
            2
        );
    }

    #[tokio::test]
    async fn dependencies_active_blocks_and_defer_dates_define_readiness() {
        let ledger = ledger().await;
        let today = NaiveDate::from_ymd_opt(2026, 8, 28).unwrap();
        let blocker = ledger.add_item("Get access", None).await.unwrap();
        let work = ledger.add_item("Implement", None).await.unwrap();
        let later = ledger.add_item("Future work", None).await.unwrap();
        ledger
            .add_dependency(&work.item.id.to_string(), &blocker.item.id.to_string())
            .await
            .unwrap();
        ledger
            .set_defer_until(
                &later.item.id.to_string(),
                Some(NaiveDate::from_ymd_opt(2026, 8, 29).unwrap()),
            )
            .await
            .unwrap();

        let ready = ledger.list_ready(ItemScope::Unscoped, today).await.unwrap();
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].item.title, "Get access");
        assert!(
            ledger
                .add_dependency(&blocker.item.id.to_string(), &work.item.id.to_string())
                .await
                .is_err()
        );

        ledger
            .set_state(&blocker.item.id.to_string(), ItemState::Done)
            .await
            .unwrap();
        let ready = ledger.list_ready(ItemScope::Unscoped, today).await.unwrap();
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].item.title, "Implement");
    }

    #[tokio::test]
    async fn active_blocks_are_distinct_from_planned_dependencies() {
        let ledger = ledger().await;
        let blocker = ledger.add_item("Waiting on review", None).await.unwrap();
        let work = ledger.add_item("Ship change", None).await.unwrap();
        ledger
            .add_blocker(&work.item.id.to_string(), &blocker.item.id.to_string())
            .await
            .unwrap();

        let stored = ledger.get_item(&work.item.id.to_string()).await.unwrap();
        assert!(stored.dependencies.is_empty());
        assert_eq!(stored.blocked_by, vec![blocker.item.id]);
        let today = NaiveDate::from_ymd_opt(2026, 8, 28).unwrap();
        assert!(
            ledger
                .list_ready(ItemScope::Unscoped, today)
                .await
                .unwrap()
                .iter()
                .all(|item| item.item.id != work.item.id)
        );

        ledger
            .set_state(&blocker.item.id.to_string(), ItemState::Done)
            .await
            .unwrap();
        assert!(
            ledger
                .list_ready(ItemScope::Unscoped, today)
                .await
                .unwrap()
                .iter()
                .all(|item| item.item.id != work.item.id)
        );

        ledger
            .remove_blocker(&work.item.id.to_string(), &blocker.item.id.to_string())
            .await
            .unwrap();
        assert!(
            ledger
                .list_ready(ItemScope::Unscoped, today)
                .await
                .unwrap()
                .iter()
                .any(|item| item.item.id == work.item.id)
        );
    }

    #[tokio::test]
    async fn focus_sequence_orders_existing_tasks_without_affecting_readiness() {
        let ledger = ledger().await;
        let first = ledger.add_item("A1", None).await.unwrap();
        let second = ledger.add_item("B1", None).await.unwrap();
        let sequence = ledger.create_focus_sequence("Today").await.unwrap();
        ledger
            .add_focus_sequence_item(&sequence.id, &second.item.id.to_string())
            .await
            .unwrap();
        ledger
            .add_focus_sequence_item(&sequence.id, &first.item.id.to_string())
            .await
            .unwrap();
        let sequences = ledger.focus_sequences().await.unwrap();
        assert_eq!(sequences.len(), 1);
        assert_eq!(sequences[0].items, vec![second.item.id, first.item.id]);
        assert_eq!(
            ledger
                .list_ready(
                    ItemScope::Unscoped,
                    NaiveDate::from_ymd_opt(2026, 8, 28).unwrap()
                )
                .await
                .unwrap()
                .len(),
            2
        );
    }

    #[tokio::test]
    async fn sessions_are_append_only_and_have_one_active_record() {
        let ledger = ledger().await;
        let item = ledger.add_item("Timed work", None).await.unwrap();
        let started = DateTime::parse_from_rfc3339("2026-08-28T09:00:00Z")
            .unwrap()
            .to_utc();
        let finished = DateTime::parse_from_rfc3339("2026-08-28T10:00:00Z")
            .unwrap()
            .to_utc();
        ledger
            .start_session(&item.item.id.to_string(), started)
            .await
            .unwrap();
        assert!(
            ledger
                .start_session(&item.item.id.to_string(), started)
                .await
                .is_err()
        );
        let session = ledger
            .finish_session(&item.item.id.to_string(), finished)
            .await
            .unwrap();
        assert_eq!(
            session.finished_at.as_deref(),
            Some("2026-08-28T10:00:00+00:00")
        );
        assert_eq!(ledger.sessions(item.item.id).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn item_patch_is_atomic_and_rejects_invalid_fields_without_partial_write() {
        let ledger = ledger().await;
        let item = ledger.add_item("Draft", None).await.unwrap();
        let updated = ledger
            .apply_patch(
                &item.item.id.to_string(),
                ItemPatch {
                    title: Some("Published".to_owned()),
                    priority: Some(Priority::High),
                    due_on: Some(Some(NaiveDate::from_ymd_opt(2026, 9, 1).unwrap())),
                    ..ItemPatch::default()
                },
            )
            .await
            .unwrap();
        assert_eq!(updated.item.title, "Published");
        assert_eq!(updated.item.priority, Priority::High);
        assert_eq!(updated.item.due_on, NaiveDate::from_ymd_opt(2026, 9, 1));
        let rejected = ledger
            .apply_patch(
                &item.item.id.to_string(),
                ItemPatch {
                    title: Some("".to_owned()),
                    priority: Some(Priority::Critical),
                    ..ItemPatch::default()
                },
            )
            .await;
        assert!(rejected.is_err());
        let unchanged = ledger.get_item(&item.item.id.to_string()).await.unwrap();
        assert_eq!(unchanged.item.title, "Published");
        assert_eq!(unchanged.item.priority, Priority::High);
    }

    #[tokio::test]
    async fn next_prefers_state_then_priority_then_due_date() {
        let ledger = ledger().await;
        let today = NaiveDate::from_ymd_opt(2026, 8, 28).unwrap();
        let active = ledger.add_item("Active low", None).await.unwrap();
        ledger
            .set_priority(&active.item.id.to_string(), Priority::Low)
            .await
            .unwrap();
        ledger
            .set_state(&active.item.id.to_string(), ItemState::Active)
            .await
            .unwrap();
        let critical = ledger.add_item("Inbox critical", None).await.unwrap();
        ledger
            .set_priority(&critical.item.id.to_string(), Priority::Critical)
            .await
            .unwrap();

        let next = ledger
            .next_item(ItemScope::Unscoped, today, |_| true)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(next.item.item.title, "Active low");

        ledger
            .set_state(&active.item.id.to_string(), ItemState::Done)
            .await
            .unwrap();
        let next = ledger
            .next_item(ItemScope::Unscoped, today, |_| true)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(next.item.item.title, "Inbox critical");
    }

    #[tokio::test]
    async fn repairs_append_history_and_preserve_scope_invariants() {
        let ledger = ledger().await;
        let first: NamespacePath = "project/first".parse().unwrap();
        let second: NamespacePath = "project/second".parse().unwrap();
        let tag: NamespacePath = "topic/old/name".parse().unwrap();
        let parent = ledger.add_item("Parent", Some(&first)).await.unwrap();
        let child = ledger
            .add_item_with(
                "Child typo",
                Some(&first),
                std::slice::from_ref(&tag),
                Some(&parent.item.id.to_string()),
            )
            .await
            .unwrap();

        assert!(
            ledger
                .move_project(&child.item.id.to_string(), Some(&second))
                .await
                .is_err()
        );
        ledger
            .edit_title(&child.item.id.to_string(), "Child corrected")
            .await
            .unwrap();
        ledger
            .remove_namespace(&child.item.id.to_string(), &tag)
            .await
            .unwrap();
        ledger
            .clear_parent(&child.item.id.to_string())
            .await
            .unwrap();
        let moved = ledger
            .move_project(&child.item.id.to_string(), Some(&second))
            .await
            .unwrap();

        assert_eq!(moved.item.title, "Child corrected");
        assert!(moved.item.parent_id.is_none());
        assert!(moved.namespaces.is_empty());
        assert_ne!(moved.item.project_id, parent.item.project_id);
        let kinds: Vec<_> = ledger
            .history(moved.item.id)
            .await
            .unwrap()
            .into_iter()
            .map(|event| event.kind)
            .collect();
        assert!(kinds.contains(&"item.title-changed".to_owned()));
        assert!(kinds.contains(&"item.parent-cleared".to_owned()));
        assert!(kinds.contains(&"item.project-changed".to_owned()));
        assert!(kinds.contains(&"namespace.removed".to_owned()));
    }

    #[tokio::test]
    async fn markdown_shaped_batch_is_atomic_and_preserves_tree_state_and_source() {
        let ledger = ledger().await;
        let project: NamespacePath = "project/import".parse().unwrap();
        let items = vec![
            BatchItem {
                title: "Plan".to_owned(),
                state: ItemState::Active,
                parent_index: None,
            },
            BatchItem {
                title: "Finished child".to_owned(),
                state: ItemState::Done,
                parent_index: Some(0),
            },
        ];
        let imported = ledger
            .import_batch(Some(&project), &items, Some("tasks.md"))
            .await
            .unwrap();
        assert_eq!(imported.len(), 2);
        assert_eq!(imported[0].item.state, ItemState::Active);
        assert_eq!(imported[1].item.parent_id, Some(imported[0].item.id));
        assert_eq!(imported[1].item.state, ItemState::Done);
        assert_eq!(
            ledger.citations(imported[1].item.id).await.unwrap().len(),
            1
        );
        assert!(!ledger.event_envelopes().await.unwrap().is_empty());

        let invalid = [BatchItem {
            title: "Orphan".to_owned(),
            state: ItemState::Inbox,
            parent_index: Some(3),
        }];
        assert!(
            ledger
                .import_batch(Some(&project), &invalid, None)
                .await
                .is_err()
        );
        assert_eq!(
            ledger
                .list_items(ItemScope::Project(&project), true)
                .await
                .unwrap()
                .len(),
            2
        );
    }

    #[tokio::test]
    async fn exact_v5_events_rebuild_an_independent_ledger() {
        let source = ledger().await;
        let project: NamespacePath = "project/sync".parse().unwrap();
        let tag: NamespacePath = "topic/transport".parse().unwrap();
        let parent = source
            .add_item("Synchronize ledger", Some(&project))
            .await
            .unwrap();
        let child = source
            .add_item_with(
                "Validate replay",
                Some(&project),
                std::slice::from_ref(&tag),
                Some(&parent.item.id.to_string()),
            )
            .await
            .unwrap();
        source
            .set_priority(&child.item.id.to_string(), Priority::High)
            .await
            .unwrap();
        source
            .add_note(&child.item.id.to_string(), "Exact bytes, local views")
            .await
            .unwrap();
        source
            .add_dependency(&child.item.id.to_string(), &parent.item.id.to_string())
            .await
            .unwrap();

        let records = source.event_records().await.unwrap();
        let target = ledger().await;
        assert_eq!(target.import_events(&records).await.unwrap(), records.len());
        assert_eq!(target.import_events(&records).await.unwrap(), 0);
        target.rebuild_views().await.unwrap();

        let source_items = source.list_items(ItemScope::All, true).await.unwrap();
        let target_items = target.list_items(ItemScope::All, true).await.unwrap();
        assert_eq!(target_items, source_items);
        assert_eq!(target.notes(child.item.id).await.unwrap().len(), 1);
        assert_eq!(target.event_records().await.unwrap(), records);
    }

    #[tokio::test]
    async fn actor_fields_membership_and_roles_survive_import() {
        let source = ledger().await;
        let quincy: NamespacePath = "actor/quincy".parse().unwrap();
        let cdint: NamespacePath = "actor/cdint".parse().unwrap();
        source
            .set_field(&quincy, "kind", Some("person"))
            .await
            .unwrap();
        source.set_field(&quincy, "gh", Some("old")).await.unwrap();
        source
            .set_field(&quincy, "gh", Some("Qu1ncyRy4n"))
            .await
            .unwrap();
        source
            .set_field(&quincy, "email", Some("q@example.com"))
            .await
            .unwrap();
        source.set_field(&quincy, "email", None).await.unwrap();
        source
            .link_namespace(&quincy, MEMBER_OF, &cdint)
            .await
            .unwrap();
        let work = source.add_item("Fix sync race", None).await.unwrap();
        let id = work.item.id.to_string();
        source.assign_role(&id, "author", &quincy).await.unwrap();
        source.assign_role(&id, "reviewer", &quincy).await.unwrap();
        source.remove_role(&id, "reviewer", &quincy).await.unwrap();
        // A role is not a tag.
        assert!(source.get_item(&id).await.unwrap().namespaces.is_empty());

        let target = ledger().await;
        target
            .import_events(&source.event_records().await.unwrap())
            .await
            .unwrap();
        let fields = target.fields(&quincy).await.unwrap();
        assert_eq!(fields.get("gh").map(String::as_str), Some("Qu1ncyRy4n"));
        assert_eq!(fields.get("kind").map(String::as_str), Some("person"));
        assert!(!fields.contains_key("email"));
        assert_eq!(
            target.namespace_linked_from(&cdint).await.unwrap(),
            vec![(MEMBER_OF.to_owned(), quincy.clone())]
        );
        assert_eq!(
            target.roles(work.item.id).await.unwrap(),
            vec![("author".to_owned(), quincy.clone())]
        );
        assert_eq!(
            target.role_items(&quincy).await.unwrap(),
            vec![("author".to_owned(), work.item.id)]
        );
    }

    #[tokio::test]
    async fn relates_to_is_stored_once_and_removable_from_either_end() {
        let ledger = ledger().await;
        let one = ledger
            .add_item("One", None)
            .await
            .unwrap()
            .item
            .id
            .to_string();
        let two = ledger
            .add_item("Two", None)
            .await
            .unwrap()
            .item
            .id
            .to_string();
        ledger
            .add_relation(&one, RelationKind::RelatesTo, &two)
            .await
            .unwrap();
        let before = ledger.event_records().await.unwrap().len();
        ledger
            .add_relation(&two, RelationKind::RelatesTo, &one)
            .await
            .unwrap();
        assert_eq!(ledger.event_records().await.unwrap().len(), before);
        ledger
            .remove_relation(&two, RelationKind::RelatesTo, &one)
            .await
            .unwrap();
        assert!(ledger.get_item(&one).await.unwrap().relations.is_empty());
    }

    #[test]
    fn repo_key_equates_ssh_and_https_remotes() {
        let key = "github.com/qu1ncyry4n/todo_app_project";
        for remote in [
            "git@github.com:Qu1ncyRy4n/todo_app_project.git",
            "https://github.com/Qu1ncyRy4n/todo_app_project",
            "ssh://git@github.com/Qu1ncyRy4n/todo_app_project.git",
            "github.com/Qu1ncyRy4n/todo_app_project/",
        ] {
            assert_eq!(repo_key(remote), key, "{remote}");
        }
    }

    #[tokio::test]
    async fn projects_are_found_by_repo_field() {
        let ledger = ledger().await;
        let todo: NamespacePath = "project/todo".parse().unwrap();
        let tag: NamespacePath = "tag/todo".parse().unwrap();
        ledger
            .set_field(&todo, "repo", Some("github.com/o/todo"))
            .await
            .unwrap();
        ledger
            .set_field(&tag, "repo", Some("github.com/o/todo"))
            .await
            .unwrap();
        assert_eq!(
            ledger
                .projects_for_repo("git@github.com:o/todo.git")
                .await
                .unwrap(),
            vec![todo]
        );
        assert!(
            ledger
                .projects_for_repo("github.com/o/other")
                .await
                .unwrap()
                .is_empty()
        );
    }
}
