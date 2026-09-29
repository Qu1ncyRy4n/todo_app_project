//! Native graph editor for the local Todo Ledger.

mod geometry;
mod theme;

use anyhow::{Context, Result, bail};
use chrono::{Local, NaiveDate};
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, Vec2};
use geometry::{BandSpacing, FlowSpacing};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, path::PathBuf};
use theme::{NodeShape, RegionShape, TagColoring, TagPalette, Theme};
use todo_core::{ItemState, NamespacePath, Priority};
use todo_store::{
    AppConfig, AppPaths, Citation, FocusSequence, ItemPatch, ItemScope, Ledger, NamespaceEntry,
    Note, StoredItem, WorkSession,
};
use tokio::runtime::Runtime;

fn main() -> Result<()> {
    let paths = AppPaths::discover()?;
    if !paths.database().exists() {
        bail!("global ledger is not initialized; run `todo init`");
    }
    let mut config = AppConfig::load(&paths.config())?;
    let actor_id = match config.actor_id.clone() {
        Some(actor_id) => actor_id,
        None => {
            let actor_id = config.ensure_actor_id().to_owned();
            config.save(&paths.config())?;
            actor_id
        }
    };
    let runtime = Runtime::new().context("create GUI async runtime")?;
    let ledger = runtime.block_on(Ledger::open_with_actor(&paths.database(), actor_id))?;
    let layout_path = paths.config_dir.join("gui-layout.json");
    let theme_path = paths.config_dir.join("theme.toml");
    eframe::run_native(
        "Todo Ledger",
        eframe::NativeOptions::default(),
        Box::new(move |_| {
            Ok(Box::new(TodoGui::new(
                runtime,
                ledger,
                layout_path,
                theme_path,
            )))
        }),
    )
    .context("run native Todo Ledger window")
}

struct TodoGui {
    runtime: Runtime,
    ledger: Ledger,
    items: Vec<StoredItem>,
    namespaces: Vec<NamespaceEntry>,
    notes: Vec<Note>,
    citations: Vec<Citation>,
    sessions: Vec<WorkSession>,
    focus_sequences: Vec<FocusSequence>,
    selected_focus_sequence: Option<String>,
    focus_title_draft: String,
    selected: Option<String>,
    title_draft: String,
    priority_draft: Priority,
    state_draft: ItemState,
    show_parent_graph: bool,
    show_dependency_graph: bool,
    show_blocker_graph: bool,
    show_tag_constellation: bool,
    show_parent_regions: bool,
    show_focus_window: bool,
    show_completed: bool,
    tag_filter: String,
    message: Option<String>,
    raw_json: String,
    tag_draft: String,
    parent_draft: Option<String>,
    node_offsets: HashMap<String, Vec2>,
    layout_path: PathBuf,
    drag_anchor: Option<(String, Pos2, Vec2)>,
    parent_search: String,
    dependency_search: String,
    blocker_search: String,
    due_draft: String,
    defer_draft: String,
    scheduled_draft: String,
    graph_pan: Vec2,
    graph_zoom: f32,
    theme: Theme,
    theme_path: PathBuf,
    layout_undo: Option<HashMap<String, Vec2>>,
    normalize_requested: bool,
    focus_layout_requested: bool,
    new_task_open: bool,
    new_task_title: String,
    note_draft: String,
    link_target_draft: String,
    link_label_draft: String,
}

#[derive(Default, Deserialize, Serialize)]
struct LocalGraphLayout {
    node_offsets: HashMap<String, [f32; 2]>,
}

impl TodoGui {
    fn new(runtime: Runtime, ledger: Ledger, layout_path: PathBuf, theme_path: PathBuf) -> Self {
        let node_offsets = load_layout(&layout_path);
        let (theme, theme_message) = match Theme::load(&theme_path) {
            Ok(theme) => (theme, None),
            Err(error) => (
                Theme::default(),
                Some(format!("Theme not loaded: {error:#}")),
            ),
        };
        let mut app = Self {
            runtime,
            ledger,
            items: Vec::new(),
            namespaces: Vec::new(),
            notes: Vec::new(),
            citations: Vec::new(),
            sessions: Vec::new(),
            focus_sequences: Vec::new(),
            selected_focus_sequence: None,
            focus_title_draft: String::new(),
            selected: None,
            title_draft: String::new(),
            priority_draft: Priority::Normal,
            state_draft: ItemState::Inbox,
            show_parent_graph: true,
            show_dependency_graph: true,
            show_blocker_graph: true,
            show_tag_constellation: true,
            show_parent_regions: true,
            show_focus_window: true,
            show_completed: false,
            tag_filter: String::new(),
            message: theme_message,
            raw_json: String::new(),
            tag_draft: String::new(),
            parent_draft: None,
            node_offsets,
            layout_path,
            drag_anchor: None,
            parent_search: String::new(),
            dependency_search: String::new(),
            blocker_search: String::new(),
            due_draft: String::new(),
            defer_draft: String::new(),
            scheduled_draft: String::new(),
            graph_pan: Vec2::ZERO,
            graph_zoom: 1.0,
            theme,
            theme_path,
            layout_undo: None,
            normalize_requested: false,
            focus_layout_requested: false,
            new_task_open: false,
            new_task_title: String::new(),
            note_draft: String::new(),
            link_target_draft: String::new(),
            link_label_draft: String::new(),
        };
        app.refresh();
        app
    }

    /// Arranges visible nodes into Focus Window bands around the active work.
    ///
    /// Completed work goes above, active work stays centered, and future or
    /// deferred work goes below, so the present moment reads as the middle of the
    /// graph. Like normalize, the previous arrangement is kept for undo.
    fn focus_layout(&mut self, rect: Rect) {
        let today = Local::now().date_naive();
        let items = self.visible_items();
        let mut completed = Vec::new();
        let mut active = Vec::new();
        let mut future = Vec::new();
        for item in &items {
            let id = item.item.id.to_string();
            let deferred = self.theme.focus.future_includes_deferred
                && (item.item.defer_until.is_some_and(|date| date > today)
                    || item.item.scheduled_on.is_some_and(|date| date > today));
            match item.item.state {
                ItemState::Done | ItemState::Canceled => completed.push(id),
                ItemState::Active if !deferred => active.push(id),
                _ => future.push(id),
            }
        }
        let spacing = BandSpacing {
            band_gap: self.theme.focus.band_gap,
            node_gap: self.theme.focus.node_gap,
            row_gap: self.theme.focus.row_gap,
            band_width: self.theme.focus.band_width,
        };
        let arranged = geometry::banded_positions(&completed, &active, &future, spacing);
        let center = rect.center();
        self.layout_undo = Some(self.node_offsets.clone());
        for (index, item) in items.iter().enumerate() {
            let id = item.item.id.to_string();
            let Some(target) = arranged.get(&id) else {
                continue;
            };
            let slot = grid_slot(index, rect);
            self.node_offsets.insert(id, (center + *target) - slot);
        }
        self.save_layout();
        self.message =
            Some("Focus Window arranged: done above, active centered, future below.".to_owned());
    }

    /// Rearranges visible nodes so relationships flow one way, down or right.
    ///
    /// The previous arrangement is kept in `layout_undo`, so a normalize that
    /// scatters a hand-built layout can be taken back.
    fn normalize_layout(&mut self, rect: Rect) {
        let items = self.visible_items();
        let ids = items
            .iter()
            .map(|item| item.item.id.to_string())
            .collect::<Vec<_>>();
        let mut edges = Vec::new();
        for item in &items {
            let id = item.item.id.to_string();
            if self.theme.layout.follow_parents
                && let Some(parent) = item.item.parent_id
            {
                edges.push((parent.to_string(), id.clone()));
            }
            if self.theme.layout.follow_dependencies {
                for dependency in &item.dependencies {
                    edges.push((dependency.to_string(), id.clone()));
                }
            }
            if self.theme.layout.follow_blockers {
                for blocker in &item.blocked_by {
                    edges.push((blocker.to_string(), id.clone()));
                }
            }
        }
        let spacing = FlowSpacing {
            direction: self.theme.layout.direction,
            layer_gap: self.theme.layout.layer_gap,
            node_gap: self.theme.layout.node_gap,
        };
        let arranged = geometry::layered_positions(&ids, &edges, spacing);
        let origin = grid_slot(0, rect);
        self.layout_undo = Some(self.node_offsets.clone());
        for (index, id) in ids.iter().enumerate() {
            let Some(target) = arranged.get(id) else {
                continue;
            };
            let slot = grid_slot(index, rect);
            self.node_offsets
                .insert(id.clone(), (origin + *target) - slot);
        }
        self.save_layout();
        self.message = Some("Layout normalized. Undo layout restores the previous one.".to_owned());
    }

