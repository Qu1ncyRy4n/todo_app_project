use anyhow::{Context, Result, anyhow, bail};
use chrono::{Local, NaiveDate};
use cid::Cid;
use serde::Serialize;
use std::{cmp::Ordering, collections::HashSet, path::Path, str::FromStr};
use todo_core::{
    Event, EventBuilder, EventKind, Item, ItemId, ItemState, NamespaceId, NamespaceKind,
    NamespacePath, Priority,
};
use turso::{Builder, Connection, Value};

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
  defer_until TEXT
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
"#;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct StoredItem {
    pub item: Item,
    pub namespaces: Vec<NamespacePath>,
    pub blockers: Vec<ItemId>,
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HistoryEntry {
    pub cid: String,
    pub kind: String,
    pub recorded_at: String,
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
}

impl Ledger {
    pub async fn open(path: &Path) -> Result<Self> {
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
        let ledger = Self { connection };
        ledger.migrate().await?;
        Ok(ledger)
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
        let event = self
            .event(
                EventBuilder::new(EventKind::NamespaceCreated, id.to_string())
                    .data("kind", path.kind().to_string())
                    .data("path", path.to_string()),
            )
            .await?;
        self.transaction_begin().await?;
        let result = async {
            self.insert_event(&event).await?;
            self.connection
                .execute(
                    "INSERT INTO namespaces(id, kind, canonical_path) VALUES (?1, ?2, ?3)",
                    values([id.to_string(), path.kind().to_string(), path.to_string()]),
                )
                .await?;
            self.set_head(&event.cid).await
        }
        .await;
        self.transaction_finish(result).await?;
        Ok(NamespaceEntry {
            id,
            kind: path.kind(),
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
        if namespace.kind != next.kind() {
            bail!(
                "cannot change namespace kind from {} to {}",
                namespace.kind,
                next.kind()
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
        let created = EventBuilder::for_item(EventKind::ItemCreated, id)
            .project(project_id)
            .parent(previous)
            .data("title", title)
            .data("state", ItemState::Inbox.to_string())
            .build()?;
        previous = Some(created.cid);
        events.push(created);
        for namespace in &namespace_entries {
            let event = EventBuilder::for_item(EventKind::NamespaceAssigned, id)
                .project(project_id)
                .parent(previous)
                .link("namespace", namespace.id.to_string())
                .build()?;
            previous = Some(event.cid);
            events.push(event);
        }
        if let Some(parent) = &parent {
            let event = EventBuilder::for_item(EventKind::ItemParentSet, id)
                .project(project_id)
                .parent(previous)
                .link("parent", parent.item.id.to_string())
                .build()?;
            previous = Some(event.cid);
            events.push(event);
        }
        let head = previous.expect("item creation always emits an event");

        self.transaction_begin().await?;
        let result = async {
            for event in &events {
                self.insert_event(event).await?;
            }
            self.connection
                .execute(
                    "INSERT INTO items(id, title, state, project_id, parent_id) VALUES (?1, ?2, ?3, ?4, ?5)",
                    vec![
                        Value::Text(id.to_string()),
                        Value::Text(title.to_owned()),
                        Value::Text(ItemState::Inbox.to_string()),
                        project.as_ref().map_or(Value::Null, |entry| Value::Text(entry.id.to_string())),
                        parent.as_ref().map_or(Value::Null, |stored| Value::Text(stored.item.id.to_string())),
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
            },
            namespaces: namespace_entries
                .into_iter()
                .map(|entry| entry.path)
                .collect(),
            blockers: Vec::new(),
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
            let mut events = Vec::new();
            let created = EventBuilder::for_item(EventKind::ItemCreated, ids[index])
                .project(project_id)
                .parent(previous)
                .data("title", item.title.trim())
                .data("state", item.state.to_string())
                .data("imported", "true")
                .build()?;
            previous = Some(created.cid);
            events.push(created);
            if let Some(parent_index) = item.parent_index {
                let parent = EventBuilder::for_item(EventKind::ItemParentSet, ids[index])
                    .project(project_id)
                    .parent(previous)
                    .link("parent", ids[parent_index].to_string())
                    .build()?;
                previous = Some(parent.cid);
                events.push(parent);
            }
            if let Some(source) = source_citation {
                let citation = EventBuilder::for_item(EventKind::ItemCitationAdded, ids[index])
                    .project(project_id)
                    .parent(previous)
                    .data("target", source)
                    .data("label", "import source")
                    .build()?;
                previous = Some(citation.cid);
                events.push(citation);
            }
            built.push((item, events));
        }
        let head = previous.expect("non-empty import emits events");

        self.transaction_begin().await?;
        let result = async {
            for (index, (item, events)) in built.iter().enumerate() {
                for event in events {
                    self.insert_event(event).await?;
                }
                let parent = item
                    .parent_index
                    .map_or(Value::Null, |parent| Value::Text(ids[parent].to_string()));
                self.connection
                    .execute(
                        "INSERT INTO items(id, title, state, project_id, parent_id) VALUES (?1, ?2, ?3, ?4, ?5)",
                        vec![
                            Value::Text(ids[index].to_string()),
                            Value::Text(item.title.trim().to_owned()),
                            Value::Text(item.state.to_string()),
                            project_id.map_or(Value::Null, |id| Value::Text(id.to_string())),
                            parent,
                        ],
                    )
                    .await?;
                if let Some(source) = source_citation {
                    let citation = events
                        .iter()
                        .find(|event| event.payload.kind == EventKind::ItemCitationAdded)
                        .expect("source citation created for each item");
                    self.connection
                        .execute(
                            "INSERT INTO citations(event_cid, item_id, target, label, recorded_at) VALUES (?1, ?2, ?3, 'import source', ?4)",
                            values([
                                citation.cid.to_string(),
                                ids[index].to_string(),
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
            && current
                .item
                .defer_until
                .is_some_and(|date| date > Local::now().date_naive())
        {
            bail!(
                "item is deferred until {}; clear the defer date before starting it",
                current.item.defer_until.expect("checked as present")
            );
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
            _ => unreachable!("date columns are internal constants"),
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
            || !item.blockers.is_empty()
            || self.has_children(item.item.id).await?
            || self.has_dependents(item.item.id).await?
        {
            bail!(
                "cannot move an item with parent, children, blockers, or dependents; clear those relationships first"
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
        if item.blockers.contains(&blocker.item.id) {
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
                    .link("blocked-by", blocker.item.id.to_string()),
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
        if !item.blockers.contains(&blocker.item.id) {
            bail!("{} is not blocked by {}", item.item.id, blocker.item.id);
        }
        let event = self
            .event(
                EventBuilder::for_item(EventKind::ItemDependencyRemoved, item.item.id)
                    .project(item.item.project_id)
                    .link("unblocked-by", blocker.item.id.to_string()),
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
                "SELECT id, title, state, project_id, parent_id, priority, due_on, defer_until FROM items WHERE project_id = ?1 ORDER BY id",
                values([id.to_string()]),
            ),
            (ItemScope::Project(_), Some(id), false) => (
                "SELECT id, title, state, project_id, parent_id, priority, due_on, defer_until FROM items WHERE project_id = ?1 AND state NOT IN ('done', 'canceled') ORDER BY id",
                values([id.to_string()]),
            ),
            (ItemScope::Unscoped, None, true) => (
                "SELECT id, title, state, project_id, parent_id, priority, due_on, defer_until FROM items WHERE project_id IS NULL ORDER BY id",
                Vec::<Value>::new(),
            ),
            (ItemScope::Unscoped, None, false) => (
                "SELECT id, title, state, project_id, parent_id, priority, due_on, defer_until FROM items WHERE project_id IS NULL AND state NOT IN ('done', 'canceled') ORDER BY id",
                Vec::<Value>::new(),
            ),
            (ItemScope::All, None, true) => (
                "SELECT id, title, state, project_id, parent_id, priority, due_on, defer_until FROM items ORDER BY id",
                Vec::<Value>::new(),
            ),
            (ItemScope::All, None, false) => (
                "SELECT id, title, state, project_id, parent_id, priority, due_on, defer_until FROM items WHERE state NOT IN ('done', 'canceled') ORDER BY id",
                Vec::<Value>::new(),
            ),
            _ => unreachable!("project IDs exist exactly for project scopes"),
        };
        let mut rows = self.connection.query(sql, params).await?;
        let mut items = Vec::new();
        while let Some(row) = rows.next().await? {
            let item = item_from_row(&row)?;
            let namespaces = self.item_namespaces(item.id).await?;
            let blockers = self.item_blockers(item.id).await?;
            items.push(StoredItem {
                item,
                namespaces,
                blockers,
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
            if !self.has_incomplete_blocker(item.item.id).await? {
                ready.push(item);
            }
        }
        Ok(ready)
    }

    pub async fn next_item(
        &self,
        scope: ItemScope<'_>,
        today: NaiveDate,
    ) -> Result<Option<NextItem>> {
        let mut ready = self.list_ready(scope, today).await?;
        ready.sort_by(compare_focus);
        let Some(item) = ready.into_iter().next() else {
            return Ok(None);
        };
        let mut reasons = vec![
            "ready: no incomplete blockers and defer date reached".to_owned(),
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
        let pattern = format!("%{reference}");
        let mut rows = self
            .connection
            .query(
                "SELECT id, title, state, project_id, parent_id, priority, due_on, defer_until FROM items WHERE id = ?1 OR id LIKE ?2 ORDER BY id LIMIT 2",
                values([reference.to_owned(), pattern]),
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
        let blockers = self.item_blockers(item.id).await?;
        Ok(StoredItem {
            item,
            namespaces,
            blockers,
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

    pub async fn history(&self, item: ItemId) -> Result<Vec<HistoryEntry>> {
        let mut rows = self.connection.query(
            "SELECT cid, kind, recorded_at FROM events WHERE subject_id = ?1 ORDER BY recorded_at, event_id",
            values([item.to_string()]),
        ).await?;
        let mut result = Vec::new();
        while let Some(row) = rows.next().await? {
            result.push(HistoryEntry {
                cid: text(&row, 0)?,
                kind: text(&row, 1)?,
                recorded_at: text(&row, 2)?,
            });
        }
        Ok(result)
    }

    pub async fn event_envelopes(&self) -> Result<Vec<Vec<u8>>> {
        let mut rows = self
            .connection
            .query(
                "SELECT envelope FROM events ORDER BY recorded_at, event_id",
                (),
            )
            .await?;
        let mut result = Vec::new();
        while let Some(row) = rows.next().await? {
            match row.get_value(0)? {
                Value::Blob(bytes) => result.push(bytes),
                _ => bail!("event envelope column is not a blob"),
            }
        }
        Ok(result)
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

    async fn item_blockers(&self, item: ItemId) -> Result<Vec<ItemId>> {
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

    async fn namespace_by_path(&self, path: &NamespacePath) -> Result<Option<NamespaceEntry>> {
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

    async fn has_incomplete_blocker(&self, item: ItemId) -> Result<bool> {
        let mut rows = self
            .connection
            .query(
                "SELECT 1 FROM item_dependencies d JOIN items b ON b.id = d.blocker_id WHERE d.item_id = ?1 AND b.state NOT IN ('done', 'canceled') LIMIT 1",
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
                "SELECT 1 FROM item_dependencies WHERE blocker_id = ?1 LIMIT 1",
                values([item.to_string()]),
            )
            .await?;
        Ok(rows.next().await?.is_some())
    }

    async fn event(&self, builder: EventBuilder) -> Result<Event> {
        let parent = self.head().await?;
        builder.parent(parent).build().context("build event")
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
    })
}

#[cfg(test)]
mod tests {
    use super::{BatchItem, ItemScope, Ledger};
    use chrono::NaiveDate;
    use tempfile::tempdir;
    use todo_core::{ItemState, NamespacePath, Priority};

    async fn ledger() -> Ledger {
        let directory = tempdir().unwrap().keep();
        Ledger::open(&directory.join("ledger.db")).await.unwrap()
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
    async fn blockers_and_defer_dates_define_readiness() {
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
            .next_item(ItemScope::Unscoped, today)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(next.item.item.title, "Active low");

        ledger
            .set_state(&active.item.id.to_string(), ItemState::Done)
            .await
            .unwrap();
        let next = ledger
            .next_item(ItemScope::Unscoped, today)
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
}
