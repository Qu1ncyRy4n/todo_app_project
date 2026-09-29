//! Native graph editor for the local Todo Ledger.

mod geometry;
mod tags;
mod theme;

use anyhow::{Context, Result, bail};
use chrono::{Datelike, Local, NaiveDate};
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, Vec2};
use geometry::{BandSpacing, FlowSpacing, OverlapSpacing, SeparationSpacing, TimelineSpacing};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, path::PathBuf};
use tags::{TagFilter, TagFilterState};
use theme::TagInheritance;
use theme::{
    InspectorTheme, NodeShape, ParentRegionStyle, RegionShape, TagColoring, TagPalette, Theme,
};
use todo_core::{ItemId, ItemState, NamespacePath, Priority, RelationKind, WeightInputs};
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
    show_relations: bool,
    relation_draft: RelationKind,
    relation_search: String,
    show_focus_window: bool,
    show_completed: bool,
    tag_filter: String,
    tag_states: TagFilter,
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
    difficulty_draft: String,
    estimate_draft: String,
    graph_pan: Vec2,
    graph_zoom: f32,
    theme: Theme,
    theme_path: PathBuf,
    inspector: InspectorTheme,
    layout_undo: Option<HashMap<String, Vec2>>,
    normalize_requested: bool,
    focus_layout_requested: bool,
    separate_requested: bool,
    relax_requested: bool,
    relax_overlaps: bool,
    focus_mode: bool,
    focus_only: bool,
    drawn_offsets: HashMap<String, Vec2>,
    timeline_requested: bool,
    show_timeline_ruler: bool,
    timeline_origin: Option<Pos2>,
    new_task_open: bool,
    new_task_title: String,
    new_task_parent: Option<String>,
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
        let relax_after_layout = theme.layout.relax_overlaps_after_layout;
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
            show_relations: true,
            relation_draft: RelationKind::ContributesTo,
            relation_search: String::new(),
            show_focus_window: true,
            show_completed: false,
            tag_filter: String::new(),
            tag_states: TagFilter::default(),
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
            difficulty_draft: String::new(),
            estimate_draft: String::new(),
            graph_pan: Vec2::ZERO,
            graph_zoom: 1.0,
            inspector: theme.inspector,
            theme,
            theme_path,
            layout_undo: None,
            normalize_requested: false,
            focus_layout_requested: false,
            separate_requested: false,
            relax_requested: false,
            relax_overlaps: relax_after_layout,
            focus_mode: false,
            focus_only: false,
            drawn_offsets: HashMap::new(),
            timeline_requested: false,
            show_timeline_ruler: true,
            timeline_origin: None,
            new_task_open: false,
            new_task_title: String::new(),
            new_task_parent: None,
            note_draft: String::new(),
            link_target_draft: String::new(),
            link_label_draft: String::new(),
        };
        app.refresh();
        app
    }

    /// Adds a typed relation from the selected task to another task.
    ///
    /// A relation may cross projects, does not imply containment, and never affects
    /// readiness, which is what separates it from a parent or a dependency.
    fn add_relation(&mut self, target: String) {
        let Some(id) = self.selected.clone() else {
            return;
        };
        let relation = self.relation_draft;
        match self
            .runtime
            .block_on(self.ledger.add_relation(&id, relation, &target))
        {
            Ok(_) => {
                self.message = Some(format!("Relation {relation} added."));
                self.refresh();
            }
            Err(error) => self.message = Some(format!("Relation not added: {error:#}")),
        }
    }

    /// Withdraws a typed relation from the selected task.
    fn remove_relation(&mut self, relation: RelationKind, target: String) {
        let Some(id) = self.selected.clone() else {
            return;
        };
        match self
            .runtime
            .block_on(self.ledger.remove_relation(&id, relation, &target))
        {
            Ok(_) => {
                self.message = Some(format!("Relation {relation} removed."));
                self.refresh();
            }
            Err(error) => self.message = Some(format!("Relation not removed: {error:#}")),
        }
    }

    /// Saves the selected task's difficulty, where an empty draft clears it.
    fn save_difficulty(&mut self) {
        let Some(id) = self.selected.clone() else {
            return;
        };
        let parsed = match parse_optional_number::<u8>(&self.difficulty_draft) {
            Ok(parsed) => parsed,
            Err(message) => {
                self.message = Some(message);
                return;
            }
        };
        match self
            .runtime
            .block_on(self.ledger.set_difficulty(&id, parsed))
        {
            Ok(_) => {
                self.message = Some("Difficulty saved.".to_owned());
                self.refresh();
            }
            Err(error) => self.message = Some(format!("Difficulty not saved: {error:#}")),
        }
    }

    /// Saves the selected task's estimate, where an empty draft clears it.
    fn save_estimate(&mut self) {
        let Some(id) = self.selected.clone() else {
            return;
        };
        let parsed = match parse_optional_number::<u32>(&self.estimate_draft) {
            Ok(parsed) => parsed,
            Err(message) => {
                self.message = Some(message);
                return;
            }
        };
        match self.runtime.block_on(self.ledger.set_estimate(&id, parsed)) {
            Ok(_) => {
                self.message = Some("Estimate saved.".to_owned());
                self.refresh();
            }
            Err(error) => self.message = Some(format!("Estimate not saved: {error:#}")),
        }
    }

    /// Draws the one filter sidebar: what is shown, and which tags are in view.
    ///
    /// Every view toggle lives here rather than being split between a toolbar row
    /// and a sidebar, so there is a single place to look for why something is or is
    /// not on screen.
    fn draw_filters(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.heading("Filters");
            egui::CollapsingHeader::new("Tasks shown")
                .default_open(true)
                .show(ui, |ui| {
                    ui.checkbox(&mut self.show_completed, "Completed and canceled");
                    ui.checkbox(&mut self.focus_only, "Active work only");
                    ui.weak("Active work only hides everything that is not active.");
                });
            egui::CollapsingHeader::new("Relationships drawn")
                .default_open(true)
                .show(ui, |ui| {
                    ui.checkbox(&mut self.show_parent_graph, "Parent tasks");
                    ui.checkbox(&mut self.show_dependency_graph, "Depends on");
                    ui.checkbox(&mut self.show_blocker_graph, "Blocked by");
                    ui.checkbox(&mut self.show_relations, "Typed relations");
                });
            egui::CollapsingHeader::new("Overlays")
                .default_open(true)
                .show(ui, |ui| {
                    ui.checkbox(&mut self.show_tag_constellation, "Tag regions");
                    ui.checkbox(&mut self.show_parent_regions, "Parent regions");
                    ui.checkbox(&mut self.show_focus_window, "Focus Window");
                    ui.checkbox(&mut self.show_timeline_ruler, "Timeline ruler");
                });
            egui::CollapsingHeader::new("Layout")
                .default_open(true)
                .show(ui, |ui| {
                    ui.checkbox(&mut self.relax_overlaps, "Push overlaps apart");
                    ui.weak("Runs after every layout command. Unstack runs it now.");
                });
            egui::CollapsingHeader::new("Tags")
                .default_open(true)
                .show(ui, |ui| self.draw_tag_filters(ui));
        });
    }

    /// Draws the tag tree, where each tag cycles neutral, only, then hidden.
    ///
    /// The search box narrows which rows are listed; it does not filter tasks on its
    /// own, so what is shown in the graph is always the tag states and nothing else.
    fn draw_tag_filters(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.text_edit_singleline(&mut self.tag_filter);
            if ui.small_button("Clear").clicked() {
                self.tag_filter.clear();
            }
        });
        if self.tag_states.is_active() && ui.button("Reset tag filters").clicked() {
            self.tag_states.clear();
        }
        let query = self.tag_filter.to_lowercase();
        let assigned = self.assigned_tag_paths();
        let rows = tags::tag_rows(assigned.iter().map(String::as_str));
        for row in rows {
            if !query.is_empty() && !row.path.to_lowercase().contains(&query) {
                continue;
            }
            let state = self.tag_states.state_of(&row.path);
            let inherited = self.tag_states.resolve(&row.path);
            ui.horizontal(|ui| {
                ui.add_space(f32::from(u8::try_from(row.depth).unwrap_or(u8::MAX)) * 12.0);
                let color = match inherited {
                    TagFilterState::Only => Color32::from_rgb(120, 190, 140),
                    TagFilterState::Hidden => Color32::from_rgb(200, 120, 110),
                    TagFilterState::Neutral => Color32::GRAY,
                };
                if ui
                    .small_button(&row.label)
                    .on_hover_text(&row.path)
                    .clicked()
                {
                    self.tag_states.cycle(&row.path);
                }
                // An inherited state is shown in parentheses, so it is clear the row
                // is following its parent rather than holding a state of its own.
                let marker =
                    if state == TagFilterState::Neutral && inherited != TagFilterState::Neutral {
                        format!("({})", inherited.marker())
                    } else {
                        state.marker().to_owned()
                    };
                ui.colored_label(color, marker);
            });
        }
    }

    /// Returns the tags a task carries in its own right.
    fn own_tags(item: &StoredItem) -> Vec<String> {
        item.namespaces
            .iter()
            .map(|tag| tag.as_str().to_owned())
            .collect()
    }

    /// Returns the tags a task inherits from the tasks above it.
    ///
    /// Walks the whole ancestor chain, so a tag set at the top of a tree reaches
    /// every task beneath it. Returns nothing when inheritance is off, and in
    /// explicit mode the tags are offered rather than applied, so the caller shows
    /// them as available instead of carried.
    fn inherited_tags(&self, item: &StoredItem) -> Vec<String> {
        if self.theme.tag.inherit == TagInheritance::Off {
            return Vec::new();
        }
        let mut ancestors = Vec::new();
        let mut current = item.item.parent_id;
        let mut guard = 0;
        while let Some(parent) = current {
            let Some(found) = self
                .items
                .iter()
                .find(|candidate| candidate.item.id == parent)
            else {
                break;
            };
            ancestors.extend(Self::own_tags(found));
            guard += 1;
            if guard > self.items.len() {
                // The ledger rejects parent cycles; a stale view should still stop.
                break;
            }
            current = found.item.parent_id;
        }
        tags::inherited_tags(&Self::own_tags(item), &ancestors)
    }

    /// Returns every tag a task counts as carrying, for filtering and coloring.
    ///
    /// Under implicit inheritance this includes what it inherits; under explicit
    /// inheritance it does not, because there the tags are only on offer.
    fn effective_tags(&self, item: &StoredItem) -> Vec<String> {
        let mut effective = Self::own_tags(item);
        if self.theme.tag.inherit == TagInheritance::Implicit {
            effective.extend(self.inherited_tags(item));
        }
        effective
    }

    /// Returns the effective tags of every visible task, keyed by id.
    fn effective_tag_index(&self, items: &[StoredItem]) -> HashMap<String, Vec<String>> {
        items
            .iter()
            .map(|item| (item.item.id.to_string(), self.effective_tags(item)))
            .collect()
    }

    /// Names another task the way a person would: its title, then a spoken id.
    ///
    /// A raw UUID says nothing about which task it is; the title does, and the
    /// proquint gives something short enough to say out loud when two titles match.
    fn describe(&self, id: &str) -> String {
        let found = self
            .items
            .iter()
            .find(|candidate| candidate.item.id.to_string() == id);
        match found {
            Some(item) => format!("{} · {}", item.item.title, item.item.id.short()),
            None => match ItemId::parse(id) {
                // A referenced task can sit outside the current filter, so fall
                // back to the spoken id rather than showing nothing.
                Ok(parsed) => format!("(not in view) · {}", parsed.short()),
                Err(_) => id.to_owned(),
            },
        }
    }

    /// Draws the inspector as five collapsible sections over the selected task.
    ///
    /// Grouping keeps the panel navigable: status, then how the task relates to
    /// other work, then its content, then its timing, with the raw editing surface
    /// tucked away last. Each group's open state starts from the theme.
    fn draw_inspector(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.heading("Inspector");
            let Some(item) = self.selected_item().cloned() else {
                ui.weak("Select a task node to inspect it.");
                if let Some(message) = self.message.clone() {
                    ui.separator();
                    ui.label(message);
                }
                return;
            };
            ui.label(format!("{} · {}", item.item.state, item.item.id.short()))
                .on_hover_text(item.item.id.to_string());
            egui::CollapsingHeader::new("Status")
                .default_open(self.inspector.open_status)
                .show(ui, |ui| self.section_status(ui, &item));
            egui::CollapsingHeader::new("Relationships")
                .default_open(self.inspector.open_relationships)
                .show(ui, |ui| self.section_relationships(ui, &item));
            egui::CollapsingHeader::new("Details")
                .default_open(self.inspector.open_details)
                .show(ui, |ui| self.section_details(ui, &item));
            egui::CollapsingHeader::new("Schedule")
                .default_open(self.inspector.open_schedule)
                .show(ui, |ui| self.section_schedule(ui, &item));
            egui::CollapsingHeader::new("Advanced")
                .default_open(self.inspector.open_advanced)
                .show(ui, |ui| self.section_advanced(ui));
            egui::CollapsingHeader::new("Section display")
                .default_open(false)
                .show(ui, |ui| self.section_display_settings(ui));
            if let Some(message) = self.message.clone() {
                ui.separator();
                ui.label(message);
            }
        });
    }

    /// Title, priority, and state: what the task is and where it stands.
    fn section_status(&mut self, ui: &mut egui::Ui, item: &StoredItem) {
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
                    ui.selectable_value(&mut self.priority_draft, priority, priority.to_string());
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
        if ui.button("Save status").clicked() {
            self.save_state();
        }
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
    }

    /// Parent, dependencies, blockers, typed relations, and focus sequences.
    fn section_relationships(&mut self, ui: &mut egui::Ui, item: &StoredItem) {
        // Shared by the dependency and blocker subsections, so either can be hidden
        // without taking the candidate list away from the other.
        let relation_candidates = self
            .items
            .iter()
            .filter(|candidate| {
                candidate.item.id != item.item.id
                    && candidate.item.project_id == item.item.project_id
            })
            .map(|candidate| (candidate.item.id.to_string(), candidate.item.title.clone()))
            .collect::<Vec<_>>();
        if ui.button("Create child task").clicked() {
            self.new_task_parent = Some(item.item.id.to_string());
            self.new_task_title.clear();
            self.new_task_open = true;
        }
        ui.separator();
        if self.inspector.show_parent {
            ui.label("Parent task");
            if let Some(parent) = self.parent_draft.clone() {
                let described = self.describe(&parent);
                ui.horizontal(|ui| {
                    ui.label(described);
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
            for (id, title) in relation_candidates
                .iter()
                .filter(|(_, title)| {
                    parent_query.is_empty() || title.to_lowercase().contains(&parent_query)
                })
                .take(5)
            {
                if ui.small_button(format!("Set parent: {title}")).clicked() {
                    self.parent_draft = Some(id.clone());
                    self.save_parent();
                }
            }
        }
        if self.inspector.show_dependencies {
            ui.separator();
            ui.label("Depends on (planned prerequisite)");
            for dependency in &item.dependencies {
                let described = self.describe(&dependency.to_string());
                ui.horizontal(|ui| {
                    ui.label(described);
                    if ui.small_button("Remove").clicked() {
                        self.toggle_dependency(dependency.to_string(), false);
                    }
                });
            }
            ui.text_edit_singleline(&mut self.dependency_search);
            let dependency_query = self.dependency_search.to_lowercase();
            for (id, title) in relation_candidates
                .iter()
                .filter(|(id, title)| {
                    !item
                        .dependencies
                        .iter()
                        .any(|dependency| dependency.to_string() == *id)
                        && (dependency_query.is_empty()
                            || title.to_lowercase().contains(&dependency_query))
                })
                .take(5)
            {
                if ui
                    .small_button(format!("Add dependency: {title}"))
                    .clicked()
                {
                    self.toggle_dependency(id.clone(), true);
                }
            }
        }
        if self.inspector.show_blockers {
            ui.separator();
            ui.label("Blocked by (active impediment)");
            if !item.blocked_by.is_empty() {
                ui.colored_label(
                    Color32::from_rgb(228, 126, 94),
                    "This task is currently blocked.",
                );
            }
            for blocker in &item.blocked_by {
                let described = self.describe(&blocker.to_string());
                ui.horizontal(|ui| {
                    ui.label(described);
                    if ui.small_button("Remove").clicked() {
                        self.toggle_blocker(blocker.to_string(), false);
                    }
                });
            }
            ui.text_edit_singleline(&mut self.blocker_search);
            let blocker_query = self.blocker_search.to_lowercase();
            for (id, title) in relation_candidates
                .iter()
                .filter(|(id, title)| {
                    !item
                        .blocked_by
                        .iter()
                        .any(|blocker| blocker.to_string() == *id)
                        && (blocker_query.is_empty()
                            || title.to_lowercase().contains(&blocker_query))
                })
                .take(5)
            {
                if ui
                    .small_button(format!("Add active blocker: {title}"))
                    .clicked()
                {
                    self.toggle_blocker(id.clone(), true);
                }
            }
        }
        if self.inspector.show_relations {
            ui.separator();
            ui.label("Typed relations (cross-project, never blocking)");
            for relation in &item.relations {
                let existing = *relation;
                let described = self.describe(&existing.target.to_string());
                ui.horizontal(|ui| {
                    ui.label(format!("{} {described}", existing.relation));
                    if ui.small_button("Remove").clicked() {
                        self.remove_relation(existing.relation, existing.target.to_string());
                    }
                });
            }
            ui.horizontal(|ui| {
                for relation in RelationKind::all() {
                    if ui
                        .selectable_label(self.relation_draft == relation, relation.to_string())
                        .clicked()
                    {
                        self.relation_draft = relation;
                    }
                }
            });
            ui.text_edit_singleline(&mut self.relation_search);
            let relation_query = self.relation_search.to_lowercase();
            // Candidates are deliberately not filtered by project: relating across
            // projects is the point of this edge.
            let relation_targets = self
                .items
                .iter()
                .filter(|candidate| candidate.item.id != item.item.id)
                .filter(|candidate| {
                    relation_query.is_empty()
                        || candidate
                            .item
                            .title
                            .to_lowercase()
                            .contains(&relation_query)
                })
                .map(|candidate| (candidate.item.id.to_string(), candidate.item.title.clone()))
                .take(5)
                .collect::<Vec<_>>();
            for (id, title) in relation_targets {
                if ui
                    .small_button(format!("{} {title}", self.relation_draft))
                    .clicked()
                {
                    self.add_relation(id);
                }
            }
        }
        if self.inspector.show_focus_sequence {
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
        }
    }

    /// Tags, notes, and linked resources: what the task is about.
    fn section_details(&mut self, ui: &mut egui::Ui, item: &StoredItem) {
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
        // Inherited tags are shown apart from assigned ones, so it is always clear
        // which tags the task holds in its own right.
        let inherited = self.inherited_tags(&item.clone());
        if !inherited.is_empty() {
            let explicit = self.theme.tag.inherit == TagInheritance::Explicit;
            ui.weak(if explicit {
                "From parent tasks, not yet assigned:"
            } else {
                "From parent tasks:"
            });
            for tag in inherited {
                ui.horizontal(|ui| {
                    ui.weak(format!("({tag})"));
                    if explicit && ui.small_button("Assign").clicked() {
                        self.tag_draft = tag.clone();
                        self.add_tag();
                    }
                });
            }
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
                    && (tag_query.is_empty() || path.to_lowercase().contains(&tag_query))
            })
            .take(5)
            .collect::<Vec<_>>();
        for namespace in tag_matches {
            if ui.small_button(format!("Add {namespace}")).clicked() {
                self.tag_draft = namespace;
                self.add_tag();
            }
        }
        if self.inspector.show_notes {
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
        }
        if self.inspector.show_links {
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
            if ui.button("Add link").clicked() {
                self.add_link();
            }
        }
    }

    /// Dates, sessions, effort, and the weight they feed.
    fn section_schedule(&mut self, ui: &mut egui::Ui, item: &StoredItem) {
        if self.inspector.show_dates {
            ui.horizontal(|ui| {
                ui.label("Due");
                ui.text_edit_singleline(&mut self.due_draft);
                if ui.small_button("Today").clicked() {
                    self.due_draft = Local::now().date_naive().to_string();
                }
                calendar_picker(ui, "due", &mut self.due_draft);
                if ui.button("Save due").clicked() {
                    self.save_date(true);
                }
            });
            ui.horizontal(|ui| {
                ui.label("Defer");
                ui.text_edit_singleline(&mut self.defer_draft);
                if ui.small_button("Today").clicked() {
                    self.defer_draft = Local::now().date_naive().to_string();
                }
                calendar_picker(ui, "defer", &mut self.defer_draft);
                if ui.button("Save defer").clicked() {
                    self.save_date(false);
                }
            });
            ui.horizontal(|ui| {
                ui.label("Scheduled");
                ui.text_edit_singleline(&mut self.scheduled_draft);
                if ui.small_button("Today").clicked() {
                    self.scheduled_draft = Local::now().date_naive().to_string();
                }
                calendar_picker(ui, "scheduled", &mut self.scheduled_draft);
                if ui.button("Save scheduled").clicked() {
                    self.save_scheduled();
                }
            });
            ui.horizontal(|ui| {
                ui.label("Difficulty 1-5");
                ui.text_edit_singleline(&mut self.difficulty_draft);
                if ui.button("Save difficulty").clicked() {
                    self.save_difficulty();
                }
            });
            ui.horizontal(|ui| {
                ui.label("Estimate minutes");
                ui.text_edit_singleline(&mut self.estimate_draft);
                if ui.button("Save estimate").clicked() {
                    self.save_estimate();
                }
            });
            // Weight is computed for display; nothing is stored for it.
            let children = child_index(&self.items);
            ui.weak(format!(
                "Weight {:.2} · size by weight is {}",
                item_weight(item, &children, &self.theme),
                if self.theme.weight.size_nodes_by_weight {
                    "on"
                } else {
                    "off"
                }
            ));
        }
        if self.inspector.show_sessions {
            ui.separator();
            ui.label("Work sessions");
            let active_session = self
                .sessions
                .iter()
                .any(|session| session.finished_at.is_none());
            if ui
                .button(if active_session {
                    "Stop session"
                } else {
                    "Start session"
                })
                .clicked()
            {
                self.toggle_session();
            }
            for session in &self.sessions {
                ui.weak(format!(
                    "{} → {}",
                    session.started_at,
                    session.finished_at.as_deref().unwrap_or("active")
                ));
            }
        }
    }

    /// The raw editing surface, kept apart from the everyday controls.
    fn section_advanced(&mut self, ui: &mut egui::Ui) {
        if !self.inspector.show_raw_json {
            ui.weak("Raw JSON is hidden in section display.");
            return;
        }
        ui.label("Raw JSON (prototype)");
        ui.add(egui::TextEdit::multiline(&mut self.raw_json).desired_rows(12));
        if ui.button("Parse JSON").clicked() {
            self.message = match serde_json::from_str::<serde_json::Value>(&self.raw_json) {
                Ok(_) => Some(
                    "JSON parses. Applying raw JSON is intentionally deferred until ItemPatch is atomic."
                        .to_owned(),
                ),
                Err(error) => Some(format!("JSON parse error: {error}")),
            };
        }
        if ui.button("Apply atomic patch").clicked() {
            self.apply_raw_patch();
        }
        if ui.button("Load title and priority").clicked() {
            self.load_raw_drafts();
        }
    }

    /// Per-subsection visibility, which changes nothing about the task itself.
    fn section_display_settings(&mut self, ui: &mut egui::Ui) {
        ui.label("Subsections shown. Hiding one changes nothing about the task.");
        ui.checkbox(&mut self.inspector.show_parent, "Parent task");
        ui.checkbox(&mut self.inspector.show_dependencies, "Depends on");
        ui.checkbox(&mut self.inspector.show_blockers, "Blocked by");
        ui.checkbox(&mut self.inspector.show_relations, "Typed relations");
        ui.checkbox(&mut self.inspector.show_focus_sequence, "Focus sequence");
        ui.checkbox(&mut self.inspector.show_dates, "Dates and effort");
        ui.checkbox(&mut self.inspector.show_sessions, "Work sessions");
        ui.checkbox(&mut self.inspector.show_notes, "Notes");
        ui.checkbox(&mut self.inspector.show_links, "Linked resources");
        ui.checkbox(&mut self.inspector.show_raw_json, "Raw JSON");
        if ui.button("Reset to theme defaults").clicked() {
            self.inspector = self.theme.inspector;
        }
    }

    /// Lays the graph out along a day axis driven by scheduled dates.
    ///
    /// Only `scheduled_on` places a task: due is a commitment and defer controls
    /// readiness, so neither is read here. Tasks with no scheduled day are parked
    /// to the left of the axis when configured, rather than piled on today.
    fn timeline_layout(&mut self, rect: Rect) {
        let today = Local::now().date_naive();
        let items = self.visible_items();
        let mut scheduled = Vec::new();
        for item in &items {
            let day = item.item.scheduled_on.map(|date| (date - today).num_days());
            if day.is_none() && !self.theme.timeline.park_unscheduled {
                continue;
            }
            scheduled.push((item.item.id.to_string(), day));
        }
        if scheduled.is_empty() {
            self.message = Some("No scheduled tasks to lay out.".to_owned());
            return;
        }
        let spacing = TimelineSpacing {
            day_width: self.theme.timeline.day_width,
            lane_height: self.theme.timeline.lane_height,
        };
        let arranged = geometry::timeline_positions(&scheduled, spacing);
        let origin = Pos2::new(rect.center().x, rect.top() + 140.0);
        self.layout_undo = Some(self.node_offsets.clone());
        for (index, item) in items.iter().enumerate() {
            let id = item.item.id.to_string();
            let Some(target) = arranged.get(&id) else {
                continue;
            };
            self.node_offsets
                .insert(id, (origin + *target) - grid_slot(index, rect));
        }
        self.relax_if_enabled(rect);
        self.focus_mode = false;
        if self.theme.timeline.reset_view {
            // Focus layout leaves the view framed on the active band. Keeping that
            // pan would place the timeline somewhere off screen.
            self.graph_pan = Vec2::ZERO;
        }
        self.timeline_origin = Some(origin);
        self.save_layout();
        self.message = Some("Timeline arranged by scheduled date.".to_owned());
    }

    /// Pushes overlapping nodes apart until each has clear space around it.
    ///
    /// Nodes that already have room stay exactly where they are, so relaxing a
    /// hand-made arrangement only unstacks what was hidden, rather than rearranging
    /// work that was placed deliberately.
    fn relax_overlaps(&mut self, rect: Rect, record_undo: bool) {
        let items = self.visible_items();
        if items.len() < 2 {
            return;
        }
        let mut placed = HashMap::new();
        for (index, item) in items.iter().enumerate() {
            let id = item.item.id.to_string();
            let offset = self.node_offsets.get(&id).copied().unwrap_or(Vec2::ZERO);
            placed.insert(id, grid_slot(index, rect).to_vec2() + offset);
        }
        let (width, height) = self.theme.node.size();
        let spacing = OverlapSpacing {
            iterations: self.theme.layout.overlap_iterations,
            gap: self.theme.layout.overlap_gap,
            strength: self.theme.layout.overlap_strength,
        };
        let relaxed = geometry::relax_overlaps(&placed, Vec2::new(width, height), spacing);
        if record_undo {
            self.layout_undo = Some(self.node_offsets.clone());
        }
        for (index, item) in items.iter().enumerate() {
            let id = item.item.id.to_string();
            let Some(target) = relaxed.get(&id) else {
                continue;
            };
            self.node_offsets
                .insert(id, *target - grid_slot(index, rect).to_vec2());
        }
        self.save_layout();
        if record_undo {
            self.message = Some("Overlapping tasks pushed apart.".to_owned());
        }
    }

    /// Pushes nodes out of tag regions they do not belong to.
    ///
    /// Each node is drawn toward the tasks sharing its deepest tag and pushed away
    /// from other tag groups nearby, so regions pull apart instead of overlapping
    /// and a node is no longer read as carrying a tag it does not have. Untagged
    /// tasks are left where they are.
    fn separate_tag_groups(&mut self, rect: Rect, record_undo: bool) {
        let items = self.visible_items();
        let mut groups = HashMap::new();
        for item in &items {
            let mut tags = item
                .namespaces
                .iter()
                .map(NamespacePath::as_str)
                .collect::<Vec<_>>();
            tags.sort_unstable();
            if let Some(deepest) = tags.into_iter().max_by_key(|tag| tag.matches('/').count()) {
                groups.insert(item.item.id.to_string(), deepest.to_owned());
            }
        }
        if groups.is_empty() {
            self.message = Some("No tagged tasks to separate.".to_owned());
            return;
        }
        let mut placed = HashMap::new();
        for (index, item) in items.iter().enumerate() {
            let id = item.item.id.to_string();
            let offset = self.node_offsets.get(&id).copied().unwrap_or(Vec2::ZERO);
            placed.insert(id, grid_slot(index, rect).to_vec2() + offset);
        }
        let spacing = SeparationSpacing {
            iterations: self.theme.layout.separation_iterations,
            attraction: self.theme.layout.separation_attraction,
            repulsion: self.theme.layout.separation_repulsion,
            radius: self.theme.layout.separation_radius,
        };
        let separated = geometry::separate_groups(&placed, &groups, spacing);
        if record_undo {
            self.layout_undo = Some(self.node_offsets.clone());
        }
        for (index, item) in items.iter().enumerate() {
            let id = item.item.id.to_string();
            let Some(target) = separated.get(&id) else {
                continue;
            };
            self.node_offsets
                .insert(id, *target - grid_slot(index, rect).to_vec2());
        }
        self.save_layout();
        if record_undo {
            // Separation invoked on its own moves nodes off the day axis, so the
            // ruler retires with it; running as a layout's follow-up leaves it be.
            self.timeline_origin = None;
            self.message =
                Some("Tag groups separated. Undo layout restores the previous one.".to_owned());
        }
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
            // Exiled work sits a multiple of the band gap away, which puts it
            // outside the frame the active band is zoomed to.
            band_gap: self.theme.focus.band_gap * self.theme.focus.exile_multiplier,
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
        if self.theme.layout.separate_after_layout {
            self.separate_tag_groups(rect, false);
        }
        self.relax_if_enabled(rect);
        self.timeline_origin = None;
        self.focus_mode = true;
        if self.theme.focus.zoom_to_active {
            self.frame_active(rect, &active);
        }
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
        if self.theme.layout.separate_after_layout {
            self.separate_tag_groups(rect, false);
        }
        // The ruler belongs to the timeline arrangement alone, so another layout
        // taking over retires it rather than leaving a scale nothing follows.
        self.relax_if_enabled(rect);
        self.timeline_origin = None;
        self.focus_mode = false;
        self.message = Some("Layout normalized. Undo layout restores the previous one.".to_owned());
    }

    /// Eases the drawn positions toward the saved ones by one frame.
    ///
    /// Layout commands write the saved offsets outright; drawing follows them a
    /// frame at a time, so a rearrangement reads as movement rather than as the
    /// graph replacing itself. Returns whether anything is still moving, so the
    /// caller can keep repainting only while it is.
    fn advance_motion(&mut self, elapsed: f32) -> bool {
        let duration = if self.theme.layout.animate {
            self.theme.layout.transition_ms / 1000.0
        } else {
            0.0
        };
        self.drawn_offsets
            .retain(|id, _| self.node_offsets.contains_key(id));
        let mut moving = false;
        for (id, target) in &self.node_offsets {
            let current = self.drawn_offsets.get(id).copied();
            let Some(current) = current else {
                // A node seen for the first time starts where it belongs rather
                // than flying in from the origin.
                self.drawn_offsets.insert(id.clone(), *target);
                continue;
            };
            let next = geometry::ease_toward(current, *target, elapsed, duration);
            if !geometry::is_settled(next, *target, 0.5) {
                moving = true;
                self.drawn_offsets.insert(id.clone(), next);
            } else {
                self.drawn_offsets.insert(id.clone(), *target);
            }
        }
        moving
    }

    /// Frames the active band in the viewport, so focus fills the canvas.
    ///
    /// Framing uses the same transform the graph draws with, so the band lands
    /// centered rather than approximately centered.
    fn frame_active(&mut self, rect: Rect, active: &[String]) {
        let items = self.visible_items();
        let mut points = Vec::new();
        for (index, item) in items.iter().enumerate() {
            let id = item.item.id.to_string();
            if !active.contains(&id) {
                continue;
            }
            let offset = self.node_offsets.get(&id).copied().unwrap_or(Vec2::ZERO);
            points.push(grid_slot(index, rect) + offset);
        }
        let (width, height) = self.theme.node.size();
        let Some(bounds) = bounds_for_points(&points, Vec2::new(width / 2.0, height / 2.0)) else {
            return;
        };
        let (zoom, pan) =
            geometry::fit_view(bounds, rect, (0.4, 2.5), self.theme.focus.zoom_margin);
        self.graph_zoom = zoom;
        self.graph_pan = pan;
    }

    /// Runs the overlap pass when it is switched on, as a layout's last step.
    fn relax_if_enabled(&mut self, rect: Rect) {
        if self.relax_overlaps {
            self.relax_overlaps(rect, false);
        }
    }

    /// Restores the arrangement captured before the last normalize.
    fn undo_layout(&mut self) {
        match self.layout_undo.take() {
            Some(offsets) => {
                self.node_offsets = offsets;
                self.timeline_origin = None;
                self.save_layout();
                self.message = Some("Previous layout restored.".to_owned());
            }
            None => self.message = Some("No layout change to undo.".to_owned()),
        }
    }

    /// Returns the items the graph is currently drawing, after state and tag filters.
    fn visible_items(&self) -> Vec<StoredItem> {
        self.items
            .iter()
            .filter(|item| {
                (self.show_completed
                    || (item.item.state != ItemState::Done
                        && item.item.state != ItemState::Canceled))
                    && (!self.focus_only || item.item.state == ItemState::Active)
                    && self
                        .tag_states
                        .accepts(self.effective_tags(item).iter().map(String::as_str))
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
                self.inspector = theme.inspector;
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
            self.difficulty_draft = item
                .item
                .difficulty
                .map(|value| value.to_string())
                .unwrap_or_default();
            self.estimate_draft = item
                .item
                .estimate_minutes
                .map(|value| value.to_string())
                .unwrap_or_default();
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
        let parent = self.new_task_parent.clone();
        match self.runtime.block_on(self.ledger.add_item(title, None)) {
            Ok(item) => {
                let id = item.item.id.to_string();
                self.new_task_title.clear();
                self.new_task_open = false;
                // The parent is set as a second step because creation is unscoped;
                // a failure to attach leaves the task itself intact and says so.
                let attached = match parent {
                    Some(parent) => {
                        match self.runtime.block_on(self.ledger.set_parent(&id, &parent)) {
                            Ok(()) => Some(Ok(())),
                            Err(error) => Some(Err(error)),
                        }
                    }
                    None => None,
                };
                self.new_task_parent = None;
                self.message = Some(match attached {
                    None => "New inbox task created.".to_owned(),
                    Some(Ok(())) => "New child task created.".to_owned(),
                    Some(Err(error)) => {
                        format!("Task created, but not attached to its parent: {error:#}")
                    }
                });
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

/// Returns the weeks of a month as rows of seven days, Monday first.
///
/// Days outside the month are `None`, so a caller can lay out a grid without
/// recomputing where the month starts and ends.
fn month_grid(year: i32, month: u32) -> Vec<[Option<NaiveDate>; 7]> {
    let Some(first) = NaiveDate::from_ymd_opt(year, month, 1) else {
        return Vec::new();
    };
    let lead = first.weekday().num_days_from_monday();
    let mut weeks = Vec::new();
    let mut week = [None; 7];
    let mut column = usize::try_from(lead).unwrap_or(0);
    let mut day = first;
    while day.month() == month {
        if let Some(slot) = week.get_mut(column) {
            *slot = Some(day);
        }
        column += 1;
        if column == 7 {
            weeks.push(week);
            week = [None; 7];
            column = 0;
        }
        let Some(next) = day.succ_opt() else {
            break;
        };
        day = next;
    }
    if week.iter().any(Option::is_some) {
        weeks.push(week);
    }
    weeks
}

/// Draws a calendar button that writes the picked day back into a text draft.
///
/// The text field stays the source of truth, so a date can still be typed or
/// cleared; the calendar only fills it in. An unreadable draft opens the calendar
/// on today rather than refusing to open.
fn calendar_picker(ui: &mut egui::Ui, id: &str, draft: &mut String) {
    let today = Local::now().date_naive();
    let selected = parse_date(draft).ok().flatten();
    let anchor = selected.unwrap_or(today);
    let popup_id = ui.make_persistent_id(("calendar", id));
    let open_id = ui.make_persistent_id(("calendar-open", id));
    let response = ui.small_button("Pick");
    // The open flag is kept in the ui's own data rather than egui's popup memory,
    // which is private, so the calendar owns its lifetime explicitly.
    let mut open = ui
        .data(|data| data.get_temp::<bool>(open_id))
        .unwrap_or(false);
    if response.clicked() {
        open = !open;
        ui.data_mut(|data| {
            data.insert_temp(open_id, open);
            data.insert_temp(popup_id, (anchor.year(), anchor.month()));
        });
    }
    if !open {
        return;
    }
    let (mut year, mut month) = ui
        .data(|data| data.get_temp::<(i32, u32)>(popup_id))
        .unwrap_or((anchor.year(), anchor.month()));
    let mut picked = None;
    let mut closed = false;
    egui::Area::new(popup_id)
        .order(egui::Order::Foreground)
        .fixed_pos(response.rect.left_bottom())
        .show(ui.ctx(), |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.horizontal(|ui| {
                    if ui.small_button("<").clicked() {
                        (year, month) = previous_month(year, month);
                    }
                    ui.label(format!("{year:04}-{month:02}"));
                    if ui.small_button(">").clicked() {
                        (year, month) = next_month(year, month);
                    }
                    if ui.small_button("Today").clicked() {
                        picked = Some(today);
                    }
                    if ui.small_button("Clear").clicked() {
                        picked = Some(today);
                        closed = true;
                        draft.clear();
                    }
                });
                for week in month_grid(year, month) {
                    ui.horizontal(|ui| {
                        for day in week {
                            match day {
                                Some(day) => {
                                    let label = format!("{:>2}", day.day());
                                    let button =
                                        egui::Button::new(label).min_size(Vec2::new(24.0, 18.0));
                                    let button = if Some(day) == selected {
                                        button.fill(Color32::from_rgb(82, 110, 156))
                                    } else if day == today {
                                        button.fill(Color32::from_gray(70))
                                    } else {
                                        button
                                    };
                                    if ui.add(button).clicked() {
                                        picked = Some(day);
                                    }
                                }
                                None => {
                                    ui.add_space(28.0);
                                }
                            }
                        }
                    });
                }
            });
        });
    ui.data_mut(|data| data.insert_temp(popup_id, (year, month)));
    if let Some(day) = picked {
        if !closed {
            *draft = day.to_string();
        }
        ui.data_mut(|data| data.insert_temp(open_id, false));
    }
}

/// Returns the month before the given one, rolling the year over.
fn previous_month(year: i32, month: u32) -> (i32, u32) {
    if month <= 1 {
        (year.saturating_sub(1), 12)
    } else {
        (year, month - 1)
    }
}

/// Returns the month after the given one, rolling the year over.
fn next_month(year: i32, month: u32) -> (i32, u32) {
    if month >= 12 {
        (year.saturating_add(1), 1)
    } else {
        (year, month + 1)
    }
}

/// Parses a whole-number draft, where an empty draft means "cleared".
fn parse_optional_number<T: std::str::FromStr>(
    value: &str,
) -> std::result::Result<Option<T>, String> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }
    value
        .parse::<T>()
        .map(Some)
        .map_err(|_| "Value must be a whole number, or empty to clear it.".to_owned())
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
        // The toolbar carries actions only. Every view toggle lives in the filter
        // sidebar, so there is one place to look for what is shown.
        egui::Panel::top("toolbar").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Todo Ledger");
                if ui.button("New task").clicked() {
                    self.new_task_parent = None;
                    self.new_task_title.clear();
                    self.new_task_open = true;
                }
                ui.separator();
                if ui.button("−").clicked() {
                    self.graph_zoom = (self.graph_zoom / 1.2).max(0.4);
                }
                if ui.button("+").clicked() {
                    self.graph_zoom = (self.graph_zoom * 1.2).min(2.5);
                }
                if ui.button("Reset view").clicked() {
                    self.graph_zoom = 1.0;
                    self.graph_pan = Vec2::ZERO;
                }
                ui.separator();
                if ui.button("Normalize").clicked() {
                    // The graph rect is only known while drawing, so the request is
                    // deferred one frame rather than guessed at here.
                    self.normalize_requested = true;
                }
                if ui.button("Focus layout").clicked() {
                    self.focus_layout_requested = true;
                }
                if ui.button("Timeline").clicked() {
                    self.timeline_requested = true;
                }
                if ui.button("Unstack").clicked() {
                    self.relax_requested = true;
                }
                if ui.button("Separate tags").clicked() {
                    self.separate_requested = true;
                }
                if ui.button("Undo layout").clicked() {
                    self.undo_layout();
                }
                ui.separator();
                if ui.button("Reload theme").clicked() {
                    self.reload_theme();
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
                    match self.new_task_parent.clone() {
                        Some(parent) => {
                            let described = self.describe(&parent);
                            ui.label(format!("Creates an inbox task under {described}."));
                        }
                        None => {
                            ui.label(
                                "Creates an unscoped inbox task. Assign a project or parent afterward.",
                            );
                        }
                    }
                    ui.text_edit_singleline(&mut self.new_task_title);
                    let valid = !self.new_task_title.trim().is_empty();
                    ui.horizontal(|ui| {
                        if ui.add_enabled(valid, egui::Button::new("Create")).clicked() {
                            self.add_task();
                        }
                        if ui.button("Cancel").clicked() {
                            self.new_task_open = false;
                            self.new_task_title.clear();
                            self.new_task_parent = None;
                        }
                    });
                });
        }
        egui::Panel::left("filters")
            .default_size(230.0)
            .resizable(true)
            .show(ui, |ui| self.draw_filters(ui));
        egui::Panel::right("inspector")
            .default_size(300.0)
            .resizable(true)
            .show(ui, |ui| self.draw_inspector(ui));
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
    if app.relax_requested {
        app.relax_requested = false;
        app.relax_overlaps(rect, true);
    }
    if app.separate_requested {
        app.separate_requested = false;
        app.separate_tag_groups(rect, true);
    }
    if app.timeline_requested {
        app.timeline_requested = false;
        app.timeline_layout(rect);
    }
    let items = app.visible_items();
    // The wheel divides hues between the tags actually on screen, so the palette is
    // rebuilt whenever the visible set changes.
    let effective = app.effective_tag_index(&items);
    let palette = match app.theme.tag.coloring {
        TagColoring::Wheel => TagPalette::build(
            effective.values().flatten().map(String::as_str),
            &app.theme.tag,
        ),
        TagColoring::GroupHash => TagPalette::default(),
    };
    let elapsed = ui.input(|input| input.stable_dt);
    if app.advance_motion(elapsed) {
        ui.ctx().request_repaint();
    }
    let positions = graph_positions(
        &items,
        rect,
        &app.drawn_offsets,
        app.graph_pan,
        app.graph_zoom,
    );
    let show_hierarchy = app.show_parent_graph;
    let show_dependencies = app.show_dependency_graph;
    let show_blockers = app.show_blocker_graph;
    let show_relations = app.show_relations;
    if let Some(origin) = app.timeline_origin
        && app.show_timeline_ruler
        && app.theme.timeline.show_ruler
    {
        draw_timeline_ruler(&painter, rect, origin, app);
    }
    if app.show_parent_regions && app.theme.parent.show_regions {
        draw_parent_regions(
            &painter,
            &items,
            &positions,
            app.graph_zoom,
            &app.theme,
            &palette,
            &effective,
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
            &effective,
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
        if show_relations {
            for relation in &item.relations {
                if let Some(to_target) = positions.get(&relation.target.to_string()) {
                    // A relation points from the item to what it contributes to or
                    // implements, and is drawn apart from structure and dependencies.
                    draw_relation_edge(
                        &painter,
                        *to,
                        *to_target,
                        relation_color(relation.relation),
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
    let child_index = child_index(&items);
    for item in &items {
        let id = item.item.id.to_string();
        let Some(center) = positions.get(&id) else {
            continue;
        };
        let (node_width, node_height) = app.theme.node.size();
        let scale = app
            .theme
            .weight
            .scale(item_weight(item, &child_index, &app.theme));
        let zoom = app.graph_zoom * scale;
        let title_font = FontId::proportional(app.theme.node.title_font_size * zoom);
        let meta_font = FontId::proportional(app.theme.node.meta_font_size * zoom);
        // The title is laid out before the node is sized, so the card can grow to
        // hold what it says instead of cutting the title off mid-word.
        let padding = app.theme.node.text_padding * zoom;
        let wrap_width = (node_width * zoom - padding * 2.0).max(1.0);
        let title_galley = painter.layout(
            node_title(&item.item.title, &app.theme),
            title_font,
            Color32::WHITE,
            wrap_width,
        );
        let meta_height = if app.theme.node.show_meta_line {
            app.theme.node.meta_font_size * zoom + padding / 2.0
        } else {
            0.0
        };
        let needed = title_galley.size().y + meta_height + padding * 2.0;
        let height = needed
            .max(node_height * zoom)
            .min(app.theme.node.max_height * zoom);
        let node = Rect::from_center_size(*center, Vec2::new(node_width * zoom, height));
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
        // While focus mode is on, work that is not active stays drawn but quiet, so
        // it reads as context rather than as part of what is being worked on.
        let exiled = app.focus_mode && item.item.state != ItemState::Active;
        let dim = |color: Color32| {
            if exiled {
                Color32::from_rgba_unmultiplied(
                    color.r(),
                    color.g(),
                    color.b(),
                    app.theme.focus.exiled_alpha,
                )
            } else {
                color
            }
        };
        let fill = dim(fill);
        let accent = node_tag_color(item, &app.theme, &palette, &effective).map(dim);
        let outline = Stroke::new(
            if selected { 3.0 } else { 1.0 },
            if selected {
                Color32::GOLD
            } else {
                dim(Color32::LIGHT_GRAY)
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
        if app.theme.node.show_meta_line {
            let title_top = node.top() + padding;
            painter.galley(
                egui::pos2(node.center().x - title_galley.size().x / 2.0, title_top),
                title_galley,
                dim(Color32::WHITE),
            );
            painter.text(
                node.center_bottom() - Vec2::new(0.0, padding / 2.0),
                Align2::CENTER_BOTTOM,
                format!("{} · {}", item.item.state, item.item.priority),
                meta_font,
                dim(Color32::LIGHT_GRAY),
            );
        } else {
            let anchor = node.center() - title_galley.size() / 2.0;
            painter.galley(anchor, title_galley, dim(Color32::WHITE));
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
            let dragged_to = offset_start + dragged_distance;
            app.node_offsets.insert(id.clone(), dragged_to);
            // A dragged node follows the pointer exactly; easing it would feel like
            // lag rather than like motion.
            app.drawn_offsets.insert(id.clone(), dragged_to);
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

/// Returns the color a typed relation is drawn in.
fn relation_color(relation: RelationKind) -> Color32 {
    match relation {
        RelationKind::ContributesTo => Color32::from_rgb(171, 130, 205),
        RelationKind::Implements => Color32::from_rgb(198, 164, 92),
    }
}

/// Draws a typed relation edge, dashed by default so it reads as a reference.
fn draw_relation_edge(
    painter: &egui::Painter,
    start: Pos2,
    end: Pos2,
    color: Color32,
    theme: &Theme,
    zoom: f32,
) {
    let (head_length, head_width) = theme.edge.arrowhead_size(zoom);
    let stroke = Stroke::new(theme.edge.width, color);
    let shaft_end = geometry::shaft_end(start, end, head_length);
    if theme.edge.dash_relations {
        let dash = (theme.edge.relation_dash_length * zoom).max(1.0);
        painter.add(egui::Shape::dashed_line(
            &[start, shaft_end],
            stroke,
            dash,
            dash,
        ));
    } else {
        painter.line_segment([start, shaft_end], stroke);
    }
    if let Some(head) = geometry::arrowhead_points(start, end, head_length, head_width) {
        painter.add(egui::Shape::convex_polygon(
            head.to_vec(),
            color,
            Stroke::NONE,
        ));
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

/// Draws the day ruler a timeline layout was built against.
///
/// The ruler is drawn from the last timeline arrangement, so it lines up with the
/// nodes rather than claiming a scale the graph does not use. Today is marked, so
/// past and upcoming scheduled work read apart at a glance.
fn draw_timeline_ruler(painter: &egui::Painter, rect: Rect, origin: Pos2, app: &TodoGui) {
    let today = Local::now().date_naive();
    let day_width = app.theme.timeline.day_width * app.graph_zoom;
    let center = rect.center();
    let axis_origin = center + (origin - center) * app.graph_zoom + app.graph_pan;
    // The axis is pinned to the top of the canvas and only follows the graph
    // horizontally. Carrying its height through the transform let a pan left over
    // from another layout push the whole ruler off screen.
    let axis_y = rect.top() + app.theme.timeline.ruler_top_offset;
    let margin = f32::from(app.theme.timeline.ruler_margin_days) * day_width;
    let ticks = geometry::ruler_ticks(
        axis_origin.x,
        day_width,
        (rect.left(), rect.right()),
        margin,
    );
    if ticks.is_empty() {
        return;
    }
    painter.line_segment(
        [
            Pos2::new(rect.left(), axis_y),
            Pos2::new(rect.right(), axis_y),
        ],
        Stroke::new(1.0, Color32::from_gray(90)),
    );
    for (offset, x) in ticks {
        let Some(day) = today.checked_add_signed(chrono::Duration::days(i64::from(offset))) else {
            continue;
        };
        let is_today = offset == 0;
        let tick_color = if is_today {
            Color32::from_rgb(224, 188, 73)
        } else {
            Color32::from_gray(90)
        };
        painter.line_segment(
            [Pos2::new(x, axis_y - 6.0), Pos2::new(x, axis_y + 6.0)],
            Stroke::new(if is_today { 2.0 } else { 1.0 }, tick_color),
        );
        painter.text(
            Pos2::new(x, axis_y - 10.0),
            Align2::CENTER_BOTTOM,
            day.format("%m-%d").to_string(),
            FontId::proportional(10.0),
            if is_today {
                tick_color
            } else {
                Color32::from_gray(140)
            },
        );
    }
}

/// Draws a containment region around each parent task and its descendants.
///
/// A parent's region wraps the whole subtree, not just its direct children, so a
/// deep branch reads as one body of work. Regions are drawn largest first so a
/// nested subtree stays visible inside its ancestor. The color comes from the
/// parent's own tag, falling back to a neutral outline when it has no tag, so the
/// region never implies a tag the parent does not carry.
/// Returns each task's visible children, keyed by parent id.
fn child_index(items: &[StoredItem]) -> HashMap<String, Vec<String>> {
    let mut children: HashMap<String, Vec<String>> = HashMap::new();
    for item in items {
        if let Some(parent) = item.item.parent_id {
            children
                .entry(parent.to_string())
                .or_default()
                .push(item.item.id.to_string());
        }
    }
    children
}

fn draw_parent_regions(
    painter: &egui::Painter,
    items: &[StoredItem],
    positions: &HashMap<String, Pos2>,
    zoom: f32,
    theme: &Theme,
    palette: &TagPalette,
    effective: &HashMap<String, Vec<String>>,
) {
    let children = child_index(items);
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
        let color =
            node_tag_color(item, theme, palette, effective).unwrap_or(Color32::from_gray(150));
        let depth = ancestor_depth(item, items);
        regions.push((subtree.len(), depth, points, color, item.item.title.clone()));
    }
    // Largest first, so a nested subtree stays visible inside its ancestor.
    regions.sort_by_key(|(size, ..)| std::cmp::Reverse(*size));
    for (_, depth, points, color, title) in regions {
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
        // Depth widens the outer boxes, so an ancestor never hugs the box inside it.
        let nesting = f32::from(u8::try_from(depth).unwrap_or(u8::MAX)) * theme.parent.nest_padding;
        let padding = (theme.parent.padding + nesting) * zoom;
        match theme.parent.region_style {
            ParentRegionStyle::Box => {
                let Some(bounds) = bounds_for_points(&points, half_size) else {
                    continue;
                };
                let header = theme.parent.header_height * zoom;
                let box_rect = Rect::from_min_max(
                    bounds.left_top() - Vec2::new(padding, padding + header),
                    bounds.right_bottom() + Vec2::new(padding, padding),
                );
                let rounding = theme.parent.corner_radius * zoom;
                painter.rect_filled(box_rect, rounding, fill);
                painter.rect_stroke(box_rect, rounding, stroke, egui::StrokeKind::Outside);
                // The title strip makes the box read as the parent task itself
                // holding a subgraph, rather than as an unlabeled enclosure.
                let header_rect = Rect::from_min_max(
                    box_rect.left_top(),
                    egui::pos2(box_rect.right(), box_rect.top() + header),
                );
                painter.rect_filled(
                    header_rect,
                    rounding,
                    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 46),
                );
                painter.text(
                    header_rect.left_center() + Vec2::new(8.0 * zoom, 0.0),
                    Align2::LEFT_CENTER,
                    short_title(&title),
                    FontId::proportional(11.0 * zoom),
                    color,
                );
            }
            ParentRegionStyle::Hull => {
                let corners = node_corners(&points, half_size);
                let hull = geometry::convex_hull(&corners);
                let padded = geometry::expand_path(&hull, padding);
                let outline = geometry::smooth_closed_path(&padded, theme.parent.smoothing);
                if outline.len() < 3 {
                    continue;
                }
                painter.add(egui::Shape::convex_polygon(outline.clone(), fill, stroke));
                if let Some(label_anchor) = outline.iter().min_by(|left, right| {
                    left.y
                        .total_cmp(&right.y)
                        .then_with(|| left.x.total_cmp(&right.x))
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
    }
}

/// Returns how many visible ancestors a task has above it.
///
/// Nested boxes use this to grow outward, so an outer box always has room for the
/// one inside it.
fn ancestor_depth(item: &StoredItem, items: &[StoredItem]) -> usize {
    let mut depth = 0;
    let mut current = item.item.parent_id;
    while let Some(parent) = current {
        let Some(found) = items.iter().find(|candidate| candidate.item.id == parent) else {
            break;
        };
        depth += 1;
        if depth > items.len() {
            // A parent cycle cannot deepen forever; the ledger rejects one, but a
            // stale view should still not spin here.
            break;
        }
        current = found.item.parent_id;
    }
    depth
}

/// Returns the presentation weight of one task, from 0.0 to 1.0.
///
/// The weight is computed here rather than stored, so changing how work is weighed
/// never touches the ledger. Descendants are counted among the visible items only,
/// so a filtered view weighs what it actually shows.
fn item_weight(item: &StoredItem, children: &HashMap<String, Vec<String>>, theme: &Theme) -> f32 {
    let today = Local::now().date_naive();
    let descendants = collect_subtree(&item.item.id.to_string(), children)
        .len()
        .saturating_sub(1);
    let inputs = WeightInputs {
        priority_rank: item.item.priority.rank(),
        days_until_due: item.item.due_on.map(|due| (due - today).num_days()),
        difficulty: item.item.difficulty,
        estimate_minutes: item.item.estimate_minutes,
        descendants,
    };
    todo_core::weight(inputs, theme.weight.factors)
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
    effective: &HashMap<String, Vec<String>>,
) {
    let mut groups: HashMap<String, Vec<Pos2>> = HashMap::new();
    for item in items {
        let id = item.item.id.to_string();
        let Some(position) = positions.get(&id).copied() else {
            continue;
        };
        let empty = Vec::new();
        let carried = effective.get(&id).unwrap_or(&empty);
        for tag in carried {
            let segments = tag.split('/').collect::<Vec<_>>();
            for depth in 2..=segments.len() {
                if let Some(prefix) = segments.get(..depth) {
                    groups.entry(prefix.join("/")).or_default().push(position);
                }
            }
        }
    }
    let mut paths = groups.keys().cloned().collect::<Vec<_>>();
    paths.sort_by_key(|path| path.matches('/').count());
    let half_size = {
        let (width, height) = theme.node.size();
        Vec2::new(width / 2.0 + 14.0, height / 2.0 + 10.0) * zoom
    };
    for path in paths {
        let Some(points) = groups.get(&path) else {
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
        // One region per cluster of nearby members, so a tag spread over two
        // distant bodies of work draws two regions instead of one that spans the
        // canvas and appears to contain everything between them.
        for cluster in geometry::cluster_points(points, theme.tag.cluster_distance * zoom) {
            if cluster.len() < theme.tag.min_region_members {
                continue;
            }
            let Some(bounds) = bounds_for_points(&cluster, half_size) else {
                continue;
            };
            let drawn = if theme.tag.region_shape == RegionShape::Hull {
                let corners = node_corners(&cluster, half_size);
                let hull = geometry::convex_hull(&corners);
                let padded = geometry::expand_path(&hull, theme.tag.region_padding);
                let outline = geometry::smooth_closed_path(&padded, theme.tag.region_smoothing);
                if outline.len() >= 3 {
                    painter.add(egui::Shape::convex_polygon(outline, fill, stroke));
                    true
                } else {
                    false
                }
            } else {
                false
            };
            if !drawn {
                painter.rect_filled(bounds.expand(16.0), 18.0, fill);
                painter.rect_stroke(bounds.expand(16.0), 18.0, stroke, egui::StrokeKind::Outside);
            }
            draw_region_label(painter, bounds, &path, color, theme, zoom);
        }
    }
}

/// Draws a region's label above it, clipped to the width the region can hold.
///
/// A label sits outside the region rather than over its members, so it never
/// overlaps a task card, and a region too narrow to read a label goes unlabeled
/// rather than showing a few clipped characters.
fn draw_region_label(
    painter: &egui::Painter,
    bounds: Rect,
    path: &str,
    color: Color32,
    theme: &Theme,
    zoom: f32,
) {
    let width = bounds.width();
    if width < theme.tag.label_min_width * zoom {
        return;
    }
    let text = if theme.tag.label_full_path {
        path.to_owned()
    } else {
        path.rsplit('/').next().unwrap_or(path).to_owned()
    };
    let font_size = theme.tag.label_font_size * zoom;
    // Roughly half the font size per character is close enough to keep a label
    // inside its region without measuring the galley first.
    let per_character = (font_size * 0.55).max(1.0);
    let mut shown = String::new();
    let mut used = 0.0_f32;
    let mut clipped = false;
    for character in text.chars() {
        if used + per_character > width {
            clipped = true;
            break;
        }
        shown.push(character);
        used += per_character;
    }
    if clipped {
        shown.pop();
        shown.push('…');
    }
    if shown.is_empty() {
        return;
    }
    painter.text(
        bounds.center_top() - Vec2::new(0.0, 6.0 * zoom),
        Align2::CENTER_BOTTOM,
        shown,
        FontId::proportional(font_size),
        color,
    );
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
fn node_tag_color(
    item: &StoredItem,
    theme: &Theme,
    palette: &TagPalette,
    effective: &HashMap<String, Vec<String>>,
) -> Option<Color32> {
    let carried = effective.get(&item.item.id.to_string());
    let mut tags = match carried {
        Some(tags) => tags.iter().map(String::as_str).collect::<Vec<_>>(),
        None => item
            .namespaces
            .iter()
            .map(NamespacePath::as_str)
            .collect::<Vec<_>>(),
    };
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

/// Returns the title to draw on a node, wrapped in full or cut short.
///
/// Wrapping is the default because a task named only by its first twenty
/// characters is a task you have to click to identify.
fn node_title(title: &str, theme: &Theme) -> String {
    if theme.node.wrap_titles {
        title.to_owned()
    } else {
        short_title(title)
    }
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