    /// Restores the arrangement captured before the last normalize.
    fn undo_layout(&mut self) {
        match self.layout_undo.take() {
            Some(offsets) => {
                self.node_offsets = offsets;
                self.save_layout();
                self.message = Some("Previous layout restored.".to_owned());
            }
            None => self.message = Some("No layout change to undo.".to_owned()),
        }
    }

    /// Returns the items the graph is currently drawing, after state and tag filters.
    fn visible_items(&self) -> Vec<StoredItem> {
        let tag_filter = self.tag_filter.to_lowercase();
        self.items
            .iter()
            .filter(|item| {
                (self.show_completed
                    || (item.item.state != ItemState::Done
                        && item.item.state != ItemState::Canceled))
                    && (tag_filter.is_empty()
                        || item
                            .namespaces
                            .iter()
                            .any(|tag| tag.as_str().to_lowercase().contains(&tag_filter)))
            })
            .cloned()
            .collect()
    }

    /// Re-reads `theme.toml`, so visual tuning does not need a restart.
    ///
    /// Writes the current defaults when no file exists yet, giving the developer a
    /// complete starting point to edit rather than a blank file.
    fn reload_theme(&mut self) {
        if !self.theme_path.exists() {
            match self.write_default_theme() {
                Ok(()) => {
                    self.message = Some(format!("Theme written to {}", self.theme_path.display()));
                }
                Err(error) => {
                    self.message = Some(format!("Theme not written: {error:#}"));
                    return;
                }
            }
        }
        match Theme::load(&self.theme_path) {
            Ok(theme) => {
                self.theme = theme;
                if self.message.is_none() {
                    self.message = Some("Theme reloaded.".to_owned());
                }
            }
            Err(error) => self.message = Some(format!("Theme not loaded: {error:#}")),
        }
    }

    /// Writes the current theme to disk as an editable starting point.
    fn write_default_theme(&self) -> Result<()> {
        if let Some(parent) = self.theme_path.parent() {
            fs::create_dir_all(parent).context("create theme directory")?;
        }
        let contents = self.theme.to_toml()?;
        fs::write(&self.theme_path, contents)
            .with_context(|| format!("write {}", self.theme_path.display()))
    }

    fn refresh(&mut self) {
        match self
            .runtime
            .block_on(self.ledger.list_items(ItemScope::All, true))
        {
            Ok(items) => {
                self.items = items;
                match self.runtime.block_on(self.ledger.namespaces()) {
                    Ok(namespaces) => self.namespaces = namespaces,
                    Err(error) => self.message = Some(format!("Could not load tags: {error:#}")),
                }
                match self.runtime.block_on(self.ledger.focus_sequences()) {
                    Ok(sequences) => self.focus_sequences = sequences,
                    Err(error) => {
                        self.message = Some(format!("Could not load focus sequences: {error:#}"))
                    }
                }
            }
            Err(error) => self.message = Some(format!("Could not refresh ledger: {error:#}")),
        }
    }

    fn create_focus_sequence(&mut self) {
        match self
            .runtime
            .block_on(self.ledger.create_focus_sequence(&self.focus_title_draft))
        {
            Ok(sequence) => {
                self.selected_focus_sequence = Some(sequence.id);
                self.focus_title_draft.clear();
                self.refresh();
            }
            Err(error) => self.message = Some(format!("Focus sequence not created: {error:#}")),
        }
    }

    fn add_selected_to_focus_sequence(&mut self) {
        let (Some(sequence), Some(item)) =
            (self.selected_focus_sequence.clone(), self.selected.clone())
        else {
            return;
        };
        match self
            .runtime
            .block_on(self.ledger.add_focus_sequence_item(&sequence, &item))
        {
            Ok(()) => {
                self.message = Some("Added task to focus sequence.".to_owned());
                self.refresh();
            }
            Err(error) => {
                self.message = Some(format!("Task not added to focus sequence: {error:#}"))
            }
        }
    }

    fn save_layout(&mut self) {
        let layout = LocalGraphLayout {
            node_offsets: self
                .node_offsets
                .iter()
                .map(|(id, offset)| (id.clone(), [offset.x, offset.y]))
                .collect(),
        };
        let result = (|| -> Result<()> {
            let parent = self
                .layout_path
                .parent()
                .context("layout path has no parent")?;
            fs::create_dir_all(parent).context("create graph-layout directory")?;
            let json = serde_json::to_string_pretty(&layout).context("encode graph layout")?;
            fs::write(&self.layout_path, json).context("write graph layout")?;
            Ok(())
        })();
        if let Err(error) = result {
            self.message = Some(format!("Graph layout not saved: {error:#}"));
        }
    }

    fn assigned_tag_paths(&self) -> Vec<String> {
        let mut paths = self
            .items
            .iter()
            .flat_map(|item| item.namespaces.iter().map(ToString::to_string))
            .collect::<Vec<_>>();
        paths.sort();
        paths.dedup();
        paths
    }

    fn selected_item(&self) -> Option<&StoredItem> {
        self.selected.as_ref().and_then(|id| {
            self.items
                .iter()
                .find(|stored| stored.item.id.to_string() == *id)
        })
    }

    fn select(&mut self, id: String) {
        if let Some(item) = self
            .items
            .iter()
            .find(|stored| stored.item.id.to_string() == id)
        {
            self.title_draft = item.item.title.clone();
            self.priority_draft = item.item.priority;
            self.state_draft = item.item.state;
            self.parent_draft = item.item.parent_id.map(|parent| parent.to_string());
            self.due_draft = item
                .item
                .due_on
                .map_or(String::new(), |date| date.to_string());
            self.defer_draft = item
                .item
                .defer_until
                .map_or(String::new(), |date| date.to_string());
            self.scheduled_draft = item
                .item
                .scheduled_on
                .map_or(String::new(), |date| date.to_string());
            self.selected = Some(id);
            match self.runtime.block_on(self.ledger.notes(item.item.id)) {
                Ok(notes) => self.notes = notes,
                Err(error) => self.message = Some(format!("Could not load notes: {error:#}")),
            }
            match self.runtime.block_on(self.ledger.citations(item.item.id)) {
                Ok(citations) => self.citations = citations,
                Err(error) => self.message = Some(format!("Could not load links: {error:#}")),
            }
            match self.runtime.block_on(self.ledger.sessions(item.item.id)) {
                Ok(sessions) => self.sessions = sessions,
                Err(error) => self.message = Some(format!("Could not load sessions: {error:#}")),
            }
            let raw_value = serde_json::json!({
                "id": item.item.id,
                "title": item.item.title,
                "state": item.item.state.to_string(),
                "priority": item.item.priority.to_string(),
                "tags": item.namespaces.iter().map(|tag| tag.as_str()).collect::<Vec<_>>(),
                "parent": item.item.parent_id,
                "depends_on": item.dependencies,
                "blocked_by": item.blocked_by,
                "is_blocked": !item.blocked_by.is_empty(),
            });
            self.raw_json =
                serde_json::to_string_pretty(&raw_value).unwrap_or_else(|_| "{}".to_owned());
        }
    }

    fn save_title(&mut self) {
        let Some(id) = self.selected.clone() else {
            return;
        };
        let title = self.title_draft.trim();
        if title.is_empty() {
            self.message = Some("Title cannot be empty.".to_owned());
            return;
        }
        match self.runtime.block_on(self.ledger.edit_title(&id, title)) {
            Ok(_) => {
                self.message = Some("Title saved.".to_owned());
                self.refresh();
            }
            Err(error) => self.message = Some(format!("Title not saved: {error:#}")),
        }
    }

    fn save_priority(&mut self) {
        let Some(id) = self.selected.clone() else {
            return;
        };
        match self
            .runtime
            .block_on(self.ledger.set_priority(&id, self.priority_draft))
        {
            Ok(_) => {
                self.message = Some("Priority saved.".to_owned());
                self.refresh();
            }
            Err(error) => self.message = Some(format!("Priority not saved: {error:#}")),
        }
    }

    fn toggle_complete(&mut self) {
        let Some(item) = self.selected_item() else {
            return;
        };
        let id = item.item.id.to_string();
        let next = if item.item.state == ItemState::Done {
            ItemState::Active
        } else {
            ItemState::Done
        };
        match self.runtime.block_on(self.ledger.set_state(&id, next)) {
            Ok(_) => {
                self.message = Some(if next == ItemState::Done {
                    "Task completed.".to_owned()
                } else {
                    "Task reopened.".to_owned()
                });
                self.refresh();
            }
            Err(error) => self.message = Some(format!("Task state not changed: {error:#}")),
        }
    }

    fn save_state(&mut self) {
        let Some(id) = self.selected.clone() else {
            return;
        };
        match self
            .runtime
            .block_on(self.ledger.set_state(&id, self.state_draft))
        {
            Ok(_) => {
                self.message = Some("State saved.".to_owned());
                self.refresh();
            }
            Err(error) => self.message = Some(format!("State not saved: {error:#}")),
        }
    }

    fn add_task(&mut self) {
        let title = self.new_task_title.trim();
        if title.is_empty() {
            self.message = Some("New task title cannot be empty.".to_owned());
            return;
        }
        match self.runtime.block_on(self.ledger.add_item(title, None)) {
            Ok(item) => {
                let id = item.item.id.to_string();
                self.new_task_title.clear();
                self.new_task_open = false;
                self.message = Some("New inbox task created.".to_owned());
                self.refresh();
                self.select(id);
            }
            Err(error) => self.message = Some(format!("Task not created: {error:#}")),
        }
    }

    fn add_note(&mut self) {
        let Some(id) = self.selected.clone() else {
            return;
        };
        let body = self.note_draft.trim();
        if body.is_empty() {
            self.message = Some("Note cannot be empty.".to_owned());
            return;
        }
        match self.runtime.block_on(self.ledger.add_note(&id, body)) {
            Ok(()) => {
                self.note_draft.clear();
                self.message = Some("Note added.".to_owned());
                if let Some(item) = self.selected_item() {
                    match self.runtime.block_on(self.ledger.notes(item.item.id)) {
                        Ok(notes) => self.notes = notes,
                        Err(error) => {
                            self.message = Some(format!("Could not refresh notes: {error:#}"))
                        }
                    }
                }
            }
            Err(error) => self.message = Some(format!("Note not added: {error:#}")),
        }
    }

    fn add_link(&mut self) {
        let Some(id) = self.selected.clone() else {
            return;
        };
        let target = self.link_target_draft.trim();
        if target.is_empty() {
            self.message = Some("A link needs a URL or file path.".to_owned());
            return;
        }
        let label =
            (!self.link_label_draft.trim().is_empty()).then_some(self.link_label_draft.trim());
        match self
            .runtime
            .block_on(self.ledger.add_citation(&id, target, label))
        {
            Ok(()) => {
                self.link_target_draft.clear();
                self.link_label_draft.clear();
                self.message = Some("Link added.".to_owned());
                if let Some(item) = self.selected_item()
                    && let Ok(citations) =
                        self.runtime.block_on(self.ledger.citations(item.item.id))
                {
                    self.citations = citations;
                }
            }
            Err(error) => self.message = Some(format!("Link not added: {error:#}")),
        }
    }

    fn toggle_session(&mut self) {
        let Some(item) = self.selected_item() else {
            return;
        };
        let id = item.item.id.to_string();
        let active = self
            .sessions
            .iter()
            .any(|session| session.finished_at.is_none());
        let result = if active {
            self.runtime
                .block_on(self.ledger.finish_session(&id, chrono::Utc::now()))
        } else {
            self.runtime
                .block_on(self.ledger.start_session(&id, chrono::Utc::now()))
        };
        match result {
            Ok(_) => {
                self.select(id);
                self.message = Some(
                    if active {
                        "Session stopped."
                    } else {
                        "Session started."
                    }
                    .to_owned(),
                );
            }
            Err(error) => self.message = Some(format!("Session not updated: {error:#}")),
        }
    }

    fn add_tag(&mut self) {
        let Some(id) = self.selected.clone() else {
            return;
        };
        let draft = self.tag_draft.trim();
        let path = if draft.starts_with("tag/") {
            draft.to_owned()
        } else {
            format!("tag/{draft}")
        };
        match path.parse::<NamespacePath>() {
            Ok(tag) => match self
                .runtime
                .block_on(self.ledger.assign_namespace(&id, &tag))
            {
                Ok(()) => {
                    self.tag_draft.clear();
                    self.message = Some("Tag added.".to_owned());
                    self.refresh();
                }
                Err(error) => self.message = Some(format!("Tag not added: {error:#}")),
            },
            Err(error) => self.message = Some(format!("Invalid tag: {error}")),
        }
    }

    fn remove_tag(&mut self, tag: &NamespacePath) {
        let Some(id) = self.selected.clone() else {
            return;
        };
        match self
            .runtime
            .block_on(self.ledger.remove_namespace(&id, tag))
        {
            Ok(()) => {
                self.message = Some("Tag removed.".to_owned());
                self.refresh();
            }
            Err(error) => self.message = Some(format!("Tag not removed: {error:#}")),
        }
    }

    fn save_parent(&mut self) {
        let Some(id) = self.selected.clone() else {
            return;
        };
        let result = match self.parent_draft.clone() {
            Some(parent) => self.runtime.block_on(self.ledger.set_parent(&id, &parent)),
            None => self.runtime.block_on(self.ledger.clear_parent(&id)),
        };
        match result {
            Ok(()) => {
                self.message = Some("Parent relationship saved.".to_owned());
                self.refresh();
            }
            Err(error) => self.message = Some(format!("Parent not saved: {error:#}")),
        }
    }

    fn toggle_blocker(&mut self, blocker: String, enabled: bool) {
        let Some(id) = self.selected.clone() else {
            return;
        };
        let result = if enabled {
            self.runtime
                .block_on(self.ledger.add_blocker(&id, &blocker))
        } else {
            self.runtime
                .block_on(self.ledger.remove_blocker(&id, &blocker))
        };
        match result {
            Ok(()) => {
                self.message = Some("Blocker relationship saved.".to_owned());
                self.refresh();
            }
            Err(error) => self.message = Some(format!("Blocker not saved: {error:#}")),
        }
    }

    fn toggle_dependency(&mut self, dependency: String, enabled: bool) {
        let Some(id) = self.selected.clone() else {
            return;
        };
        let result = if enabled {
            self.runtime
                .block_on(self.ledger.add_dependency(&id, &dependency))
        } else {
            self.runtime
                .block_on(self.ledger.remove_dependency(&id, &dependency))
        };
        match result {
            Ok(()) => {
                self.message = Some("Dependency relationship saved.".to_owned());
                self.refresh();
            }
            Err(error) => self.message = Some(format!("Dependency not saved: {error:#}")),
        }
    }

    fn save_date(&mut self, due: bool) {
        let Some(id) = self.selected.clone() else {
            return;
        };
        let draft = if due {
            &self.due_draft
        } else {
            &self.defer_draft
        };
        let date = match parse_date(draft) {
            Ok(date) => date,
            Err(message) => {
                self.message = Some(message);
                return;
            }
        };
        let result = if due {
            self.runtime.block_on(self.ledger.set_due_on(&id, date))
        } else {
            self.runtime
                .block_on(self.ledger.set_defer_until(&id, date))
        };
        match result {
            Ok(_) => {
                self.message = Some("Date saved.".to_owned());
                self.refresh();
            }
            Err(error) => self.message = Some(format!("Date not saved: {error:#}")),
        }
    }

    fn save_scheduled(&mut self) {
        let Some(id) = self.selected.clone() else {
            return;
        };
        let date = match parse_date(&self.scheduled_draft) {
            Ok(date) => date,
            Err(message) => {
                self.message = Some(message);
                return;
            }
        };
        match self.runtime.block_on(self.ledger.apply_patch(
            &id,
            ItemPatch {
                scheduled_on: Some(date),
                ..ItemPatch::default()
            },
        )) {
            Ok(_) => {
                self.message = Some("Scheduled date saved.".to_owned());
                self.refresh();
            }
            Err(error) => self.message = Some(format!("Scheduled date not saved: {error:#}")),
        }
    }

    fn load_raw_drafts(&mut self) {
        let value = match serde_json::from_str::<serde_json::Value>(&self.raw_json) {
            Ok(value) => value,
            Err(error) => {
                self.message = Some(format!("JSON parse error: {error}"));
                return;
            }
        };
        let Some(title) = value.get("title").and_then(serde_json::Value::as_str) else {
            self.message = Some("Raw JSON needs a string title.".to_owned());
            return;
        };
        let Some(priority) = value.get("priority").and_then(serde_json::Value::as_str) else {
            self.message = Some("Raw JSON needs a string priority.".to_owned());
            return;
        };
        match priority.parse::<Priority>() {
            Ok(priority) => {
                self.title_draft = title.to_owned();
                self.priority_draft = priority;
                self.message =
                    Some("Raw title and priority loaded; review and save each field.".to_owned());
            }
            Err(error) => self.message = Some(format!("Invalid raw priority: {error}")),
        }
    }

    fn apply_raw_patch(&mut self) {
        let Some(id) = self.selected.clone() else {
            return;
        };
        let value: serde_json::Value = match serde_json::from_str(&self.raw_json) {
            Ok(value) => value,
            Err(error) => {
                self.message = Some(format!("JSON parse error: {error}"));
                return;
            }
        };
        let date = |name: &str| -> Result<Option<Option<NaiveDate>>> {
            let Some(value) = value.get(name) else {
                return Ok(None);
            };
            let Some(text) = value.as_str() else {
                bail!("{name} must be a YYYY-MM-DD string or empty");
            };
            Ok(Some(if text.is_empty() {
                None
            } else {
                Some(NaiveDate::parse_from_str(text, "%Y-%m-%d")?)
            }))
        };
        let patch = (|| -> Result<ItemPatch> {
            Ok(ItemPatch {
                title: value
                    .get("title")
                    .map(|v| {
                        v.as_str()
                            .ok_or_else(|| anyhow::anyhow!("title must be a string"))
                            .map(str::to_owned)
                    })
                    .transpose()?,
                priority: value
                    .get("priority")
                    .map(|v| {
                        v.as_str()
                            .ok_or_else(|| anyhow::anyhow!("priority must be a string"))?
                            .parse()
                            .map_err(anyhow::Error::from)
                    })
                    .transpose()?,
                state: value
                    .get("state")
                    .map(|v| {
                        v.as_str()
                            .ok_or_else(|| anyhow::anyhow!("state must be a string"))?
                            .parse()
                            .map_err(anyhow::Error::from)
                    })
                    .transpose()?,
                due_on: date("due_on")?,
                defer_until: date("defer_until")?,
                scheduled_on: date("scheduled_on")?,
            })
        })();
        match patch.and_then(|patch| self.runtime.block_on(self.ledger.apply_patch(&id, patch))) {
            Ok(_) => {
                self.message = Some("Raw JSON patch applied atomically.".to_owned());
                self.refresh();
                self.select(id);
            }
            Err(error) => self.message = Some(format!("Raw JSON not applied: {error:#}")),
        }
    }
}

fn parse_date(value: &str) -> std::result::Result<Option<NaiveDate>, String> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }
    NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map(Some)
        .map_err(|_| "Date must use YYYY-MM-DD, or be empty to clear it.".to_owned())
}

impl eframe::App for TodoGui {
    fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
        if !ui.ctx().egui_wants_keyboard_input() {
            let step = if ui.input(|input| input.modifiers.shift) {
                80.0
            } else {
                24.0
            };
            if ui.input(|input| input.key_pressed(egui::Key::ArrowLeft)) {
                self.graph_pan.x += step;
            }
            if ui.input(|input| input.key_pressed(egui::Key::ArrowRight)) {
                self.graph_pan.x -= step;
            }
            if ui.input(|input| input.key_pressed(egui::Key::ArrowUp)) {
                self.graph_pan.y += step;
            }
            if ui.input(|input| input.key_pressed(egui::Key::ArrowDown)) {
                self.graph_pan.y -= step;
            }
        }
        egui::Panel::top("toolbar").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Todo Ledger");
                if ui.button("New task").clicked() {
                    self.new_task_open = true;
                }
                ui.separator();
                ui.checkbox(&mut self.show_parent_graph, "Parent tasks");
                ui.checkbox(&mut self.show_dependency_graph, "Depends on");
                ui.checkbox(&mut self.show_blocker_graph, "Blocked by");
                ui.checkbox(&mut self.show_tag_constellation, "Tag constellation");
                ui.checkbox(&mut self.show_parent_regions, "Parent regions");
                ui.checkbox(&mut self.show_focus_window, "Focus Window");
                if ui.button("−").clicked() {
                    self.graph_zoom = (self.graph_zoom / 1.2).max(0.4);
                }
                if ui.button("+").clicked() {
                    self.graph_zoom = (self.graph_zoom * 1.2).min(2.5);
                }
                if ui.button("Normalize").clicked() {
                    // The graph rect is only known while drawing, so the request is
                    // deferred one frame rather than guessed at here.
                    self.normalize_requested = true;
                }
                if ui.button("Focus layout").clicked() {
                    self.focus_layout_requested = true;
                }
                if ui.button("Undo layout").clicked() {
                    self.undo_layout();
                }
                if ui.button("Reload theme").clicked() {
                    self.reload_theme();
                }
                if ui.button("Reset view").clicked() {
                    self.graph_zoom = 1.0;
                    self.graph_pan = Vec2::ZERO;
                }
                if ui.button("Refresh").clicked() {
                    self.refresh();
                }
            });
        });
        if self.new_task_open {
            egui::Window::new("New task")
                .collapsible(false)
                .resizable(false)
                .show(ui.ctx(), |ui| {
                    ui.label(
                        "Creates an unscoped inbox task. Assign a project or parent afterward.",
                    );
                    ui.text_edit_singleline(&mut self.new_task_title);
                    let valid = !self.new_task_title.trim().is_empty();
                    ui.horizontal(|ui| {
                        if ui.add_enabled(valid, egui::Button::new("Create")).clicked() {
                            self.add_task();
                        }
                        if ui.button("Cancel").clicked() {
                            self.new_task_open = false;
                            self.new_task_title.clear();
                        }
                    });
                });
        }
        egui::Panel::left("filters")
            .default_size(210.0)
            .show(ui, |ui| {
                ui.heading("Filters");
                ui.checkbox(&mut self.show_completed, "Show completed");
                ui.checkbox(&mut self.show_parent_graph, "Parent task graph");
                ui.checkbox(&mut self.show_dependency_graph, "Depends-on graph");
                ui.checkbox(&mut self.show_blocker_graph, "Blocked-by graph");
                ui.checkbox(&mut self.show_tag_constellation, "Tag constellation");
                ui.checkbox(&mut self.show_focus_window, "Focus Window");
                ui.separator();
                ui.label("Tags");
                ui.text_edit_singleline(&mut self.tag_filter);
                if ui.button("Clear tag filter").clicked() {
                    self.tag_filter.clear();
                }
                egui::ScrollArea::vertical().show(ui, |ui| {
                    let query = self.tag_filter.to_lowercase();
                    let matches = self
                        .assigned_tag_paths()
                        .into_iter()
                        .filter(|path| query.is_empty() || path.to_lowercase().contains(&query))
                        .take(5)
                        .collect::<Vec<_>>();
                    for path in matches {
                        if ui.small_button(&path).clicked() {
                            self.tag_filter = path;
                        }
                    }
                });
            });
        egui::Panel::right("inspector")
            .default_size(280.0)
            .show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                ui.heading("Inspector");
                if let Some(item) = self.selected_item().cloned() {
                    ui.label(format!("{} · {}", item.item.state, item.item.id));
                    ui.separator();
                    ui.label("Title");
                    ui.text_edit_singleline(&mut self.title_draft);
                    let title_valid = !self.title_draft.trim().is_empty();
                    if !title_valid {
                        ui.colored_label(Color32::RED, "Title cannot be empty.");
                    }
                    if ui
                        .add_enabled(title_valid, egui::Button::new("Save title"))
                        .clicked()
                    {
                        self.save_title();
                    }
                    ui.separator();
                    ui.label("Priority");
                    egui::ComboBox::from_id_salt("priority")
                        .selected_text(self.priority_draft.to_string())
                        .show_ui(ui, |ui| {
                            for priority in [
                                Priority::Low,
                                Priority::Normal,
                                Priority::High,
                                Priority::Critical,
                            ] {
                                ui.selectable_value(
                                    &mut self.priority_draft,
                                    priority,
                                    priority.to_string(),
                                );
                            }
                        });
                    if ui.button("Save priority").clicked() {
                        self.save_priority();
                    }
                    ui.separator();
                    ui.label("Status");
                    egui::ComboBox::from_id_salt("state")
                        .selected_text(self.state_draft.to_string())
                        .show_ui(ui, |ui| {
                            for state in [
                                ItemState::Inbox,
                                ItemState::Active,
                                ItemState::Done,
                                ItemState::Canceled,
                            ]
                            .into_iter()
                            .filter(|state| item.item.state.can_transition_to(*state))
                            {
                                ui.selectable_value(&mut self.state_draft, state, state.to_string());
                            }
                        });
                    if ui.button("Save status").clicked() { self.save_state(); }
                    ui.separator();
                    if ui
                        .button(if item.item.state == ItemState::Done {
                            "Reopen task"
                        } else {
                            "Complete task"
                        })
                        .clicked()
                    {
                        self.toggle_complete();
                    }
                    ui.separator();
                    ui.label("Tags");
                    for tag in &item.namespaces {
                        ui.horizontal(|ui| {
                            ui.monospace(tag.as_str());
                            if ui.small_button("Remove").clicked() {
                                self.remove_tag(tag);
                            }
                        });
                    }
                    if item.namespaces.is_empty() {
                        ui.weak("No tags");
                    }
                    ui.horizontal(|ui| {
                        ui.text_edit_singleline(&mut self.tag_draft);
                        if ui.button("Add tag").clicked() {
                            self.add_tag();
                        }
                    });
                    let tag_query = self.tag_draft.to_lowercase();
                    let tag_matches = self
                        .assigned_tag_paths()
                        .into_iter()
                        .filter(|path| {
                            !item.namespaces.iter().any(|tag| tag.as_str() == path)
                                && (tag_query.is_empty()
                                    || path.to_lowercase().contains(&tag_query))
                        })
                        .take(5)
                        .collect::<Vec<_>>();
                    for namespace in tag_matches {
                        if ui.small_button(format!("Add {namespace}")).clicked() {
                            self.tag_draft = namespace;
                            self.add_tag();
                        }
                    }
                    ui.separator();
                    ui.label("Parent task");
                    let candidates = self
                        .items
                        .iter()
                        .filter(|candidate| {
                            candidate.item.id != item.item.id
                                && candidate.item.project_id == item.item.project_id
                        })
                        .map(|candidate| (candidate.item.id.to_string(), candidate.item.title.clone()))
                        .collect::<Vec<_>>();
                    if let Some(parent) = self.parent_draft.clone() {
                        ui.horizontal(|ui| {
                            ui.monospace(parent);
                            if ui.small_button("Remove").clicked() {
                                self.parent_draft = None;
                                self.save_parent();
                            }
                        });
                    }
                    ui.horizontal(|ui| {
                        ui.text_edit_singleline(&mut self.parent_search);
                        if ui.button("Clear parent").clicked() {
                            self.parent_draft = None;
                            self.save_parent();
                        }
                    });
                    let parent_query = self.parent_search.to_lowercase();
                    for (id, title) in candidates.into_iter().filter(|(_, title)| {
                        parent_query.is_empty() || title.to_lowercase().contains(&parent_query)
                    }).take(5) {
                        if ui.small_button(format!("Set parent: {title}")).clicked() {
                            self.parent_draft = Some(id);
                            self.save_parent();
                        }
                    }
                    ui.label("Depends on (planned prerequisite)");
                    let relation_candidates = self
                        .items
                        .iter()
                        .filter(|candidate| {
                            candidate.item.id != item.item.id
                                && candidate.item.project_id == item.item.project_id
                        })
                        .map(|candidate| (candidate.item.id.to_string(), candidate.item.title.clone()))
                        .collect::<Vec<_>>();
                    for dependency in &item.dependencies {
                        ui.horizontal(|ui| {
                            ui.monospace(dependency.to_string());
                            if ui.small_button("Remove").clicked() {
                                self.toggle_dependency(dependency.to_string(), false);
                            }
                        });
                    }
                    ui.text_edit_singleline(&mut self.dependency_search);
                    let dependency_query = self.dependency_search.to_lowercase();
                    for (id, title) in relation_candidates.iter().filter(|(id, title)| {
                        !item.dependencies.iter().any(|dependency| dependency.to_string() == *id)
                            && (dependency_query.is_empty() || title.to_lowercase().contains(&dependency_query))
                    }).take(5) {
                        if ui.small_button(format!("Add dependency: {title}")).clicked() {
                            self.toggle_dependency(id.clone(), true);
                        }
                    }
                    ui.separator();
                    ui.label("Blocked by (active impediment)");
                    if !item.blocked_by.is_empty() {
                        ui.colored_label(Color32::from_rgb(228, 126, 94), "This task is currently blocked.");
                    }
                    for blocker in &item.blocked_by {
                        ui.horizontal(|ui| {
                            ui.monospace(blocker.to_string());
                            if ui.small_button("Remove").clicked() {
                                self.toggle_blocker(blocker.to_string(), false);
                            }
                        });
                    }
                    ui.text_edit_singleline(&mut self.blocker_search);
                    let blocker_query = self.blocker_search.to_lowercase();
                    for (id, title) in relation_candidates.into_iter().filter(|(id, title)| {
                        !item.blocked_by.iter().any(|blocker| blocker.to_string() == *id)
                            && (blocker_query.is_empty() || title.to_lowercase().contains(&blocker_query))
                    }).take(5) {
                        if ui.small_button(format!("Add active blocker: {title}")).clicked() {
                            self.toggle_blocker(id, true);
                        }
                    }
                    ui.separator();
                    ui.label("Focus sequence");
                    ui.horizontal(|ui| {
                        ui.text_edit_singleline(&mut self.focus_title_draft);
                        if ui.button("New sequence").clicked() {
                            self.create_focus_sequence();
                        }
                    });
                    for sequence in &self.focus_sequences {
                        if ui
                            .selectable_label(
                                self.selected_focus_sequence.as_deref() == Some(sequence.id.as_str()),
                                format!("{} ({})", sequence.title, sequence.items.len()),
                            )
                            .clicked()
                        {
                            self.selected_focus_sequence = Some(sequence.id.clone());
                        }
                    }
                    if self.selected_focus_sequence.is_some()
                        && ui.button("Add this task to selected sequence").clicked()
                    {
                        self.add_selected_to_focus_sequence();
                    }
                    ui.separator();
                    ui.label("Dates");
                    ui.horizontal(|ui| {
                        ui.label("Due");
                        ui.text_edit_singleline(&mut self.due_draft);
                        if ui.small_button("Today").clicked() { self.due_draft = Local::now().date_naive().to_string(); }
                        if ui.button("Save due").clicked() { self.save_date(true); }
                    });
                    ui.horizontal(|ui| {
                        ui.label("Defer");
                        ui.text_edit_singleline(&mut self.defer_draft);
                        if ui.small_button("Today").clicked() { self.defer_draft = Local::now().date_naive().to_string(); }
                        if ui.button("Save defer").clicked() { self.save_date(false); }
                    });
                    ui.horizontal(|ui| {
                        ui.label("Scheduled");
                        ui.text_edit_singleline(&mut self.scheduled_draft);
                        if ui.small_button("Today").clicked() { self.scheduled_draft = Local::now().date_naive().to_string(); }
                        if ui.button("Save scheduled").clicked() { self.save_scheduled(); }
                    });
                    ui.separator();
                    ui.label("Work sessions");
                    let active_session = self.sessions.iter().any(|session| session.finished_at.is_none());
                    if ui.button(if active_session { "Stop session" } else { "Start session" }).clicked() {
                        self.toggle_session();
                    }
                    for session in &self.sessions {
                        ui.weak(format!("{} → {}", session.started_at, session.finished_at.as_deref().unwrap_or("active")));
                    }
                    ui.separator();
                    ui.label("Notes / description");
                    for note in &self.notes {
                        ui.group(|ui| {
                            ui.label(&note.body);
                        });
                    }
                    ui.add(egui::TextEdit::multiline(&mut self.note_draft).desired_rows(4));
                    if ui.button("Add note").clicked() {
                        self.add_note();
                    }
                    ui.separator();
                    ui.label("Linked resources");
                    for citation in &self.citations {
                        ui.horizontal(|ui| {
                            ui.label(citation.label.as_deref().unwrap_or(&citation.target));
                            if ui.small_button("Open").clicked() {
                                ui.ctx().open_url(egui::OpenUrl::new_tab(&citation.target));
                            }
                        });
                    }
                    ui.text_edit_singleline(&mut self.link_target_draft);
                    ui.text_edit_singleline(&mut self.link_label_draft);
                    if ui.button("Add link").clicked() { self.add_link(); }
                    ui.separator();
                    ui.label("Raw JSON (prototype)");
                    ui.add(egui::TextEdit::multiline(&mut self.raw_json).desired_rows(12));
                    if ui.button("Parse JSON").clicked() {
                        self.message = match serde_json::from_str::<serde_json::Value>(&self.raw_json) {
                            Ok(_) => Some("JSON parses. Applying raw JSON is intentionally deferred until ItemPatch is atomic.".to_owned()),
                            Err(error) => Some(format!("JSON parse error: {error}")),
                        };
                    }
                    if ui.button("Apply atomic patch").clicked() {
                        self.apply_raw_patch();
                    }
                    if ui.button("Load title and priority").clicked() {
                        self.load_raw_drafts();
                    }
                } else {
                    ui.weak("Select a task node to inspect it.");
                }
                if let Some(message) = &self.message {
                    ui.separator();
                    ui.label(message);
                }
                });
            });
        egui::CentralPanel::default().show(ui, |ui| draw_graph(ui, self));
    }
}

fn draw_graph(ui: &mut egui::Ui, app: &mut TodoGui) {
    let (rect, response) = ui.allocate_exact_size(ui.available_size(), Sense::drag());
    if ui.input(|input| input.pointer.middle_down()) {
        app.graph_pan += ui.input(|input| input.pointer.delta());
    }
    let painter = ui.painter_at(rect);
    if app.normalize_requested {
        app.normalize_requested = false;
        app.normalize_layout(rect);
    }
    if app.focus_layout_requested {
        app.focus_layout_requested = false;
        app.focus_layout(rect);
    }
    let items = app.visible_items();
    // The wheel divides hues between the tags actually on screen, so the palette is
    // rebuilt whenever the visible set changes.
    let palette = match app.theme.tag.coloring {
        TagColoring::Wheel => TagPalette::build(
            items
                .iter()
                .flat_map(|item| item.namespaces.iter().map(NamespacePath::as_str)),
            &app.theme.tag,
        ),
        TagColoring::GroupHash => TagPalette::default(),
    };
    let positions = graph_positions(
        &items,
        rect,
        &app.node_offsets,
        app.graph_pan,
        app.graph_zoom,
    );
    let show_hierarchy = app.show_parent_graph;
    let show_dependencies = app.show_dependency_graph;
    let show_blockers = app.show_blocker_graph;
    if app.show_parent_regions && app.theme.parent.show_regions {
        draw_parent_regions(
            &painter,
            &items,
            &positions,
            app.graph_zoom,
            &app.theme,
            &palette,
        );
    }
    if app.show_tag_constellation {
        draw_tag_constellation(
            &painter,
            &items,
            &positions,
            app.graph_zoom,
            &app.theme,
            &palette,
        );
    }
    if app.show_focus_window && app.theme.focus.show_frame {
        let active = items
            .iter()
            .filter(|item| item.item.state == ItemState::Active)
            .filter_map(|item| positions.get(&item.item.id.to_string()).copied())
            .collect::<Vec<_>>();
        if let Some(bounds) = bounds_for_points(&active, Vec2::new(105.0, 55.0) * app.graph_zoom) {
            painter.rect_stroke(
                bounds.expand(26.0),
                12.0,
                Stroke::new(2.0, Color32::from_rgb(224, 188, 73)),
                egui::StrokeKind::Outside,
            );
            painter.text(
                bounds.left_top() - Vec2::new(0.0, 12.0),
                Align2::LEFT_BOTTOM,
                "FOCUS WINDOW · active now",
                FontId::proportional(12.0),
                Color32::from_rgb(224, 188, 73),
            );
        }
    }
    let mut clicked = None;
    for item in &items {
        let Some(to) = positions.get(&item.item.id.to_string()) else {
            continue;
        };
        if show_hierarchy
            && let Some(parent) = item
                .item
                .parent_id
                .and_then(|id| positions.get(&id.to_string()))
        {
            let (start, end) = edge_points(*parent, *to, app.graph_zoom);
            draw_edge(
                &painter,
                start,
                end,
                Color32::from_rgb(82, 156, 112),
                &app.theme,
                app.graph_zoom,
            );
        }
        if show_dependencies {
            for dependency in &item.dependencies {
                if let Some(from) = positions.get(&dependency.to_string()) {
                    let (start, end) = edge_points(*from, *to, app.graph_zoom);
                    draw_edge(
                        &painter,
                        start,
                        end,
                        Color32::from_rgb(75, 142, 210),
                        &app.theme,
                        app.graph_zoom,
                    );
                }
            }
        }
        if show_blockers {
            for blocker in &item.blocked_by {
                if let Some(from) = positions.get(&blocker.to_string()) {
                    let (start, end) = edge_points(*from, *to, app.graph_zoom);
                    draw_edge(
                        &painter,
                        start,
                        end,
                        Color32::from_rgb(218, 83, 73),
                        &app.theme,
                        app.graph_zoom,
                    );
                }
            }
        }
    }
    for item in &items {
        let id = item.item.id.to_string();
        let Some(center) = positions.get(&id) else {
            continue;
        };
        let (node_width, node_height) = app.theme.node.size();
        let node =
            Rect::from_center_size(*center, Vec2::new(node_width, node_height) * app.graph_zoom);
        let response = ui.interact(node, ui.make_persistent_id(&id), Sense::click_and_drag());
        let selected = app.selected.as_deref() == Some(id.as_str());
        let fill = if !item.blocked_by.is_empty() {
            Color32::from_rgb(119, 57, 51)
        } else {
            match item.item.state {
                ItemState::Active => Color32::from_rgb(47, 104, 72),
                ItemState::Done => Color32::from_gray(80),
                ItemState::Canceled => Color32::from_rgb(100, 61, 61),
                ItemState::Inbox => Color32::from_rgb(59, 79, 94),
            }
        };
        let accent = node_tag_color(item, &app.theme, &palette);
        let outline = Stroke::new(
            if selected { 3.0 } else { 1.0 },
            if selected {
                Color32::GOLD
            } else {
                Color32::LIGHT_GRAY
            },
        );
        draw_node_body(
            &painter,
            node,
            fill,
            outline,
            accent,
            &app.theme,
            app.graph_zoom,
        );
        let title_font = FontId::proportional(app.theme.node.title_font_size * app.graph_zoom);
        if app.theme.node.show_meta_line {
            painter.text(
                node.center_top() + Vec2::new(0.0, 12.0 * app.graph_zoom),
                Align2::CENTER_TOP,
                short_title(&item.item.title),
                title_font,
                Color32::WHITE,
            );
            painter.text(
                node.center_bottom() - Vec2::new(0.0, 10.0 * app.graph_zoom),
                Align2::CENTER_BOTTOM,
                format!("{} · {}", item.item.state, item.item.priority),
                FontId::proportional(app.theme.node.meta_font_size * app.graph_zoom),
                Color32::LIGHT_GRAY,
            );
        } else {
            painter.text(
                node.center(),
                Align2::CENTER_CENTER,
                short_title(&item.item.title),
                title_font,
                Color32::WHITE,
            );
        }
        if response.drag_started()
            && let Some(pointer) = response.interact_pointer_pos()
        {
            let offset = app.node_offsets.get(&id).copied().unwrap_or(Vec2::ZERO);
            app.drag_anchor = Some((id.clone(), pointer, offset));
        }
        if let Some((dragged, pointer_start, offset_start)) = app.drag_anchor.clone()
            && dragged == id
            && let Some(pointer) = response.interact_pointer_pos()
        {
            let dragged_distance = (pointer - pointer_start) / app.graph_zoom.max(f32::EPSILON);
            app.node_offsets
                .insert(id.clone(), offset_start + dragged_distance);
        }
        if response.drag_stopped() {
            app.drag_anchor = None;
            app.save_layout();
        }
        if response.clicked() {
            clicked = Some(id.clone());
        }
    }
    if let Some(id) = clicked {
        app.select(id);
    }
    if response.clicked() {
        app.drag_anchor = None;
    }
}

/// Returns the unpositioned grid slot an item occupies before any offset.
///
/// Normalizing works by moving a node from this slot to a computed one, so the
/// base grid and the normalized layout share one coordinate space.
fn grid_slot(index: usize, rect: Rect) -> Pos2 {
    let width = (rect.width() - 190.0).max(1.0);
    let column = index % 4;
    let row = index / 4;
    let column_offset = match column {
        0 => 0.0,
        1 => 1.0,
        2 => 2.0,
        _ => 3.0,
    };
    let row_offset = u16::try_from(row).map_or(f32::MAX, f32::from);
    Pos2::new(
        rect.left() + 95.0 + column_offset * (width / 3.0),
        rect.top() + 70.0 + row_offset * 105.0,
    )
}

fn graph_positions(
    items: &[StoredItem],
    rect: Rect,
    offsets: &HashMap<String, Vec2>,
    pan: Vec2,
    zoom: f32,
) -> HashMap<String, Pos2> {
    let mut positions = HashMap::new();
    for (index, item) in items.iter().enumerate() {
        let id = item.item.id.to_string();
        let position = grid_slot(index, rect);
        // Offsets live in graph units, before zoom, so a normalized or dragged
        // arrangement keeps its shape when the graph is zoomed.
        let position = position + offsets.get(&id).copied().unwrap_or(Vec2::ZERO);
        let position = rect.center() + (position - rect.center()) * zoom + pan;
        positions.insert(id, position);
    }
    positions
}

fn edge_points(from: Pos2, to: Pos2, zoom: f32) -> (Pos2, Pos2) {
    let delta = to - from;
    let length = delta.length();
    if length <= f32::EPSILON {
        return (from, to);
    }
    let direction = delta / length;
    let half = Vec2::new(75.0, 29.0) * zoom;
    let from_scale = (half.x / direction.x.abs()).min(half.y / direction.y.abs());
    let to_scale = (half.x / direction.x.abs()).min(half.y / direction.y.abs());
    let start = from + direction * from_scale;
    let end = to - direction * (to_scale + 5.0);
    (start, end)
}

/// Draws a node's body in the configured shape, with its tag accent.
///
/// A rectangular node carries the accent as a stripe on its leading edge; a
/// circular node carries it as a ring, since a stripe reads poorly on a curve.
fn draw_node_body(
    painter: &egui::Painter,
    node: Rect,
    fill: Color32,
    outline: Stroke,
    accent: Option<Color32>,
    theme: &Theme,
    zoom: f32,
) {
    let rounding = theme.node.rounding() * zoom;
    match theme.node.shape {
        NodeShape::Circle => {
            let radius = node.width().min(node.height()) / 2.0;
            painter.circle_filled(node.center(), radius, fill);
            if let Some(accent) = accent {
                painter.circle_stroke(
                    node.center(),
                    radius - theme.tag.node_stripe_width * zoom / 2.0,
                    Stroke::new(theme.tag.node_stripe_width * zoom, accent),
                );
            }
            painter.circle_stroke(node.center(), radius, outline);
        }
        NodeShape::RoundedRect | NodeShape::Capsule => {
            painter.rect_filled(node, rounding, fill);
            if let Some(accent) = accent {
                let stripe = theme.tag.node_stripe_width * zoom;
                painter.rect_filled(
                    Rect::from_min_max(
                        node.left_top(),
                        egui::pos2(node.left() + stripe, node.bottom()),
                    ),
                    rounding,
                    accent,
                );
            }
            painter.rect_stroke(node, rounding, outline, egui::StrokeKind::Outside);
        }
    }
}

/// Draws one relationship edge with an arrowhead of theme-controlled size.
///
/// The head size does not follow the edge length, unlike the built-in arrow
/// painter, so long and short edges are tipped identically.
fn draw_edge(
    painter: &egui::Painter,
    start: Pos2,
    end: Pos2,
    color: Color32,
    theme: &Theme,
    zoom: f32,
) {
    let (head_length, head_width) = theme.edge.arrowhead_size(zoom);
    let stroke = Stroke::new(theme.edge.width, color);
    painter.line_segment(
        [start, geometry::shaft_end(start, end, head_length)],
        stroke,
    );
    if let Some(head) = geometry::arrowhead_points(start, end, head_length, head_width) {
        painter.add(egui::Shape::convex_polygon(
            head.to_vec(),
            color,
            Stroke::NONE,
        ));
    }
}

/// Draws a containment region around each parent task and its descendants.
///
/// A parent's region wraps the whole subtree, not just its direct children, so a
/// deep branch reads as one body of work. Regions are drawn largest first so a
/// nested subtree stays visible inside its ancestor. The color comes from the
/// parent's own tag, falling back to a neutral outline when it has no tag, so the
/// region never implies a tag the parent does not carry.
fn draw_parent_regions(
    painter: &egui::Painter,
    items: &[StoredItem],
    positions: &HashMap<String, Pos2>,
    zoom: f32,
    theme: &Theme,
    palette: &TagPalette,
) {
    let mut children: HashMap<String, Vec<String>> = HashMap::new();
    for item in items {
        if let Some(parent) = item.item.parent_id {
            children
                .entry(parent.to_string())
                .or_default()
                .push(item.item.id.to_string());
        }
    }
    let half_size = {
        let (width, height) = theme.node.size();
        Vec2::new(width / 2.0, height / 2.0) * zoom
    };
    let mut regions = Vec::new();
    for item in items {
        let id = item.item.id.to_string();
        let subtree = collect_subtree(&id, &children);
        if subtree.len() <= theme.parent.min_descendants {
            continue;
        }
        let points = subtree
            .iter()
            .filter_map(|member| positions.get(member).copied())
            .collect::<Vec<_>>();
        if points.len() < 2 {
            continue;
        }
        let color = node_tag_color(item, theme, palette).unwrap_or(Color32::from_gray(150));
        regions.push((subtree.len(), points, color, item.item.title.clone()));
    }
    regions.sort_by_key(|(size, ..)| std::cmp::Reverse(*size));
    for (_, points, color, title) in regions {
        let corners = node_corners(&points, half_size);
        let hull = geometry::convex_hull(&corners);
        let padded = geometry::expand_path(&hull, theme.parent.padding);
        let outline = geometry::smooth_closed_path(&padded, theme.parent.smoothing);
        if outline.len() < 3 {
            continue;
        }
        let fill = Color32::from_rgba_unmultiplied(
            color.r(),
            color.g(),
            color.b(),
            theme.parent.fill_alpha,
        );
        let stroke = Stroke::new(
            theme.parent.stroke_width,
            Color32::from_rgba_unmultiplied(
                color.r(),
                color.g(),
                color.b(),
                theme.parent.stroke_alpha,
            ),
        );
        painter.add(egui::Shape::convex_polygon(outline.clone(), fill, stroke));
        if let Some(label_anchor) = outline.iter().min_by(|left, right| {
            (left.y, left.x)
                .partial_cmp(&(right.y, right.x))
                .unwrap_or(std::cmp::Ordering::Equal)
        }) {
            painter.text(
                *label_anchor - Vec2::new(0.0, 4.0),
                Align2::CENTER_BOTTOM,
                short_title(&title),
                FontId::proportional(11.0),
                color,
            );
        }
    }
}

/// Returns a task's id together with every visible descendant beneath it.
fn collect_subtree(root: &str, children: &HashMap<String, Vec<String>>) -> Vec<String> {
    let mut found = vec![root.to_owned()];
    let mut pending = vec![root.to_owned()];
    while let Some(current) = pending.pop() {
        let Some(next) = children.get(&current) else {
            continue;
        };
        for child in next {
            if found.contains(child) {
                continue;
            }
            found.push(child.clone());
            pending.push(child.clone());
        }
    }
    found
}

fn draw_tag_constellation(
    painter: &egui::Painter,
    items: &[StoredItem],
    positions: &HashMap<String, Pos2>,
    zoom: f32,
    theme: &Theme,
    palette: &TagPalette,
) {
    let mut groups: HashMap<String, Vec<Pos2>> = HashMap::new();
    for item in items {
        let Some(position) = positions.get(&item.item.id.to_string()).copied() else {
            continue;
        };
        for tag in &item.namespaces {
            let segments = tag.as_str().split('/').collect::<Vec<_>>();
            for depth in 2..=segments.len() {
                if let Some(prefix) = segments.get(..depth) {
                    groups.entry(prefix.join("/")).or_default().push(position);
                }
            }
        }
    }
    let mut paths = groups.keys().cloned().collect::<Vec<_>>();
    paths.sort_by_key(|path| path.matches('/').count());
    for path in paths {
        let Some(points) = groups.get(&path) else {
            continue;
        };
        let half_size = Vec2::new(92.0, 38.0) * zoom;
        let Some(bounds) = bounds_for_points(points, half_size) else {
            continue;
        };
        let color = palette.color(&path, &theme.tag);
        let fill = Color32::from_rgba_unmultiplied(
            color.r(),
            color.g(),
            color.b(),
            theme.tag.region_fill_alpha,
        );
        let stroke = Stroke::new(
            1.0,
            Color32::from_rgba_unmultiplied(
                color.r(),
                color.g(),
                color.b(),
                theme.tag.region_stroke_alpha,
            ),
        );
        if theme.tag.region_shape == RegionShape::Hull {
            let corners = node_corners(points, half_size);
            let hull = geometry::convex_hull(&corners);
            let padded = geometry::expand_path(&hull, theme.tag.region_padding);
            let outline = geometry::smooth_closed_path(&padded, theme.tag.region_smoothing);
            if outline.len() >= 3 {
                painter.add(egui::Shape::convex_polygon(outline, fill, stroke));
                painter.text(
                    bounds.left_top() + Vec2::new(5.0, 4.0),
                    Align2::LEFT_TOP,
                    path,
                    FontId::proportional(11.0),
                    color,
                );
                continue;
            }
        }
        painter.rect_filled(bounds.expand(16.0), 18.0, fill);
        painter.rect_stroke(bounds.expand(16.0), 18.0, stroke, egui::StrokeKind::Outside);
        painter.text(
            bounds.left_top() + Vec2::new(5.0, 4.0),
            Align2::LEFT_TOP,
            path,
            FontId::proportional(11.0),
            color,
        );
    }
}

/// Returns the corner points of every node box, for hulls that wrap whole nodes.
///
/// Hulling node centers alone would clip the nodes themselves, so the hull is
/// built from the boxes the nodes occupy.
fn node_corners(points: &[Pos2], half_size: Vec2) -> Vec<Pos2> {
    let mut corners = Vec::with_capacity(points.len() * 4);
    for point in points {
        corners.push(*point + Vec2::new(-half_size.x, -half_size.y));
        corners.push(*point + Vec2::new(half_size.x, -half_size.y));
        corners.push(*point + Vec2::new(half_size.x, half_size.y));
        corners.push(*point + Vec2::new(-half_size.x, half_size.y));
    }
    corners
}

fn bounds_for_points(points: &[Pos2], half_size: Vec2) -> Option<Rect> {
    let first = *points.first()?;
    Some(points.iter().skip(1).fold(
        Rect::from_center_size(first, half_size * 2.0),
        |bounds, point| bounds.union(Rect::from_center_size(*point, half_size * 2.0)),
    ))
}

/// Returns a node's tag accent from the item's own deepest tag.
///
/// The accent never comes from a region the node merely sits inside, so an item
/// tagged only `work` keeps the plain `work` color while drawn within a
/// `work/cdint` region. Ties break alphabetically to stay deterministic.
fn node_tag_color(item: &StoredItem, theme: &Theme, palette: &TagPalette) -> Option<Color32> {
    let mut tags = item
        .namespaces
        .iter()
        .map(NamespacePath::as_str)
        .collect::<Vec<_>>();
    tags.sort_unstable();
    let deepest = tags
        .into_iter()
        .max_by_key(|tag| tag.matches('/').count())?;
    Some(palette.color(deepest, &theme.tag))
}

fn load_layout(path: &std::path::Path) -> HashMap<String, Vec2> {
    fs::read_to_string(path)
        .ok()
        .and_then(|contents| serde_json::from_str::<LocalGraphLayout>(&contents).ok())
        .map(|layout| {
            layout
                .node_offsets
                .into_iter()
                .map(|(id, [x, y])| (id, Vec2::new(x, y)))
                .collect()
        })
        .unwrap_or_default()
}

fn short_title(title: &str) -> String {
    title.chars().take(22).collect()
}

#[cfg(test)]
mod tests {
    use super::collect_subtree;
    use std::collections::HashMap;

    fn tree(pairs: &[(&str, &[&str])]) -> HashMap<String, Vec<String>> {
        pairs
            .iter()
            .map(|(parent, children)| {
                (
                    (*parent).to_owned(),
                    children.iter().map(|child| (*child).to_owned()).collect(),
                )
            })
            .collect()
    }

    #[test]
    fn a_subtree_includes_every_descendant() {
        let children = tree(&[("root", &["a", "b"]), ("a", &["deep"])]);
        let mut found = collect_subtree("root", &children);
        found.sort();
        assert_eq!(found, vec!["a", "b", "deep", "root"]);
    }

    #[test]
    fn a_leaf_is_its_own_subtree() {
        let children = tree(&[("root", &["leaf"])]);
        assert_eq!(collect_subtree("leaf", &children), vec!["leaf"]);
    }

    #[test]
    fn a_parent_cycle_does_not_loop_forever() {
        let children = tree(&[("a", &["b"]), ("b", &["a"])]);
        let mut found = collect_subtree("a", &children);
        found.sort();
        assert_eq!(found, vec!["a", "b"]);
    }
}
