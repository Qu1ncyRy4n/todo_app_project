//! Native graph editor for the local Todo Ledger.

mod geometry;
mod palette;
mod tags;
mod theme;

use anyhow::{Context, Result, bail};
use chrono::{Datelike, Local, NaiveDate};
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, Vec2};
use geometry::{
    BandSpacing, FlowSpacing, GroupSpacing, OverlapSpacing, SeparationSpacing, TimelineSpacing,
    TreeSpacing,
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, path::PathBuf};
use tags::{TagFilter, TagFilterState};
use theme::{
    Channel, CloudMemberShape, ColorMode, ConnectorStyle, FamilyChildStyle, FamilyHeaderStyle,
    FamilyView, FamilyWellFill, InspectorTheme, NodeShape, NormalizeStyle, ParentRegionStyle,
    RegionShape, TagCloudLinkMode, TagColoring, TagInheritance, TagPalette, TagTheme,
    TerminalPalette, Theme, band_color,
};
use todo_core::entry::{ASSIGNEE, AUTHOR, CONTRIBUTOR, REQUESTER};
use todo_core::{
    ItemId, ItemState, NamespaceKind, NamespacePath, Priority, RelationKind, WeightInputs,
};
use todo_store::{
    AppConfig, AppPaths, Citation, FocusSequence, HistoryEntry, ItemPatch, ItemScope, Ledger,
    NamespaceEntry, Note, StoredItem, WorkSession,
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
    // Opened once up front so a broken ledger fails at launch, then released.
    drop(runtime.block_on(Ledger::open_with_actor(&paths.database(), actor_id.clone()))?);
    let access = LedgerAccess {
        path: paths.database(),
        actor_id,
    };
    let agent_filter = AgentFilter::from_config(config.agent_items.as_deref());
    let layout_path = paths.config_dir.join("gui-layout.json");
    let theme_path = paths.config_dir.join("theme.toml");
    eframe::run_native(
        "Todo Ledger",
        eframe::NativeOptions::default(),
        Box::new(move |_| {
            Ok(Box::new(TodoGui::new(
                runtime,
                access,
                layout_path,
                theme_path,
                agent_filter,
            )))
        }),
    )
    .context("run native Todo Ledger window")
}

/// Where the ledger lives and who the editor writes as.
///
/// Turso locks the database file for as long as it is open, so the editor holds
/// this instead of an open `Ledger` and opens one per operation. The CLI is then
/// locked out for milliseconds rather than for the whole session, and the
/// store's open retry absorbs any overlap.
struct LedgerAccess {
    path: PathBuf,
    actor_id: String,
}

impl LedgerAccess {
    /// The latest modification time of the database and its write-ahead log.
    fn modified(&self) -> Option<std::time::SystemTime> {
        let mut wal = self.path.clone().into_os_string();
        wal.push("-wal");
        [self.path.as_os_str(), wal.as_os_str()]
            .into_iter()
            .filter_map(|path| fs::metadata(path).and_then(|meta| meta.modified()).ok())
            .max()
    }
}

/// Where a file's symlinks lead, when it last changed, and its size.
type FileStamp = (PathBuf, std::time::SystemTime, u64);

/// Stamps a file so a change can be noticed without reading it, or `None` when
/// it does not exist.
///
/// The resolved path is part of the stamp because home-manager files are
/// symlinks into the Nix store, where every modification time is 1970: a
/// generation switch moves only the link.
fn file_stamp(path: &std::path::Path) -> Option<FileStamp> {
    let resolved = fs::canonicalize(path).ok()?;
    let meta = fs::metadata(&resolved).ok()?;
    Some((resolved, meta.modified().ok()?, meta.len()))
}

/// What the graph is grouped by at one level.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum GroupBy {
    /// This level is unused, which is how grouping is narrowed to fewer levels.
    #[default]
    None,
    /// Keep each structural family together under its top-level task.
    Hierarchy,
    Tag,
    Sequence,
    Time,
    Priority,
    State,
}

impl GroupBy {
    /// Returns every grouping, for the sidebar control.
    fn all() -> [Self; 7] {
        [
            Self::None,
            Self::Hierarchy,
            Self::Sequence,
            Self::Priority,
            Self::Tag,
            Self::Time,
            Self::State,
        ]
    }

    /// Returns the label shown for this grouping.
    fn label(self) -> &'static str {
        match self {
            Self::None => "—",
            Self::Hierarchy => "Hierarchy",
            Self::Tag => "Tag",
            Self::Sequence => "Sequence",
            Self::Time => "Time",
            Self::Priority => "Priority",
            Self::State => "State",
        }
    }
}

/// What members of a group are ordered by.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum SortBy {
    /// Most urgent first, which is the order work is usually scanned in.
    #[default]
    Priority,
    Title,
    Due,
    Weight,
    State,
    /// Position in the selected focus sequence, then everything else.
    Sequence,
    /// Oldest first; identifiers are time-ordered, so this is capture order.
    Created,
}

impl SortBy {
    fn all() -> [Self; 7] {
        [
            Self::Priority,
            Self::Sequence,
            Self::Due,
            Self::Weight,
            Self::Title,
            Self::State,
            Self::Created,
        ]
    }

    fn label(self) -> &'static str {
        match self {
            Self::Title => "Title",
            Self::Priority => "Priority",
            Self::Due => "Due date",
            Self::Weight => "Weight",
            Self::State => "State",
            Self::Sequence => "Sequence position",
            Self::Created => "Capture order",
        }
    }
}

/// What a drag started from.
///
/// The starting offsets of everything the drag moves are captured once, so each
/// frame places them at their start plus the total pointer travel. Applying the
/// travel to wherever a node currently sits would add it again every frame, which
/// accelerates the node away from the pointer instead of following it.
struct DragAnchor {
    id: String,
    pointer_start: Pos2,
    offsets_start: HashMap<String, Vec2>,
}

/// Which agent-created tasks the graph shows; `agent_items` sets the default.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AgentFilter {
    Show,
    Hide,
    Only,
}

impl AgentFilter {
    const ALL: [Self; 3] = [Self::Show, Self::Hide, Self::Only];

    fn from_config(value: Option<&str>) -> Self {
        match value {
            Some("hide") => Self::Hide,
            Some("only") => Self::Only,
            _ => Self::Show,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Show => "Show",
            Self::Hide => "Hide",
            Self::Only => "Only",
        }
    }

    fn keeps(self, item: &StoredItem) -> bool {
        match self {
            Self::Show => true,
            Self::Hide => item.agent.is_none(),
            Self::Only => item.agent.is_some(),
        }
    }
}

/// The tabs of the Settings window, which holds how the graph looks and moves.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum SettingsTab {
    #[default]
    Layout,
    Cards,
    Families,
    Tags,
    Motion,
    Colors,
    Inspector,
}

impl SettingsTab {
    const ALL: [Self; 7] = [
        Self::Layout,
        Self::Cards,
        Self::Families,
        Self::Tags,
        Self::Motion,
        Self::Colors,
        Self::Inspector,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::Layout => "Layout",
            Self::Cards => "Cards",
            Self::Families => "Families",
            Self::Tags => "Tags",
            Self::Motion => "Motion",
            Self::Colors => "Colors",
            Self::Inspector => "Inspector",
        }
    }
}

/// One filter that differs from its default, shown as a chip above the canvas so
/// what is hidden stays visible while the filter drawer is closed.
#[derive(Clone, Debug, Eq, PartialEq)]
enum FilterChip {
    Completed,
    ActiveOnly,
    Agent,
    Tag(String, TagFilterState),
    Search,
    Lines,
    Overlays,
}

/// The inspector value being edited in place, if any. One at a time: Enter
/// commits it, Escape or selecting another task drops it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Edit {
    Title,
    Person,
    Due,
    Defer,
    Scheduled,
    Estimate,
    Difficulty,
}

impl Edit {
    /// The time fields, in the order the Time band lists them.
    const TIME: [Self; 5] = [
        Self::Due,
        Self::Defer,
        Self::Scheduled,
        Self::Estimate,
        Self::Difficulty,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::Title => "title",
            Self::Person => "person",
            Self::Due => "due",
            Self::Defer => "defer",
            Self::Scheduled => "scheduled",
            Self::Estimate => "estimate",
            Self::Difficulty => "difficulty",
        }
    }

    fn is_date(self) -> bool {
        matches!(self, Self::Due | Self::Defer | Self::Scheduled)
    }
}

/// Roles offered in the inspector; any other role name can still be typed.
const ROLES: [&str; 4] = [AUTHOR, CONTRIBUTOR, ASSIGNEE, REQUESTER];

struct TodoGui {
    runtime: Runtime,
    ledger: LedgerAccess,
    /// Ledger file time at the last refresh; see `refresh_if_ledger_changed`.
    ledger_seen: Option<std::time::SystemTime>,
    items: Vec<StoredItem>,
    namespaces: Vec<NamespaceEntry>,
    notes: Vec<Note>,
    /// The selected task's people, as (role, actor path).
    roles: Vec<(String, NamespacePath)>,
    role_draft: String,
    actor_draft: String,
    agent_filter: AgentFilter,
    /// The configured `agent_items`, which Reset filters returns to.
    agent_default: AgentFilter,
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
    expanded_families: std::collections::HashSet<String>,
    family_presentations: HashMap<String, FamilyPresentation>,
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
    drag_anchor: Option<DragAnchor>,
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
    /// The theme as last read from or written to disk, so unsaved panel edits
    /// can be told apart from it.
    theme_loaded: Theme,
    /// Stamps of `theme.toml` and the palette file at the last load or save; see
    /// `reload_theme_if_changed`.
    theme_seen: Vec<Option<FileStamp>>,
    /// Cleared on load and on Reload theme, so the next frame applies the color
    /// mode and palette; applying needs the egui context, which only a frame has.
    colors_applied: bool,
    inspector: InspectorTheme,
    layout_undo: Option<LayoutUndo>,
    normalize_requested: bool,
    reset_view_requested: bool,
    focus_layout_requested: bool,
    separate_requested: bool,
    relax_requested: bool,
    relax_overlaps: bool,
    focus_mode: bool,
    focus_only: bool,
    group_by: [GroupBy; 3],
    sort_by: SortBy,
    group_requested: bool,
    group_labels: Vec<(String, Pos2)>,
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
    search: String,
    search_focus: bool,
    show_filters: bool,
    show_inspector: bool,
    show_settings: bool,
    settings_tab: SettingsTab,
    editing: Option<Edit>,
    /// Set when an edit begins, so its field takes focus once and only once.
    edit_focus: bool,
    /// The selected task's events, loaded when the footer opens and dropped on
    /// refresh so it never shows a stale list.
    history: Option<(String, Vec<HistoryEntry>)>,
}

/// A local presentation override for one parent family. It belongs to the GUI
/// layout file, never to the event ledger or task record.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum FamilyPresentation {
    #[default]
    Dynamic,
    Box,
    Tree,
}

impl FamilyPresentation {
    fn label(self) -> &'static str {
        match self {
            Self::Dynamic => "Dynamic",
            Self::Box => "Box",
            Self::Tree => "Tree",
        }
    }
}

#[derive(Default, Deserialize, Serialize)]
struct LocalGraphLayout {
    node_offsets: HashMap<String, [f32; 2]>,
    #[serde(default)]
    family_presentations: HashMap<String, FamilyPresentation>,
}

struct LayoutUndo {
    offsets: HashMap<String, Vec2>,
    expanded_families: std::collections::HashSet<String>,
}

impl TodoGui {
    fn new(
        runtime: Runtime,
        ledger: LedgerAccess,
        layout_path: PathBuf,
        theme_path: PathBuf,
        agent_filter: AgentFilter,
    ) -> Self {
        let saved_layout = load_layout(&layout_path);
        let node_offsets = saved_layout
            .node_offsets
            .iter()
            .map(|(id, [x, y])| (id.clone(), Vec2::new(*x, *y)))
            .collect();
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
            ledger_seen: None,
            items: Vec::new(),
            namespaces: Vec::new(),
            notes: Vec::new(),
            roles: Vec::new(),
            role_draft: AUTHOR.to_owned(),
            actor_draft: String::new(),
            agent_filter,
            agent_default: agent_filter,
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
            expanded_families: std::collections::HashSet::new(),
            family_presentations: saved_layout.family_presentations,
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
            theme_loaded: theme.clone(),
            theme,
            theme_path,
            theme_seen: Vec::new(),
            colors_applied: false,
            layout_undo: None,
            normalize_requested: false,
            reset_view_requested: false,
            focus_layout_requested: false,
            separate_requested: false,
            relax_requested: false,
            relax_overlaps: relax_after_layout,
            focus_mode: false,
            focus_only: false,
            // Structural families make the starting graph legible; other board
            // groupings remain available when the operator asks for them.
            group_by: [GroupBy::Hierarchy, GroupBy::None, GroupBy::None],
            sort_by: SortBy::default(),
            group_requested: false,
            group_labels: Vec::new(),
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
            search: String::new(),
            search_focus: false,
            show_filters: true,
            show_inspector: true,
            show_settings: false,
            settings_tab: SettingsTab::default(),
            editing: None,
            edit_focus: false,
            history: None,
        };
        app.refresh_if_ledger_changed();
        app.theme_seen = app.theme_stamps();
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
        match self.with_ledger(async |ledger| ledger.add_relation(&id, relation, &target).await) {
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
        match self.with_ledger(async |ledger| ledger.remove_relation(&id, relation, &target).await)
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
        match self.with_ledger(async |ledger| ledger.set_difficulty(&id, parsed).await) {
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
        match self.with_ledger(async |ledger| ledger.set_estimate(&id, parsed).await) {
            Ok(_) => {
                self.message = Some("Estimate saved.".to_owned());
                self.refresh();
            }
            Err(error) => self.message = Some(format!("Estimate not saved: {error:#}")),
        }
    }

    /// Draws the filter drawer: what is shown, and nothing about how it looks.
    ///
    /// How the graph is arranged, drawn, and moved lives in the Settings window,
    /// so this panel answers one question.
    fn draw_filters(&mut self, ui: &mut egui::Ui) {
        // Controls such as long tag paths and relationship labels should remain
        // reachable in a narrow sidebar. Vertical scrolling handles its length;
        // horizontal scrolling preserves a control rather than clipping its label.
        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.heading("Filter");
                    if ui.small_button("reset").clicked() {
                        self.reset_filters();
                    }
                });
                egui::CollapsingHeader::new("Tasks shown")
                    .default_open(true)
                    .show(ui, |ui| {
                        ui.checkbox(&mut self.show_completed, "Completed and canceled");
                        ui.checkbox(&mut self.focus_only, "Active work only");
                        ui.weak("Active work only hides everything that is not active.");
                        ui.horizontal(|ui| {
                            ui.label("Agent-created");
                            egui::ComboBox::from_id_salt("agent-filter")
                                .selected_text(self.agent_filter.label())
                                .show_ui(ui, |ui| {
                                    for filter in AgentFilter::ALL {
                                        ui.selectable_value(
                                            &mut self.agent_filter,
                                            filter,
                                            filter.label(),
                                        );
                                    }
                                });
                        });
                    });
                egui::CollapsingHeader::new("Tags")
                    .default_open(true)
                    .show(ui, |ui| self.draw_tag_filters(ui));
                egui::CollapsingHeader::new("Lines drawn")
                    .default_open(true)
                    .show(ui, |ui| {
                        egui::Grid::new("lines-drawn").show(ui, |ui| {
                            ui.checkbox(&mut self.show_parent_graph, "Parents");
                            ui.checkbox(&mut self.show_dependency_graph, "Depends on");
                            ui.end_row();
                            ui.checkbox(&mut self.show_blocker_graph, "Blocked by");
                            ui.checkbox(&mut self.show_relations, "Relations");
                            ui.end_row();
                        });
                    });
                egui::CollapsingHeader::new("Overlays")
                    .default_open(true)
                    .show(ui, |ui| {
                        egui::Grid::new("overlays").show(ui, |ui| {
                            ui.checkbox(&mut self.show_tag_constellation, "Tag regions");
                            ui.checkbox(&mut self.show_parent_regions, "Parent regions");
                            ui.end_row();
                            ui.checkbox(&mut self.show_focus_window, "Focus window");
                            ui.checkbox(&mut self.show_timeline_ruler, "Timeline ruler");
                            ui.end_row();
                        });
                    });
            });
    }

    /// Restores the transient visibility controls without touching an arrangement
    /// or the theme. A person who has narrowed the graph needs one known way back
    /// to the initial view, not a tour through every individual control.
    fn reset_filters(&mut self) {
        self.show_completed = false;
        self.focus_only = false;
        self.show_parent_graph = true;
        self.show_dependency_graph = true;
        self.show_blocker_graph = true;
        self.show_relations = true;
        self.show_tag_constellation = true;
        self.show_parent_regions = true;
        self.show_focus_window = true;
        self.show_timeline_ruler = true;
        self.agent_filter = self.agent_default;
        self.tag_filter.clear();
        self.tag_states.clear();
        self.search.clear();
        self.message = Some("Filters restored to their defaults.".to_owned());
    }

    /// The filters that differ from a fresh window, one chip each.
    ///
    /// Agent filtering counts whenever it hides anything, even when that is the
    /// configured default, because it still answers "where is my task?".
    fn active_filters(&self) -> Vec<FilterChip> {
        let mut chips = Vec::new();
        if self.show_completed {
            chips.push(FilterChip::Completed);
        }
        if self.focus_only {
            chips.push(FilterChip::ActiveOnly);
        }
        if self.agent_filter != AgentFilter::Show {
            chips.push(FilterChip::Agent);
        }
        chips.extend(
            self.tag_states
                .set()
                .map(|(path, state)| FilterChip::Tag(path.to_owned(), state)),
        );
        if !self.search.trim().is_empty() {
            chips.push(FilterChip::Search);
        }
        if !(self.show_parent_graph
            && self.show_dependency_graph
            && self.show_blocker_graph
            && self.show_relations)
        {
            chips.push(FilterChip::Lines);
        }
        if !(self.show_tag_constellation
            && self.show_parent_regions
            && self.show_focus_window
            && self.show_timeline_ruler)
        {
            chips.push(FilterChip::Overlays);
        }
        chips
    }

    fn chip_label(&self, chip: &FilterChip) -> String {
        match chip {
            FilterChip::Completed => "showing done".to_owned(),
            FilterChip::ActiveOnly => "active only".to_owned(),
            FilterChip::Agent => match self.agent_filter {
                AgentFilter::Only => "agent tasks only".to_owned(),
                _ => "agent tasks hidden".to_owned(),
            },
            FilterChip::Tag(path, TagFilterState::Hidden) => format!("tag {path} hidden"),
            FilterChip::Tag(path, _) => format!("tag {path} only"),
            FilterChip::Search => format!("search “{}”", self.search.trim()),
            FilterChip::Lines => "some lines hidden".to_owned(),
            FilterChip::Overlays => "some overlays off".to_owned(),
        }
    }

    /// Returns one filter to its default, which is what clicking its chip does.
    fn clear_filter(&mut self, chip: &FilterChip) {
        match chip {
            FilterChip::Completed => self.show_completed = false,
            FilterChip::ActiveOnly => self.focus_only = false,
            FilterChip::Agent => self.agent_filter = AgentFilter::Show,
            FilterChip::Tag(path, _) => self.tag_states.reset(path),
            FilterChip::Search => self.search.clear(),
            FilterChip::Lines => {
                self.show_parent_graph = true;
                self.show_dependency_graph = true;
                self.show_blocker_graph = true;
                self.show_relations = true;
            }
            FilterChip::Overlays => {
                self.show_tag_constellation = true;
                self.show_parent_regions = true;
                self.show_focus_window = true;
                self.show_timeline_ruler = true;
            }
        }
    }

    /// The chip row above the canvas, shown only while something is filtered.
    fn draw_filter_bar(&mut self, ui: &mut egui::Ui, chips: &[FilterChip]) {
        ui.horizontal_wrapped(|ui| {
            ui.weak("filtered:");
            for chip in chips {
                if ui
                    .small_button(format!("{} ✕", self.chip_label(chip)))
                    .on_hover_text("Clear this filter")
                    .clicked()
                {
                    self.clear_filter(chip);
                }
            }
            if ui.small_button("clear all").clicked() {
                self.reset_filters();
            }
        });
    }

    /// The commands that rearrange the graph, listed in the toolbar's Layout menu.
    fn arrangement_buttons(&mut self, ui: &mut egui::Ui) {
        // The graph rect is only known while drawing, so each request is deferred
        // one frame rather than guessed at here.
        if ui.button("Normalize").clicked() {
            self.normalize_requested = true;
        }
        if ui.button("Focus layout").clicked() {
            self.focus_layout_requested = true;
        }
        if ui.button("Group").clicked() {
            self.group_requested = true;
        }
        if ui.button("Timeline").clicked() {
            self.timeline_requested = true;
        }
        ui.separator();
        if ui.button("Unstack").clicked() {
            self.relax_requested = true;
        }
        if ui.button("Separate tags").clicked() {
            self.separate_requested = true;
        }
        ui.separator();
        if ui.button("Undo layout").clicked() {
            self.undo_layout();
        }
    }

    /// Draws the Settings window: how the graph looks and moves, in tabs, with the
    /// theme-file actions in its footer. Changes apply live, as before.
    fn draw_settings(&mut self, ctx: &egui::Context) {
        let mut open = self.show_settings;
        egui::Window::new("Settings")
            .open(&mut open)
            .default_width(380.0)
            .default_height(460.0)
            .resizable(true)
            .show(ctx, |ui| {
                ui.horizontal_wrapped(|ui| {
                    for tab in SettingsTab::ALL {
                        ui.selectable_value(&mut self.settings_tab, tab, tab.label());
                    }
                });
                ui.separator();
                egui::Panel::bottom("settings-footer").show(ui, |ui| self.settings_footer(ui));
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| match self.settings_tab {
                        SettingsTab::Layout => self.view_settings(ui),
                        SettingsTab::Cards => self.card_settings(ui),
                        SettingsTab::Families => self.family_settings(ui),
                        SettingsTab::Tags => self.tag_settings(ui),
                        SettingsTab::Motion => self.force_settings(ui),
                        SettingsTab::Colors => self.color_settings(ui),
                        SettingsTab::Inspector => self.section_display_settings(ui),
                    });
            });
        self.show_settings = open;
    }

    /// Save, revert, and reload for `theme.toml`, with whether anything is unsaved.
    fn settings_footer(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            if self.theme == self.theme_loaded {
                ui.weak("theme.toml · saved");
            } else {
                ui.label("theme.toml · unsaved changes");
                if ui.button("Revert").clicked() {
                    self.theme = self.theme_loaded.clone();
                    self.colors_applied = false;
                }
            }
            if ui.button("Save to theme.toml").clicked() {
                match self.write_theme() {
                    Ok(()) => {
                        self.message =
                            Some(format!("Theme saved to {}", self.theme_path.display()));
                    }
                    Err(error) => self.message = Some(format!("Theme not saved: {error:#}")),
                }
            }
            if ui.button("Reload from disk").clicked() {
                self.reload_theme();
            }
        });
    }

    /// Color mode and the terminal palette the surfaces borrow from.
    fn color_settings(&mut self, ui: &mut egui::Ui) {
        ui.label("Mode");
        ui.horizontal(|ui| {
            for (mode, label) in [
                (ColorMode::Dark, "Dark"),
                (ColorMode::Light, "Light"),
                (ColorMode::System, "System"),
            ] {
                if ui
                    .selectable_value(&mut self.theme.colors.mode, mode, label)
                    .changed()
                {
                    self.colors_applied = false;
                }
            }
        });
        ui.label("Palette file (Ghostty format)");
        let palette = ui.add(
            egui::TextEdit::singleline(&mut self.theme.colors.palette_file)
                .hint_text("~/.config/ghostty/themes/…"),
        );
        if palette.lost_focus() {
            self.colors_applied = false;
            self.theme_seen = self.theme_stamps();
        }
        ui.weak("Restyles panels and canvas; card colors stay with tags and status.");
    }

    /// Modes and arrangement settings, the ones reached for while looking.
    fn view_settings(&mut self, ui: &mut egui::Ui) {
        ui.label("Hierarchy grows");
        ui.horizontal(|ui| {
            ui.selectable_value(
                &mut self.theme.layout.tree_axis,
                geometry::TreeAxis::Down,
                "Top down",
            );
            ui.selectable_value(
                &mut self.theme.layout.tree_axis,
                geometry::TreeAxis::Right,
                "Left to right",
            );
        });
        ui.label("Timeline runs");
        ui.horizontal(|ui| {
            ui.selectable_value(
                &mut self.theme.timeline.axis,
                geometry::TimelineAxis::LeftRight,
                "Left to right",
            );
            ui.selectable_value(
                &mut self.theme.timeline.axis,
                geometry::TimelineAxis::TopDown,
                "Top down",
            );
        });
        ui.horizontal(|ui| {
            ui.selectable_value(
                &mut self.theme.timeline.scale,
                geometry::TimelineScale::Elapsed,
                "Elapsed",
            );
            ui.selectable_value(
                &mut self.theme.timeline.scale,
                geometry::TimelineScale::Sequence,
                "Sequence",
            );
        });
        egui::ComboBox::from_id_salt("family-child-depth")
            .selected_text(if self.theme.parent.child_depth == 0 {
                "All".to_owned()
            } else {
                self.theme.parent.child_depth.to_string()
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut self.theme.parent.child_depth, 0, "All child depths");
                for depth in 1..=6 {
                    ui.selectable_value(
                        &mut self.theme.parent.child_depth,
                        depth,
                        format!("{depth} generation(s)"),
                    );
                }
            });
        ui.weak("Collapsed branches show an expand control; active paths stay visible.");
        ui.separator();
        ui.label("Group by, in order");
        for level in 0..3 {
            let Some(slot) = self.group_by.get_mut(level) else {
                continue;
            };
            egui::ComboBox::from_id_salt(("group-by", level))
                .selected_text(slot.label())
                .show_ui(ui, |ui| {
                    for grouping in GroupBy::all() {
                        ui.selectable_value(slot, grouping, grouping.label());
                    }
                });
        }
        ui.weak("First makes the columns; the rest divide them into blocks.");
        ui.label("Order within a group");
        egui::ComboBox::from_id_salt("sort-by")
            .selected_text(self.sort_by.label())
            .show_ui(ui, |ui| {
                for sort in SortBy::all() {
                    ui.selectable_value(&mut self.sort_by, sort, sort.label());
                }
            });
        ui.weak("The order tasks are placed in on the graph.");
        ui.separator();
        ui.checkbox(
            &mut self.theme.focus.parents_of_active_read_active,
            "Parents of active work read active",
        );
    }

    /// How each task card is drawn.
    fn card_settings(&mut self, ui: &mut egui::Ui) {
        ui.label("Shape");
        ui.horizontal(|ui| {
            ui.selectable_value(
                &mut self.theme.node.shape,
                NodeShape::RoundedRect,
                "Rounded",
            );
            ui.selectable_value(&mut self.theme.node.shape, NodeShape::Capsule, "Capsule");
            ui.selectable_value(&mut self.theme.node.shape, NodeShape::Circle, "Circle");
        });
        ui.checkbox(&mut self.theme.node.wrap_titles, "Wrap long titles");
        ui.checkbox(
            &mut self.theme.node.soften_outline_in_family,
            "Soften same-family card outlines",
        );
        ui.add_enabled_ui(self.theme.node.soften_outline_in_family, |ui| {
            ui.checkbox(
                &mut self.theme.node.hide_outline_in_family,
                "Hide them completely",
            );
        });
        ui.checkbox(
            &mut self.theme.edge.scale_arrowheads_with_zoom,
            "Scale arrowheads with zoom",
        );
    }

    /// How tag regions are shaped, colored, and linked.
    fn tag_settings(&mut self, ui: &mut egui::Ui) {
        ui.label("Tag clouds");
        ui.horizontal(|ui| {
            ui.selectable_value(
                &mut self.theme.tag.region_shape,
                RegionShape::Cloud,
                "Clouds",
            );
            ui.selectable_value(&mut self.theme.tag.region_shape, RegionShape::Hull, "Hull");
            ui.selectable_value(
                &mut self.theme.tag.region_shape,
                RegionShape::Rect,
                "Rectangles",
            );
        });
        ui.add_enabled_ui(self.theme.tag.region_shape == RegionShape::Cloud, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(
                    &mut self.theme.tag.cloud_member_shape,
                    CloudMemberShape::Circle,
                    "Circles",
                );
                ui.selectable_value(
                    &mut self.theme.tag.cloud_member_shape,
                    CloudMemberShape::Card,
                    "Card clouds",
                );
            });
        });
        ui.weak("Card clouds follow each task card and connect with square-ended rails.");
        ui.add(
            egui::Slider::new(&mut self.theme.tag.cloud_hierarchy_depth, 0..=8)
                .custom_formatter(|depth, _| {
                    if depth == 0.0 {
                        "All".to_owned()
                    } else {
                        format!("{depth:.0}")
                    }
                })
                .text("Cloud hierarchy depth"),
        );
        ui.add(
            egui::Slider::new(&mut self.theme.tag.branch_hue_spread, 0.0..=1.0)
                .text("Sibling hue span"),
        );
        ui.add(
            egui::Slider::new(&mut self.theme.tag.depth_hue_drift, -0.08..=0.08)
                .text("Depth hue drift"),
        );
        ui.horizontal(|ui| {
            ui.label("Cloud links");
            ui.selectable_value(
                &mut self.theme.tag.cloud_link_mode,
                TagCloudLinkMode::Nearest,
                "Nearest",
            );
            ui.selectable_value(
                &mut self.theme.tag.cloud_link_mode,
                TagCloudLinkMode::WithinRadius,
                "Within radius",
            );
        });
        ui.add_enabled_ui(
            self.theme.tag.cloud_link_mode == TagCloudLinkMode::Nearest,
            |ui| {
                ui.add(
                    egui::Slider::new(&mut self.theme.tag.cloud_neighbor_count, 0..=12)
                        .text("Nearest neighbors"),
                );
            },
        );
        ui.add_enabled_ui(
            self.theme.tag.cloud_link_mode == TagCloudLinkMode::WithinRadius,
            |ui| {
                ui.add(
                    egui::Slider::new(&mut self.theme.tag.cloud_link_radius, 20.0..=800.0)
                        .text("Link radius"),
                );
            },
        );
        ui.checkbox(&mut self.theme.tag.cloud_fill_gaps, "Fill local cloud gaps");
        ui.checkbox(
            &mut self.theme.tag.cloud_cut_out_unrelated,
            "Cut out unrelated hierarchy clouds",
        );
        ui.add_enabled_ui(self.theme.tag.cloud_cut_out_unrelated, |ui| {
            ui.checkbox(&mut self.theme.tag.cloud_cutout_outline, "Outline cutouts");
        });
    }

    /// How parent tasks hold and connect their children.
    fn family_settings(&mut self, ui: &mut egui::Ui) {
        ui.label("Task families");
        egui::ComboBox::from_id_salt("family-header-style")
            .selected_text(match self.theme.parent.header_style {
                FamilyHeaderStyle::Header => "Header",
                FamilyHeaderStyle::AttachedTab => "Attached tab",
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(
                    &mut self.theme.parent.header_style,
                    FamilyHeaderStyle::Header,
                    "Header",
                );
                ui.selectable_value(
                    &mut self.theme.parent.header_style,
                    FamilyHeaderStyle::AttachedTab,
                    "Attached tab",
                );
            });
        egui::ComboBox::from_id_salt("family-child-style")
            .selected_text(match self.theme.parent.child_style {
                FamilyChildStyle::RecessedBoard => "Recessed board",
                FamilyChildStyle::NestedFamily => "Nested family",
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(
                    &mut self.theme.parent.child_style,
                    FamilyChildStyle::RecessedBoard,
                    "Recessed board",
                );
                ui.selectable_value(
                    &mut self.theme.parent.child_style,
                    FamilyChildStyle::NestedFamily,
                    "Nested family",
                );
            });
        egui::ComboBox::from_id_salt("family-well-fill")
            .selected_text(match self.theme.parent.well_fill {
                FamilyWellFill::Transparent => "Transparent child space",
                FamilyWellFill::Dark => "Dark child space",
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(
                    &mut self.theme.parent.well_fill,
                    FamilyWellFill::Transparent,
                    "Transparent child space",
                );
                ui.selectable_value(
                    &mut self.theme.parent.well_fill,
                    FamilyWellFill::Dark,
                    "Dark child space",
                );
            });
        ui.checkbox(
            &mut self.theme.parent.clip_well_below_attached_tab,
            "Keep child fill below attached card",
        );
        ui.label("Dynamic family boxes");
        ui.add(
            egui::Slider::new(&mut self.theme.parent.dynamic_min_children, 1..=12)
                .text("Children required (n)"),
        );
        ui.add(
            egui::Slider::new(
                &mut self.theme.parent.dynamic_cluster_distance,
                20.0..=800.0,
            )
            .text("Cluster range (m)"),
        );
        ui.add(
            egui::Slider::new(&mut self.theme.parent.parent_join_max_gap, 0.0..=240.0)
                .text("Parent-to-rim gap"),
        );
        ui.add(
            egui::Slider::new(&mut self.theme.parent.parent_join_width_ratio, 0.0..=1.5)
                .text("Parent join / card width"),
        );
        ui.weak("Dynamic boxes use n and m. A nearby aligned box joins its parent rim; other boxes keep the normal family tree.");
        ui.add(
            egui::Slider::new(&mut self.theme.parent.outline_weight, 1.0..=5.0)
                .text("Parent outline weight"),
        );
        ui.horizontal(|ui| {
            ui.label("Connector");
            ui.selectable_value(
                &mut self.theme.parent.connector_style,
                ConnectorStyle::FillCore,
                "Family core",
            );
            ui.selectable_value(
                &mut self.theme.parent.connector_style,
                ConnectorStyle::BlueCore,
                "Family casing",
            );
        });
    }

    /// The forces that arrange the graph, adjustable while watching them work.
    ///
    /// These edit the live theme, so a change shows on the next layout command
    /// rather than on the next restart. Save to theme.toml writes them down.
    fn force_settings(&mut self, ui: &mut egui::Ui) {
        ui.checkbox(&mut self.relax_overlaps, "Push overlaps apart");
        ui.checkbox(
            &mut self.theme.layout.separate_after_layout,
            "Separate tag groups",
        );
        ui.checkbox(&mut self.theme.layout.compact_active, "Pack task families");
        ui.separator();
        ui.label("Spacing");
        ui.add(egui::Slider::new(&mut self.theme.layout.leaf_gap, 60.0..=500.0).text("Leaf gap"));
        ui.add(egui::Slider::new(&mut self.theme.layout.row_gap, 60.0..=500.0).text("Row gap"));
        ui.add(
            egui::Slider::new(&mut self.theme.layout.active_head_gap, 80.0..=600.0)
                .text("Family gap"),
        );
        ui.separator();
        ui.label("Overlap");
        ui.add(egui::Slider::new(&mut self.theme.layout.overlap_gap, 0.0..=160.0).text("Gap"));
        ui.add(
            egui::Slider::new(&mut self.theme.layout.overlap_strength, 0.05..=1.0).text("Strength"),
        );
        ui.add(
            egui::Slider::new(&mut self.theme.layout.overlap_iterations, 1..=200).text("Passes"),
        );
        ui.separator();
        ui.label("Tag separation");
        ui.add(
            egui::Slider::new(&mut self.theme.layout.separation_attraction, 0.0..=1.0)
                .text("Pull to own group"),
        );
        ui.add(
            egui::Slider::new(&mut self.theme.layout.separation_repulsion, 0.0..=60.0)
                .text("Push from others"),
        );
        ui.add(
            egui::Slider::new(&mut self.theme.layout.separation_radius, 50.0..=1200.0)
                .text("Reach"),
        );
        ui.add(
            egui::Slider::new(&mut self.theme.layout.separation_iterations, 1..=60).text("Passes"),
        );
        ui.separator();
        ui.label("Glide");
        ui.checkbox(&mut self.theme.layout.animate, "Glide to new positions");
        ui.add(
            egui::Slider::new(&mut self.theme.layout.transition_ms, 0.0..=1500.0)
                .text("Glide milliseconds"),
        );
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

    /// Draws the inspector in everyday editing order, with rare controls last.
    ///
    /// Grouping keeps the panel navigable: status, then how the task relates to
    /// other work, then its content, then its timing, with the raw editing surface
    /// tucked away last. Each group's open state starts from the theme.
    fn draw_inspector(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                let Some(item) = self.selected_item().cloned() else {
                    ui.weak("Select a task node to inspect it.");
                    if let Some(message) = self.message.clone() {
                        ui.separator();
                        ui.label(message);
                    }
                    return;
                };
                self.inspector_header(ui, &item);
                ui.separator();
                self.band_people(ui, &item);
                egui::CollapsingHeader::new("Family & dependencies")
                    .default_open(self.inspector.open_relationships)
                    .show(ui, |ui| self.section_relationships(ui, &item));
                egui::CollapsingHeader::new("Tags, notes & links")
                    .default_open(self.inspector.open_details)
                    .show(ui, |ui| self.section_details(ui, &item));
                self.band_time(ui, &item);
                ui.separator();
                self.inspector_footer(ui, &item);
                if let Some(message) = self.message.clone() {
                    ui.separator();
                    ui.label(message);
                }
            });
    }

    /// The title, state and priority chips, spoken id, and parent link.
    ///
    /// The title reads as a heading and edits in place: click it or `✎`, then
    /// Enter saves and Escape restores it. A chip's menu saves on choice, so no
    /// edit here needs a Save button.
    fn inspector_header(&mut self, ui: &mut egui::Ui, item: &StoredItem) {
        if self.editing == Some(Edit::Title) {
            let field = ui.add(
                egui::TextEdit::singleline(&mut self.title_draft)
                    .desired_width(f32::INFINITY)
                    .font(egui::TextStyle::Heading),
            );
            field.request_focus();
            if ui.input(|input| input.key_pressed(egui::Key::Escape)) {
                self.title_draft = item.item.title.clone();
                self.editing = None;
            } else if field.lost_focus() {
                // Enter and clicking away both commit, as in any inline editor.
                if self.title_draft.trim() != item.item.title {
                    self.save_title();
                }
                self.editing = None;
            }
        } else {
            ui.horizontal_wrapped(|ui| {
                let title = ui.add(
                    egui::Label::new(egui::RichText::new(&item.item.title).heading())
                        .sense(Sense::click()),
                );
                let pencil = ui.small_button("✎");
                if title.on_hover_text("Click to edit").clicked() || pencil.clicked() {
                    self.title_draft = item.item.title.clone();
                    self.editing = Some(Edit::Title);
                }
            });
        }
        ui.horizontal_wrapped(|ui| {
            let holds = ids_holding_active_work(&self.items).contains(&item.item.id.to_string());
            let state = egui::RichText::new(format!("{} ▾", item.item.state))
                .color(status_color(item, holds));
            ui.menu_button(state, |ui| {
                for state in [
                    ItemState::Inbox,
                    ItemState::Active,
                    ItemState::Done,
                    ItemState::Canceled,
                    ItemState::Void,
                ]
                .into_iter()
                .filter(|state| item.item.state.can_transition_to(*state))
                {
                    if ui.button(state.to_string()).clicked() {
                        self.state_draft = state;
                        self.save_state();
                    }
                }
            });
            ui.menu_button(format!("{} ▾", item.item.priority), |ui| {
                for priority in [
                    Priority::Low,
                    Priority::Normal,
                    Priority::High,
                    Priority::Critical,
                ] {
                    if ui.button(priority.to_string()).clicked() {
                        self.priority_draft = priority;
                        self.save_priority();
                    }
                }
            });
            let done = item.item.state == ItemState::Done;
            if ui
                .small_button(if done { "↺" } else { "✓" })
                .on_hover_text(if done { "Reopen" } else { "Complete" })
                .clicked()
            {
                self.toggle_complete();
            }
            let short = item.item.id.short();
            if ui
                .small_button(format!("{short} ⧉"))
                .on_hover_text("Copy the spoken id")
                .clicked()
            {
                ui.ctx().copy_text(short.to_string());
            }
        });
        if let Some(parent) = item.item.parent_id
            && self.inspector.show_parent
        {
            let title = self
                .items
                .iter()
                .find(|candidate| candidate.item.id == parent)
                .map_or_else(
                    || parent.short().to_string(),
                    |found| found.item.title.clone(),
                );
            ui.horizontal(|ui| {
                ui.weak("in");
                if ui.link(title).clicked() {
                    self.select(parent.to_string());
                }
            });
        }
    }

    /// History, raw JSON, and the full id: rarely needed, so quiet and last.
    fn inspector_footer(&mut self, ui: &mut egui::Ui, item: &StoredItem) {
        let id = item.item.id.to_string();
        egui::CollapsingHeader::new(egui::RichText::new("history").weak())
            .id_salt("inspector-history")
            .show(ui, |ui| {
                if self
                    .history
                    .as_ref()
                    .is_none_or(|(loaded, _)| *loaded != id)
                {
                    let subject = item.item.id;
                    match self.with_ledger(async |ledger| ledger.history(subject).await) {
                        Ok(entries) => self.history = Some((id.clone(), entries)),
                        Err(error) => {
                            self.message = Some(format!("History not loaded: {error:#}"));
                            self.history = Some((id.clone(), Vec::new()));
                        }
                    }
                }
                for entry in self.history.iter().flat_map(|(_, entries)| entries) {
                    let agent = entry
                        .agent
                        .as_deref()
                        .map(|agent| format!(" · {agent}"))
                        .unwrap_or_default();
                    ui.weak(format!("{}  {}{agent}", entry.recorded_at, entry.kind));
                }
            });
        if self.inspector.show_raw_json {
            egui::CollapsingHeader::new(egui::RichText::new("raw").weak())
                .id_salt("inspector-raw")
                .default_open(self.inspector.open_advanced)
                .show(ui, |ui| self.section_advanced(ui));
        }
        // Weight is computed for display; nothing is stored for it.
        let children = child_index(&self.items);
        ui.weak(format!(
            "weight {:.2}{}",
            item_weight(item, &children, &self.theme),
            if self.theme.weight.size_nodes_by_weight {
                " · sizes the card"
            } else {
                ""
            }
        ));
        ui.horizontal(|ui| {
            ui.weak(format!("id {id}"));
            if ui
                .small_button("⧉")
                .on_hover_text("Copy the full id")
                .clicked()
            {
                ui.ctx().copy_text(id.clone());
            }
        });
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
            let mut show_parent_suggestions = false;
            ui.horizontal(|ui| {
                let search = ui.add(
                    egui::TextEdit::singleline(&mut self.parent_search)
                        .hint_text("Search parent tasks"),
                );
                show_parent_suggestions =
                    search.hovered() || search.has_focus() || !self.parent_search.trim().is_empty();
                if ui.button("Clear parent").clicked() {
                    self.parent_draft = None;
                    self.save_parent();
                }
            });
            if show_parent_suggestions {
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
            let dependency_search = ui.add(
                egui::TextEdit::singleline(&mut self.dependency_search)
                    .hint_text("Search prerequisite tasks"),
            );
            if dependency_search.hovered()
                || dependency_search.has_focus()
                || !self.dependency_search.trim().is_empty()
            {
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
            let blocker_search = ui.add(
                egui::TextEdit::singleline(&mut self.blocker_search)
                    .hint_text("Search blocking tasks"),
            );
            if blocker_search.hovered()
                || blocker_search.has_focus()
                || !self.blocker_search.trim().is_empty()
            {
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
            let relation_search = ui.add(
                egui::TextEdit::singleline(&mut self.relation_search)
                    .hint_text("Search related tasks"),
            );
            if relation_search.hovered()
                || relation_search.has_focus()
                || !self.relation_search.trim().is_empty()
            {
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

    /// Starts editing one inspector value in place.
    fn begin(&mut self, edit: Edit) {
        self.editing = Some(edit);
        self.edit_focus = true;
    }

    /// Draws one inspector band: a colored stripe down its left edge, a colored
    /// label, a one-line summary while collapsed, and a `+ ▾` add menu.
    ///
    /// The color is only ever a stripe and a label, never a fill behind text. An
    /// empty band dims both, so it reads as "nothing here yet" at a glance.
    #[allow(
        clippy::too_many_arguments,
        reason = "each band differs in exactly these parts; a struct would only rename them"
    )]
    fn band(
        &mut self,
        ui: &mut egui::Ui,
        label: &str,
        color: Color32,
        open: bool,
        summary: String,
        empty: bool,
        add: impl FnOnce(&mut Self, &mut egui::Ui),
        body: impl FnOnce(&mut Self, &mut egui::Ui),
    ) {
        let id = ui.make_persistent_id(("band", label));
        let state =
            egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, open);
        let collapsed = !state.is_open();
        let color = if empty {
            color.gamma_multiply(0.5)
        } else {
            color
        };
        let frame = egui::Frame::NONE
            .inner_margin(egui::Margin {
                left: 8,
                ..Default::default()
            })
            .show(ui, |ui| {
                state
                    .show_header(ui, |ui| {
                        ui.label(egui::RichText::new(label).color(color).strong());
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.menu_button("+ ▾", |ui| add(self, ui));
                            if collapsed && !summary.is_empty() {
                                ui.add(
                                    egui::Label::new(egui::RichText::new(summary).weak())
                                        .truncate(),
                                );
                            }
                        });
                    })
                    .body(|ui| body(self, ui));
            });
        let rect = frame.response.rect;
        ui.painter().rect_filled(
            Rect::from_min_size(rect.min, Vec2::new(3.0, rect.height())),
            1.0,
            color,
        );
        ui.add_space(4.0);
    }

    /// Who made, did, owns, or asked for the task, and who signed its creation.
    fn band_people(&mut self, ui: &mut egui::Ui, item: &StoredItem) {
        let summary = self
            .roles
            .iter()
            .map(|(role, actor)| format!("{role} {}", actor_name(actor)))
            .collect::<Vec<_>>()
            .join(" · ");
        let empty = self.roles.is_empty() && item.agent.is_none();
        let color = band_color(&self.theme.colors.inspector.people);
        self.band(
            ui,
            "PEOPLE",
            color,
            self.inspector.open_status,
            summary,
            empty,
            |app, ui| {
                for role in ROLES {
                    if ui.button(role).clicked() {
                        app.role_draft = role.to_owned();
                        app.actor_draft.clear();
                        app.begin(Edit::Person);
                    }
                }
            },
            |app, ui| {
                if let Some(agent) = &item.agent {
                    inspector_row(ui, "signed", format!("{agent} · agent"), false);
                }
                for (role, actor) in app.roles.clone() {
                    if inspector_row(ui, &role, actor_name(&actor), true).1 {
                        app.remove_role(&role, &actor);
                    }
                }
                if app.editing == Some(Edit::Person) {
                    app.person_editor(ui);
                }
            },
        );
    }

    /// The add-a-person row: the chosen role, a name, and known names to pick.
    fn person_editor(&mut self, ui: &mut egui::Ui) {
        let mut outcome = ui
            .horizontal(|ui| {
                inspector_key(ui, &self.role_draft);
                inline_text(ui, &mut self.actor_draft, &mut self.edit_focus, "name")
            })
            .inner;
        let query = self.actor_draft.trim().to_lowercase();
        let matches = self
            .namespaces
            .iter()
            .filter(|entry| entry.kind == NamespaceKind::Actor)
            .map(|entry| actor_name(&entry.path).to_owned())
            .filter(|name| name.contains(&query) && *name != query)
            .take(5)
            .collect::<Vec<_>>();
        ui.horizontal_wrapped(|ui| {
            for name in matches {
                if ui.small_button(&name).clicked() {
                    self.actor_draft = name;
                    outcome = Some(true);
                }
            }
        });
        ui.weak("Enter adds · Esc cancels");
        match outcome {
            Some(true) => {
                self.add_role();
                self.editing = None;
            }
            Some(false) => self.editing = None,
            None => {}
        }
    }

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
        let mut show_tag_suggestions = false;
        ui.horizontal(|ui| {
            let tag_input = ui
                .add(egui::TextEdit::singleline(&mut self.tag_draft).hint_text("Search tag paths"));
            show_tag_suggestions =
                tag_input.hovered() || tag_input.has_focus() || !self.tag_draft.trim().is_empty();
            if ui.button("Add tag").clicked() {
                self.add_tag();
            }
        });
        if show_tag_suggestions {
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

    /// Dates, effort, and work sessions. Only fields with a value get a row;
    /// `+ ▾` offers the rest.
    fn band_time(&mut self, ui: &mut egui::Ui, item: &StoredItem) {
        let today = Local::now().date_naive();
        let values = Edit::TIME.map(|edit| (edit, time_value(item, edit, today)));
        let running = self
            .sessions
            .iter()
            .any(|session| session.finished_at.is_none());
        let summary = values
            .iter()
            .filter_map(|(edit, value)| Some(format!("{} {}", edit.label(), value.as_ref()?)))
            .chain(running.then(|| "session running".to_owned()))
            .collect::<Vec<_>>()
            .join(" · ");
        let empty = values.iter().all(|(_, value)| value.is_none()) && self.sessions.is_empty();
        let unset = values
            .iter()
            .filter(|(_, value)| value.is_none())
            .map(|(edit, _)| *edit)
            .collect::<Vec<_>>();
        let color = band_color(&self.theme.colors.inspector.time);
        self.band(
            ui,
            "TIME",
            color,
            self.inspector.open_schedule,
            summary,
            empty,
            |app, ui| {
                if app.inspector.show_dates {
                    for edit in unset {
                        if ui.button(edit.label()).clicked() {
                            app.time_draft(edit).clear();
                            app.begin(edit);
                        }
                    }
                }
                if app.inspector.show_sessions {
                    ui.separator();
                    let label = if running {
                        "stop session"
                    } else {
                        "start session"
                    };
                    if ui.button(label).clicked() {
                        app.toggle_session();
                    }
                }
            },
            |app, ui| {
                if app.inspector.show_dates {
                    for (edit, value) in values {
                        if app.editing == Some(edit) {
                            app.time_editor(ui, edit, item);
                        } else if let Some(value) = value {
                            let (clicked, remove) = inspector_row(ui, edit.label(), value, true);
                            if clicked {
                                app.begin(edit);
                            }
                            if remove {
                                app.time_draft(edit).clear();
                                app.commit_time(edit);
                            }
                        }
                    }
                }
                if app.inspector.show_sessions && !app.sessions.is_empty() {
                    let count = app.sessions.len();
                    let state = if running { " · running" } else { "" };
                    inspector_row(ui, "sessions", format!("{count}{state}"), false);
                }
            },
        );
    }

    /// The in-place editor for one time field, with Today and a calendar for
    /// dates. Escape reloads the drafts so the field shows its stored value.
    fn time_editor(&mut self, ui: &mut egui::Ui, edit: Edit, item: &StoredItem) {
        let mut focus = std::mem::take(&mut self.edit_focus);
        let hint = match edit {
            Edit::Estimate => "minutes",
            Edit::Difficulty => "1 to 5",
            _ => "YYYY-MM-DD",
        };
        let outcome = ui
            .horizontal(|ui| {
                inspector_key(ui, edit.label());
                let mut outcome = inline_text(ui, self.time_draft(edit), &mut focus, hint);
                if edit.is_date() {
                    if ui.small_button("today").clicked() {
                        *self.time_draft(edit) = Local::now().date_naive().to_string();
                        outcome = Some(true);
                    }
                    calendar_picker(ui, edit.label(), self.time_draft(edit));
                }
                outcome
            })
            .inner;
        match outcome {
            Some(true) => {
                self.commit_time(edit);
                self.editing = None;
            }
            Some(false) => {
                self.editing = None;
                self.select(item.item.id.to_string());
            }
            None => {}
        }
    }

    /// The draft behind an editable field.
    fn time_draft(&mut self, edit: Edit) -> &mut String {
        match edit {
            Edit::Title => &mut self.title_draft,
            Edit::Person => &mut self.actor_draft,
            Edit::Due => &mut self.due_draft,
            Edit::Defer => &mut self.defer_draft,
            Edit::Scheduled => &mut self.scheduled_draft,
            Edit::Estimate => &mut self.estimate_draft,
            Edit::Difficulty => &mut self.difficulty_draft,
        }
    }

    /// Saves a time field from its draft, where an empty draft clears it.
    fn commit_time(&mut self, edit: Edit) {
        match edit {
            Edit::Due => self.save_date(true),
            Edit::Defer => self.save_date(false),
            Edit::Scheduled => self.save_scheduled(),
            Edit::Estimate => self.save_estimate(),
            Edit::Difficulty => self.save_difficulty(),
            Edit::Title | Edit::Person => {}
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
            axis: self.theme.timeline.axis,
            scale: self.theme.timeline.scale,
        };
        let arranged = geometry::timeline_positions(&scheduled, spacing);
        let origin = Pos2::new(rect.center().x, rect.top() + 140.0);
        self.remember_layout();
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
        self.group_labels.clear();
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
            self.remember_layout();
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
            self.remember_layout();
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
        let holding_active = if self.theme.focus.parents_of_active_read_active {
            ids_holding_active_work(&items)
        } else {
            std::collections::HashSet::new()
        };
        let mut completed = Vec::new();
        let mut active = Vec::new();
        let mut future = Vec::new();
        for item in &items {
            let id = item.item.id.to_string();
            if focus_is_active(item, &holding_active, &self.theme.focus, today) {
                active.push(id);
            } else if matches!(item.item.state, ItemState::Done | ItemState::Canceled) {
                completed.push(id);
            } else {
                future.push(id);
            }
        }
        // Each band is placed in the chosen order, so the sort reaches the Focus
        // Window and not only the grouped view.
        let weights = self.weight_index(&items);
        let sequence_rank = self.sequence_ranks();
        let sort = self.sort_by;
        let order = |band: &mut Vec<String>| {
            band.sort_by(|left, right| {
                let find = |id: &String| {
                    items
                        .iter()
                        .find(|candidate| candidate.item.id.to_string() == *id)
                };
                match (find(left), find(right)) {
                    (Some(first), Some(second)) => {
                        sort_items(sort, first, second, &weights, &sequence_rank)
                    }
                    _ => left.cmp(right),
                }
            });
        };
        order(&mut completed);
        order(&mut active);
        order(&mut future);
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
        self.remember_layout();
        for (index, item) in items.iter().enumerate() {
            let id = item.item.id.to_string();
            let Some(target) = arranged.get(&id) else {
                continue;
            };
            let slot = grid_slot(index, rect);
            self.node_offsets.insert(id, (center + *target) - slot);
        }
        self.save_layout();
        // Tag attraction and generic repulsion do not know about the focus
        // exclusion boundary; running them here can pull exiled cards back in.
        self.timeline_origin = None;
        self.focus_mode = true;
        if self.theme.focus.zoom_to_active {
            self.frame_active(rect, &active);
        }
        self.group_labels.clear();
        self.message =
            Some("Focus Window arranged: done above, active centered, future below.".to_owned());
    }

    /// Hangs each task's subtree beneath it, with active work leading.
    ///
    /// Roots are ordered so active work, and work holding active work, comes first,
    /// which puts what is being worked on across the top with its children below it.
    /// Finished prerequisites are lifted into a band above, where they read as
    /// context rather than as work still waiting.
    fn hierarchy_layout(&mut self, rect: Rect) {
        let items = self.visible_items();
        let visible = items
            .iter()
            .map(|item| item.item.id.to_string())
            .collect::<std::collections::HashSet<_>>();
        let holding = ids_holding_active_work(&items);
        let mut children: std::collections::BTreeMap<String, Vec<String>> =
            std::collections::BTreeMap::new();
        let mut roots = Vec::new();
        for item in &items {
            let id = item.item.id.to_string();
            match item.item.parent_id.map(|parent| parent.to_string()) {
                Some(parent) if visible.contains(&parent) => {
                    children.entry(parent).or_default().push(id);
                }
                // A task whose parent is filtered out leads its own tree rather
                // than disappearing into one that is not on screen.
                _ => roots.push(id),
            }
        }
        let weights = self.weight_index(&items);
        let sequence_rank = self.sequence_ranks();
        let sort = self.sort_by;
        let compare_ids = |left: &String, right: &String| {
            let find = |id: &String| {
                items
                    .iter()
                    .find(|candidate| candidate.item.id.to_string() == *id)
            };
            match (find(left), find(right)) {
                (Some(first), Some(second)) => {
                    sort_items(sort, first, second, &weights, &sequence_rank)
                }
                _ => left.cmp(right),
            }
        };
        // A tree is only scannable when its sibling order is meaningful. Keeping
        // this separate from placement lets every arrangement share Sort by.
        for siblings in children.values_mut() {
            siblings.sort_by(&compare_ids);
        }
        let lifted = if self.theme.layout.done_above {
            self.finished_prerequisites(&items)
        } else {
            std::collections::HashSet::new()
        };
        let leading = |id: &String| {
            let item = items
                .iter()
                .find(|candidate| candidate.item.id.to_string() == *id);
            !item.is_some_and(|item| item.item.state == ItemState::Active || holding.contains(id))
        };
        roots.retain(|id| !lifted.contains(id));
        roots.sort_by(|left, right| {
            leading(left)
                .cmp(&leading(right))
                .then_with(|| compare_ids(left, right))
        });
        let spacing = TreeSpacing {
            leaf_gap: self.theme.layout.leaf_gap,
            row_gap: self.theme.layout.row_gap,
            axis: self.theme.layout.tree_axis,
        };
        let levels = hierarchy_levels(&items, &visible);
        let mut arranged = geometry::tree_positions_at_levels(&roots, &children, &levels, spacing);
        if self.theme.layout.compact_active {
            let bands = family_dependency_bands(&items, &roots, &children);
            arranged = geometry::pack_tree_families(
                &roots,
                &children,
                &arranged,
                &bands,
                Vec2::new(
                    self.theme.node.size().0 + self.theme.parent.padding * 4.0,
                    self.theme.node.max_height.min(110.0) + self.theme.parent.padding * 4.0,
                ),
                Vec2::splat(self.theme.layout.active_head_gap * 0.25),
            );
        }
        // Finished prerequisites sit above the work that needed them, so a glance
        // down the column reads as what was done, then what is being done.
        let mut lifted_column = 0.0_f32;
        for id in lifted {
            let above = items
                .iter()
                .filter(|candidate| {
                    candidate
                        .dependencies
                        .iter()
                        .any(|dependency| dependency.to_string() == id)
                })
                .find_map(|dependent| {
                    arranged
                        .get(&dependent.item.id.to_string())
                        .map(|placed| placed.x)
                });
            let x = above.unwrap_or_else(|| {
                lifted_column += spacing.leaf_gap;
                lifted_column
            });
            arranged.insert(id, Vec2::new(x, -self.theme.layout.done_band_gap));
        }
        let center = if arranged.is_empty() {
            Vec2::ZERO
        } else {
            let min = Vec2::new(
                arranged
                    .values()
                    .map(|at| at.x)
                    .fold(f32::INFINITY, f32::min),
                arranged
                    .values()
                    .map(|at| at.y)
                    .fold(f32::INFINITY, f32::min),
            );
            let max = Vec2::new(
                arranged
                    .values()
                    .map(|at| at.x)
                    .fold(f32::NEG_INFINITY, f32::max),
                arranged
                    .values()
                    .map(|at| at.y)
                    .fold(f32::NEG_INFINITY, f32::max),
            );
            (min + max) / 2.0
        };
        self.apply_arrangement(rect, &items, &arranged, rect.center() - center);
        // The tree spacing is intentional: a generic repulsion pass may push a
        // child across a sibling or out of its parent's containment region.
        // Manual Unstack remains available for unusually tall wrapped cards.
        self.timeline_origin = None;
        self.focus_mode = false;
        self.frame_visible_nodes(rect);
        self.group_labels.clear();
        self.message =
            Some("Hierarchy arranged. Undo layout restores the previous one.".to_owned());
    }

    /// Arranges the graph into labeled columns by the chosen groupings.
    ///
    /// The first grouping makes the columns; the second and third divide each
    /// column into blocks separated by space, so three levels read on one canvas
    /// without a column per combination. Within the deepest block, the sort order
    /// decides the order tasks appear in.
    ///
    /// Tasks holding none of a grouped property land in their own column or block
    /// rather than being dropped, which is how untagged or unsequenced work becomes
    /// findable instead of merely absent.
    fn group_layout(&mut self, rect: Rect) {
        let items = self.visible_items();
        if items.is_empty() {
            self.message = Some("Nothing to group.".to_owned());
            return;
        }
        if self.group_by.first().copied() == Some(GroupBy::Hierarchy) {
            // A hierarchy is not a column category: its useful order is parent
            // before child, with the child inside the parent's visual family.
            self.hierarchy_layout(rect);
            self.message = Some("Grouped by structural family.".to_owned());
            return;
        }
        let weights = self.weight_index(&items);
        let levels = self
            .group_by
            .iter()
            .copied()
            .filter(|level| *level != GroupBy::None)
            .collect::<Vec<_>>();
        let sort = self.sort_by;
        let sequence_rank = self.sequence_ranks();
        let mut columns: std::collections::BTreeMap<String, Vec<&StoredItem>> =
            std::collections::BTreeMap::new();
        for item in &items {
            let key = levels.first().map_or_else(
                || "all tasks".to_owned(),
                |level| self.group_key(item, *level),
            );
            columns.entry(key).or_default().push(item);
        }
        let first_group = levels.first().copied().unwrap_or(GroupBy::None);
        let mut ordered_columns = columns.into_iter().collect::<Vec<_>>();
        ordered_columns
            .sort_by(|(left, _), (right, _)| compare_group_labels(first_group, left, right));
        let mut ordered = Vec::new();
        let mut labels = Vec::new();
        for (column_index, (name, mut members)) in ordered_columns.into_iter().enumerate() {
            // Sorting by the deeper group keys first keeps each block together;
            // the chosen sort then orders tasks inside a block.
            members.sort_by(|left, right| {
                for level in levels.iter().skip(1) {
                    let ordering = compare_group_labels(
                        *level,
                        &self.group_key(left, *level),
                        &self.group_key(right, *level),
                    );
                    if ordering != std::cmp::Ordering::Equal {
                        return ordering;
                    }
                }
                sort_items(sort, left, right, &weights, &sequence_rank)
            });
            let mut rows = Vec::new();
            let mut previous: Option<Vec<String>> = None;
            for item in members {
                let keys = levels
                    .iter()
                    .skip(1)
                    .map(|level| self.group_key(item, *level))
                    .collect::<Vec<_>>();
                // A change at a shallower level leaves more space than a change at
                // a deeper one, so the nesting is visible in the spacing itself.
                let break_before = match &previous {
                    None => 0,
                    Some(before) => keys
                        .iter()
                        .zip(before.iter())
                        .position(|(now, then)| now != then)
                        .map_or(0, |depth| if depth == 0 { 2 } else { 1 }),
                };
                if break_before > 0
                    && let Some(sub) = keys.first()
                {
                    labels.push((sub.clone(), column_index, rows.len(), break_before));
                }
                rows.push(geometry::GroupMember {
                    id: item.item.id.to_string(),
                    break_before,
                });
                previous = Some(keys);
            }
            ordered.push((name, rows));
        }
        let spacing = GroupSpacing {
            column_gap: self.theme.layout.group_column_gap,
            row_gap: self.theme.layout.group_row_gap,
        };
        let arranged = geometry::grouped_positions(&ordered, spacing);
        let origin = Pos2::new(rect.left() + 220.0, rect.top() + 190.0);
        self.apply_arrangement(rect, &items, &arranged, origin);
        // Headings are remembered in graph space, so they follow the pan and zoom
        // exactly as the columns they name do.
        self.group_labels = ordered
            .iter()
            .enumerate()
            .map(|(column, (name, _))| {
                let x = f32::from(u16::try_from(column).unwrap_or(u16::MAX)) * spacing.column_gap;
                (name.clone(), origin + Vec2::new(x, -70.0))
            })
            .collect();
        for (name, column, row, _) in labels {
            let x = f32::from(u16::try_from(column).unwrap_or(u16::MAX)) * spacing.column_gap;
            let y = f32::from(u16::try_from(row).unwrap_or(u16::MAX)) * spacing.row_gap;
            self.group_labels
                .push((name, origin + Vec2::new(x, y - spacing.row_gap / 2.0)));
        }
        self.relax_if_enabled(rect);
        self.timeline_origin = None;
        self.focus_mode = false;
        self.message = Some(format!(
            "Grouped by {}, ordered by {}.",
            self.grouping_description(),
            self.sort_by.label().to_lowercase()
        ));
    }

    /// Returns the groupings in force, for the message after arranging.
    fn grouping_description(&self) -> String {
        let named = self
            .group_by
            .iter()
            .filter(|level| **level != GroupBy::None)
            .map(|level| level.label().to_lowercase())
            .collect::<Vec<_>>();
        if named.is_empty() {
            "nothing".to_owned()
        } else {
            named.join(", then ")
        }
    }

    /// Returns each task's position in the focus sequence it belongs to.
    ///
    /// Tasks outside every sequence sort after those inside one, so ordering by
    /// sequence puts the planned run first and the rest behind it.
    fn sequence_ranks(&self) -> HashMap<String, usize> {
        let mut ranks = HashMap::new();
        for sequence in &self.focus_sequences {
            for (position, member) in sequence.items.iter().enumerate() {
                ranks.entry(member.to_string()).or_insert(position);
            }
        }
        ranks
    }

    /// Returns the computed weight of each task, keyed by id.
    fn weight_index(&self, items: &[StoredItem]) -> HashMap<String, f32> {
        let children = child_index(items);
        items
            .iter()
            .map(|item| {
                (
                    item.item.id.to_string(),
                    item_weight(item, &children, &self.theme),
                )
            })
            .collect()
    }

    /// Returns the column a task belongs in under the current grouping.
    fn group_key(&self, item: &StoredItem, level: GroupBy) -> String {
        match level {
            GroupBy::None => String::new(),
            GroupBy::Hierarchy => "structural family".to_owned(),
            GroupBy::Tag => {
                let mut tags = self.effective_tags(item);
                tags.sort();
                tags.into_iter()
                    .max_by_key(|tag| tag.matches('/').count())
                    .unwrap_or_else(|| "(untagged)".to_owned())
            }
            GroupBy::Sequence => {
                let id = item.item.id.to_string();
                self.focus_sequences
                    .iter()
                    .find(|sequence| sequence.items.iter().any(|member| member.to_string() == id))
                    .map_or_else(|| "(no sequence)".to_owned(), |found| found.title.clone())
            }
            GroupBy::Time => {
                let today = Local::now().date_naive();
                let when = item.item.scheduled_on.or(item.item.due_on);
                match when {
                    None => "(undated)".to_owned(),
                    Some(date) if date < today => "overdue".to_owned(),
                    Some(date) if date == today => "today".to_owned(),
                    Some(date) if (date - today).num_days() <= 7 => "this week".to_owned(),
                    Some(_) => "later".to_owned(),
                }
            }
            GroupBy::Priority => item.item.priority.to_string(),
            GroupBy::State => item.item.state.to_string(),
        }
    }

    /// Returns finished tasks that something visible still depends on.
    fn finished_prerequisites(&self, items: &[StoredItem]) -> std::collections::HashSet<String> {
        let finished = items
            .iter()
            .filter(|item| {
                item.item.state == ItemState::Done || item.item.state == ItemState::Canceled
            })
            .map(|item| item.item.id.to_string())
            .collect::<std::collections::HashSet<_>>();
        items
            .iter()
            .flat_map(|item| item.dependencies.iter().map(|id| id.to_string()))
            .filter(|id| finished.contains(id))
            .collect()
    }

    /// Writes an arrangement of graph-space positions into the saved offsets.
    fn apply_arrangement(
        &mut self,
        rect: Rect,
        items: &[StoredItem],
        arranged: &HashMap<String, Vec2>,
        origin: Pos2,
    ) {
        self.remember_layout();
        for (index, item) in items.iter().enumerate() {
            let id = item.item.id.to_string();
            let Some(target) = arranged.get(&id) else {
                continue;
            };
            self.node_offsets
                .insert(id, (origin + *target) - grid_slot(index, rect));
        }
        self.save_layout();
    }

    /// Rearranges only the connected parent/child tree under the pointer.
    ///
    /// The root stays anchored and collapsed descendants are revealed before
    /// arranging; unrelated families retain their drawn positions.
    fn normalize_family(&mut self, rect: Rect, clicked: &str) {
        let before = self.visible_items();
        if !before
            .iter()
            .any(|item| item.item.id.to_string() == clicked)
        {
            return;
        }
        let eligible = self.filtered_items();
        let present = eligible
            .iter()
            .map(|item| item.item.id.to_string())
            .collect::<std::collections::HashSet<_>>();
        let parents = eligible
            .iter()
            .filter_map(|item| {
                let parent = item.item.parent_id?.to_string();
                present
                    .contains(&parent)
                    .then(|| (item.item.id.to_string(), parent))
            })
            .collect::<HashMap<_, _>>();
        let mut root = clicked.to_owned();
        let mut seen = std::collections::HashSet::new();
        while seen.insert(root.clone()) {
            let Some(parent) = parents.get(&root) else {
                break;
            };
            root = parent.clone();
        }
        let Some(root_index) = before
            .iter()
            .position(|item| item.item.id.to_string() == root)
        else {
            return;
        };
        let root_at = grid_slot(root_index, rect)
            + self.node_offsets.get(&root).copied().unwrap_or(Vec2::ZERO);
        self.remember_layout();
        self.expanded_families.insert(root.clone());
        let items = self.visible_items();
        self.rebase_visible_offsets(&before, &items, rect);
        let children = items
            .iter()
            .filter_map(|item| {
                let parent = item.item.parent_id?.to_string();
                present
                    .contains(&parent)
                    .then(|| (parent, item.item.id.to_string()))
            })
            .fold(
                std::collections::BTreeMap::<String, Vec<String>>::new(),
                |mut children, (parent, child)| {
                    children.entry(parent).or_default().push(child);
                    children
                },
            );
        let spacing = TreeSpacing {
            leaf_gap: self.theme.layout.leaf_gap,
            row_gap: self.theme.layout.row_gap,
            axis: self.theme.layout.tree_axis,
        };
        let arranged = geometry::tree_positions_at_levels(
            &[root.clone()],
            &children,
            &HashMap::new(),
            spacing,
        );
        let Some(root_target) = arranged.get(&root).copied() else {
            return;
        };
        for (index, item) in items.iter().enumerate() {
            let id = item.item.id.to_string();
            if let Some(at) = arranged.get(&id) {
                self.node_offsets
                    .insert(id, root_at + *at - root_target - grid_slot(index, rect));
            }
        }
        self.focus_mode = false;
        self.save_layout();
        self.message =
            Some("Connected task family arranged; other families stayed put.".to_owned());
    }

    /// Rearranges visible nodes so relationships flow one way, down or right.
    ///
    /// The previous arrangement is kept in `layout_undo`, so a normalize that
    /// scatters a hand-built layout can be taken back.
    fn normalize_layout(&mut self, rect: Rect) {
        if self.theme.layout.style == NormalizeStyle::Hierarchy {
            self.hierarchy_layout(rect);
            return;
        }
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
        self.remember_layout();
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
        self.frame_visible_nodes(rect);
        self.group_labels.clear();
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

    /// Centers the camera on the main visible cluster and fits most of its cards.
    ///
    /// The retained central majority prevents a deliberately remote card from
    /// shrinking the working cluster to an unreadable size. The omitted minority
    /// remains available by pan and is not filtered or moved.
    fn frame_visible_nodes(&mut self, rect: Rect) {
        let items = self.visible_items();
        let points = items
            .iter()
            .enumerate()
            .map(|(index, item)| {
                let id = item.item.id.to_string();
                grid_slot(index, rect) + self.node_offsets.get(&id).copied().unwrap_or(Vec2::ZERO)
            })
            .collect::<Vec<_>>();
        let (width, height) = self.theme.node.size();
        let Some(bounds) =
            geometry::majority_bounds(&points, Vec2::new(width / 2.0, height / 2.0), 0.85)
        else {
            return;
        };
        let (zoom, pan) = geometry::fit_view(bounds, rect, (0.2, 4.0), 72.0);
        self.graph_zoom = zoom;
        self.graph_pan = pan;
    }

    /// Runs the overlap pass when it is switched on, as a layout's last step.
    fn relax_if_enabled(&mut self, rect: Rect) {
        if self.relax_overlaps {
            self.relax_overlaps(rect, false);
        }
    }

    /// Captures one reversible graph action, including collapsed-family state.
    fn remember_layout(&mut self) {
        self.layout_undo = Some(LayoutUndo {
            offsets: self.node_offsets.clone(),
            expanded_families: self.expanded_families.clone(),
        });
    }

    /// Visibility changes reorder grid slots; keep already-visible cards at
    /// their drawn graph positions while the newly revealed cards join them.
    fn rebase_visible_offsets(&mut self, before: &[StoredItem], after: &[StoredItem], rect: Rect) {
        let prior = before
            .iter()
            .enumerate()
            .map(|(index, item)| {
                let id = item.item.id.to_string();
                let at = grid_slot(index, rect)
                    + self.node_offsets.get(&id).copied().unwrap_or(Vec2::ZERO);
                (id, at)
            })
            .collect::<HashMap<_, _>>();
        for (index, item) in after.iter().enumerate() {
            let id = item.item.id.to_string();
            if let Some(at) = prior.get(&id) {
                self.node_offsets.insert(id, *at - grid_slot(index, rect));
            }
        }
    }

    fn toggle_family_expanded(&mut self, id: &str, rect: Rect) {
        let before = self.visible_items();
        self.remember_layout();
        if !self.expanded_families.remove(id) {
            self.expanded_families.insert(id.to_owned());
        }
        let after = self.visible_items();
        self.rebase_visible_offsets(&before, &after, rect);
        self.save_layout();
    }

    /// Restores positions and expansion together so a local layout is undoable.
    fn undo_layout(&mut self) {
        match self.layout_undo.take() {
            Some(previous) => {
                self.node_offsets = previous.offsets;
                self.expanded_families = previous.expanded_families;
                self.timeline_origin = None;
                self.save_layout();
                self.message = Some("Previous layout restored.".to_owned());
            }
            None => self.message = Some("No layout change to undo.".to_owned()),
        }
    }

    /// Searches what a person can actually see or say: the title, either
    /// rendering of the identifier, and the effective (inherited) tags.
    fn search_matches(&self, item: &StoredItem) -> bool {
        let id = item.item.id;
        matches_query(
            &self.search,
            &item.item.title,
            &[id.to_string(), id.proquint()],
            &self.effective_tags(item),
        )
    }

    /// Selects the first match so Enter jumps rather than only highlighting.
    fn jump_to_first_match(&mut self) {
        if let Some(found) = self
            .visible_items()
            .into_iter()
            .find(|item| self.search_matches(item))
        {
            let id = found.item.id.to_string();
            self.selected = Some(id.clone());
            self.select(id);
        }
    }

    /// Returns the items the graph is currently drawing: the explicit filters,
    /// then the child-depth collapse that hides distant descendants.
    fn visible_items(&self) -> Vec<StoredItem> {
        limit_child_depth(
            self.filtered_items(),
            self.theme.parent.child_depth,
            &self.expanded_families,
            self.selected.as_deref(),
        )
    }

    /// Applies only the explicit filters; child-depth collapse is a view layer.
    fn filtered_items(&self) -> Vec<StoredItem> {
        self.items
            .iter()
            .filter(|item| {
                (self.show_completed
                    || (item.item.state != ItemState::Done
                        && item.item.state != ItemState::Canceled))
                    && (!self.focus_only || item.item.state == ItemState::Active)
                    && self.agent_filter.keeps(item)
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
            match self.write_theme() {
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
                self.theme_loaded = theme.clone();
                self.theme = theme;
                self.colors_applied = false;
                // The palette path may have changed with the theme, so the watched
                // set is taken from the theme just loaded.
                self.theme_seen = self.theme_stamps();
                if self.message.is_none() {
                    self.message = Some("Theme reloaded.".to_owned());
                }
            }
            Err(error) => self.message = Some(format!("Theme not loaded: {error:#}")),
        }
    }

    /// Stamps of the files the theme comes from: `theme.toml` and its palette.
    fn theme_stamps(&self) -> Vec<Option<FileStamp>> {
        std::iter::once(self.theme_path.clone())
            .chain(self.theme.colors.palette_path())
            .map(|path| file_stamp(&path))
            .collect()
    }

    /// Reloads the theme when `theme.toml` or its palette changed on disk, so a
    /// regenerated palette or a hand edit shows without pressing Reload theme.
    ///
    /// Unsaved panel edits win: rather than discard them, the change is reported
    /// and left for Reload theme. A deleted `theme.toml` is not rewritten behind
    /// the developer's back; Reload theme still writes the defaults on request.
    fn reload_theme_if_changed(&mut self) {
        let stamps = self.theme_stamps();
        if stamps == self.theme_seen {
            return;
        }
        // Recorded before reloading, so a file that fails to parse is reported
        // once rather than every second until it is fixed.
        self.theme_seen = stamps;
        if !self.theme_path.exists() {
            return;
        }
        if self.theme != self.theme_loaded {
            self.message = Some("theme.toml changed on disk; Reload theme to apply it.".to_owned());
            return;
        }
        self.message = None;
        self.reload_theme();
    }

    /// Applies the color mode and any borrowed terminal palette to egui.
    ///
    /// The palette restyles the surfaces only: panels, canvas, text, selection,
    /// and links. Card colors stay with the tag and status channels.
    fn apply_colors(&mut self, ctx: &egui::Context) {
        let colors = &self.theme.colors;
        // ponytail: in system mode the palette lands on whichever appearance is
        // current at load; an OS switch afterwards shows egui's stock visuals until
        // Reload theme. Apply to both themes if that ever matters.
        let (preference, theme) = match colors.mode {
            ColorMode::Dark => (egui::ThemePreference::Dark, egui::Theme::Dark),
            ColorMode::Light => (egui::ThemePreference::Light, egui::Theme::Light),
            ColorMode::System => (egui::ThemePreference::System, ctx.theme()),
        };
        ctx.set_theme(preference);
        let Some(path) = colors.palette_path() else {
            return;
        };
        let palette = match fs::read_to_string(&path) {
            Ok(text) => TerminalPalette::parse_ghostty(&text),
            Err(error) => {
                self.message = Some(format!("Palette not read from {}: {error}", path.display()));
                return;
            }
        };
        // The palette's background decides the widget fills, not the mode: a
        // light palette over dark visuals left dark text on dark dropdowns.
        let mut visuals = match palette.is_dark() {
            Some(true) => egui::Visuals::dark(),
            Some(false) => egui::Visuals::light(),
            None => theme.default_visuals(),
        };
        if let Some(background) = palette.background {
            visuals.panel_fill = background;
            visuals.window_fill = background;
            visuals.extreme_bg_color = background;
        }
        visuals.override_text_color = palette.foreground;
        if let Some(selection) = palette.selection {
            visuals.selection.bg_fill = selection;
        }
        if let Some(accent) = palette.accent {
            visuals.hyperlink_color = accent;
        }
        ctx.set_visuals_of(theme, visuals);
    }

    /// Writes the current theme to disk, as a starting point or to keep changes.
    ///
    /// Settings adjusted in the side panel edit the live theme; this is what makes
    /// them survive a restart.
    ///
    /// The editor's own write is recorded as seen, so the theme watch does not
    /// reload what was just saved.
    fn write_theme(&mut self) -> Result<()> {
        if let Some(parent) = self.theme_path.parent() {
            fs::create_dir_all(parent).context("create theme directory")?;
        }
        let contents = self.theme.to_toml()?;
        fs::write(&self.theme_path, contents)
            .with_context(|| format!("write {}", self.theme_path.display()))?;
        self.theme_loaded = self.theme.clone();
        self.theme_seen = self.theme_stamps();
        Ok(())
    }

    /// Opens the ledger for one operation and closes it again; see `LedgerAccess`.
    fn with_ledger<T>(&self, operation: impl AsyncFnOnce(&Ledger) -> Result<T>) -> Result<T> {
        self.runtime.block_on(async {
            let ledger =
                Ledger::open_with_actor(&self.ledger.path, self.ledger.actor_id.clone()).await?;
            operation(&ledger).await
        })
    }

    /// Refreshes when the ledger files changed since the last look, which is
    /// how a CLI write reaches an open editor.
    ///
    /// The stamp is taken after refreshing, so any write the refresh itself
    /// causes while opening is not mistaken for an outside change.
    fn refresh_if_ledger_changed(&mut self) {
        if self.ledger.modified() != self.ledger_seen {
            self.refresh();
            self.ledger_seen = self.ledger.modified();
        }
    }

    fn refresh(&mut self) {
        self.history = None;
        match self.with_ledger(async |ledger| ledger.list_items(ItemScope::All, true).await) {
            Ok(items) => {
                self.items = items;
                match self.with_ledger(async |ledger| ledger.namespaces().await) {
                    Ok(namespaces) => self.namespaces = namespaces,
                    Err(error) => self.message = Some(format!("Could not load tags: {error:#}")),
                }
                match self.with_ledger(async |ledger| ledger.focus_sequences().await) {
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
            .with_ledger(async |ledger| ledger.create_focus_sequence(&self.focus_title_draft).await)
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
            .with_ledger(async |ledger| ledger.add_focus_sequence_item(&sequence, &item).await)
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
            family_presentations: self.family_presentations.clone(),
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
            if self.selected.as_deref() != Some(id.as_str()) {
                self.editing = None;
            }
            self.selected = Some(id);
            match self.with_ledger(async |ledger| ledger.notes(item.item.id).await) {
                Ok(notes) => self.notes = notes,
                Err(error) => self.message = Some(format!("Could not load notes: {error:#}")),
            }
            match self.with_ledger(async |ledger| ledger.citations(item.item.id).await) {
                Ok(citations) => self.citations = citations,
                Err(error) => self.message = Some(format!("Could not load links: {error:#}")),
            }
            match self.with_ledger(async |ledger| ledger.roles(item.item.id).await) {
                Ok(roles) => self.roles = roles,
                Err(error) => self.message = Some(format!("Could not load people: {error:#}")),
            }
            match self.with_ledger(async |ledger| ledger.sessions(item.item.id).await) {
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
        match self.with_ledger(async |ledger| ledger.edit_title(&id, title).await) {
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
        match self.with_ledger(async |ledger| ledger.set_priority(&id, self.priority_draft).await) {
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
        match self.with_ledger(async |ledger| ledger.set_state(&id, next).await) {
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
        match self.with_ledger(async |ledger| ledger.set_state(&id, self.state_draft).await) {
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
        match self.with_ledger(async |ledger| ledger.add_item(title, None).await) {
            Ok(item) => {
                let id = item.item.id.to_string();
                self.new_task_title.clear();
                self.new_task_open = false;
                // The parent is set as a second step because creation is unscoped;
                // a failure to attach leaves the task itself intact and says so.
                let attached = match parent {
                    Some(parent) => {
                        match self.with_ledger(async |ledger| ledger.set_parent(&id, &parent).await)
                        {
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
        match self.with_ledger(async |ledger| ledger.add_note(&id, body).await) {
            Ok(()) => {
                self.note_draft.clear();
                self.message = Some("Note added.".to_owned());
                if let Some(item) = self.selected_item() {
                    match self.with_ledger(async |ledger| ledger.notes(item.item.id).await) {
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
        match self.with_ledger(async |ledger| ledger.add_citation(&id, target, label).await) {
            Ok(()) => {
                self.link_target_draft.clear();
                self.link_label_draft.clear();
                self.message = Some("Link added.".to_owned());
                if let Some(item) = self.selected_item()
                    && let Ok(citations) =
                        self.with_ledger(async |ledger| ledger.citations(item.item.id).await)
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
            self.with_ledger(async |ledger| ledger.finish_session(&id, chrono::Utc::now()).await)
        } else {
            self.with_ledger(async |ledger| ledger.start_session(&id, chrono::Utc::now()).await)
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
            Ok(tag) => {
                match self.with_ledger(async |ledger| ledger.assign_namespace(&id, &tag).await) {
                    Ok(()) => {
                        self.tag_draft.clear();
                        self.message = Some("Tag added.".to_owned());
                        self.refresh();
                    }
                    Err(error) => self.message = Some(format!("Tag not added: {error:#}")),
                }
            }
            Err(error) => self.message = Some(format!("Invalid tag: {error}")),
        }
    }

    fn add_role(&mut self) {
        let Some(id) = self.selected.clone() else {
            return;
        };
        let role = self.role_draft.clone();
        let path = match format!("actor/{}", self.actor_draft.trim()).parse::<NamespacePath>() {
            Ok(path) => path,
            Err(error) => {
                self.message = Some(format!("Invalid actor: {error}"));
                return;
            }
        };
        match self.with_ledger(async |ledger| ledger.assign_role(&id, &role, &path).await) {
            Ok(()) => {
                self.actor_draft.clear();
                self.message = Some(format!("{role}: {} added.", actor_name(&path)));
                self.refresh();
                self.select(id);
            }
            Err(error) => self.message = Some(format!("Person not added: {error:#}")),
        }
    }

    fn remove_role(&mut self, role: &str, actor: &NamespacePath) {
        let Some(id) = self.selected.clone() else {
            return;
        };
        match self.with_ledger(async |ledger| ledger.remove_role(&id, role, actor).await) {
            Ok(()) => {
                self.message = Some(format!("{role}: {} removed.", actor_name(actor)));
                self.refresh();
                self.select(id);
            }
            Err(error) => self.message = Some(format!("Person not removed: {error:#}")),
        }
    }

    fn remove_tag(&mut self, tag: &NamespacePath) {
        let Some(id) = self.selected.clone() else {
            return;
        };
        match self.with_ledger(async |ledger| ledger.remove_namespace(&id, tag).await) {
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
            Some(parent) => self.with_ledger(async |ledger| ledger.set_parent(&id, &parent).await),
            None => self.with_ledger(async |ledger| ledger.clear_parent(&id).await),
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
            self.with_ledger(async |ledger| ledger.add_blocker(&id, &blocker).await)
        } else {
            self.with_ledger(async |ledger| ledger.remove_blocker(&id, &blocker).await)
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
            self.with_ledger(async |ledger| ledger.add_dependency(&id, &dependency).await)
        } else {
            self.with_ledger(async |ledger| ledger.remove_dependency(&id, &dependency).await)
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
            self.with_ledger(async |ledger| ledger.set_due_on(&id, date).await)
        } else {
            self.with_ledger(async |ledger| ledger.set_defer_until(&id, date).await)
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
        match self.with_ledger(async |ledger| {
            ledger
                .apply_patch(
                    &id,
                    ItemPatch {
                        scheduled_on: Some(date),
                        ..ItemPatch::default()
                    },
                )
                .await
        }) {
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
        match patch
            .and_then(|patch| self.with_ledger(async |ledger| ledger.apply_patch(&id, patch).await))
        {
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

/// Case-insensitive substring match across a task's searchable text.
///
/// An empty query matches everything, so "search off" and "search cleared" are
/// the same state and need no separate flag.
fn matches_query(query: &str, title: &str, ids: &[String], tags: &[String]) -> bool {
    let needle = query.trim().to_lowercase();
    if needle.is_empty() {
        return true;
    }
    title.to_lowercase().contains(&needle)
        || ids.iter().any(|id| id.to_lowercase().contains(&needle))
        || tags.iter().any(|tag| tag.to_lowercase().contains(&needle))
}

impl eframe::App for TodoGui {
    fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
        if !self.colors_applied {
            self.colors_applied = true;
            self.apply_colors(ui.ctx());
        }
        self.refresh_if_ledger_changed();
        self.reload_theme_if_changed();
        // ponytail: polls file times once a second; a file watcher is the upgrade
        // if idle repaints ever cost too much.
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_secs(1));
        // `/` reaches the search box from anywhere on the canvas; Escape leaves it.
        // Both are gated on the canvas having focus so typing a title is unaffected.
        if !ui.ctx().egui_wants_keyboard_input()
            && ui.input(|input| input.key_pressed(egui::Key::Slash))
        {
            self.search_focus = true;
        }
        if ui.input(|input| input.key_pressed(egui::Key::Escape)) && !self.search.is_empty() {
            self.search.clear();
        }
        // F, comma, and I open the filter drawer, Settings, and the inspector.
        if !ui.ctx().egui_wants_keyboard_input() {
            ui.input(|input| {
                self.show_filters ^= input.key_pressed(egui::Key::F);
                self.show_settings ^= input.key_pressed(egui::Key::Comma);
                self.show_inspector ^= input.key_pressed(egui::Key::I);
            });
        }
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
        // The toolbar carries actions and the three surface toggles. The ledger
        // and theme reload on their own, so neither needs a button here.
        let chips = self.active_filters();
        egui::Panel::top("toolbar").show(ui, |ui| {
            ui.horizontal(|ui| {
                if ui.button("+ New").clicked() {
                    self.new_task_parent = None;
                    self.new_task_title.clear();
                    self.new_task_open = true;
                }
                let filter_label = if chips.is_empty() {
                    "Filter".to_owned()
                } else {
                    format!("Filter · {}", chips.len())
                };
                ui.toggle_value(&mut self.show_filters, filter_label)
                    .on_hover_text("Filter drawer (F)");
                ui.menu_button("Layout ▾", |ui| self.arrangement_buttons(ui));
                ui.separator();
                if ui.button("−").clicked() {
                    self.graph_zoom = (self.graph_zoom / 1.2).max(0.2);
                }
                ui.weak(format!("{:.0}%", self.graph_zoom * 100.0));
                if ui.button("+").clicked() {
                    self.graph_zoom = (self.graph_zoom * 1.2).min(4.0);
                }
                if ui.button("⌖").on_hover_text("Reset view").clicked() {
                    // The graph viewport is only known after both side panels
                    // have claimed their space, so framing is deferred to canvas draw.
                    self.reset_view_requested = true;
                }
                ui.separator();
                let search = ui.add(
                    egui::TextEdit::singleline(&mut self.search)
                        .desired_width(180.0)
                        .hint_text("/ search"),
                );
                if self.search_focus {
                    search.request_focus();
                    self.search_focus = false;
                }
                if search.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter)) {
                    self.jump_to_first_match();
                }
                if !self.search.is_empty() {
                    let hits = self
                        .visible_items()
                        .iter()
                        .filter(|item| self.search_matches(item))
                        .count();
                    ui.weak(format!("{hits} match"));
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.toggle_value(&mut self.show_inspector, "▣")
                        .on_hover_text("Inspector (I)");
                    ui.toggle_value(&mut self.show_settings, "⚙")
                        .on_hover_text("Settings (,)");
                });
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
        if self.show_settings {
            self.draw_settings(ui.ctx());
        }
        if self.show_filters {
            egui::Panel::left("filters")
                .default_size(230.0)
                .resizable(true)
                .show(ui, |ui| self.draw_filters(ui));
        }
        if self.show_inspector {
            egui::Panel::right("inspector")
                .default_size(280.0)
                .min_size(220.0)
                .max_size(480.0)
                .resizable(true)
                .show(ui, |ui| self.draw_inspector(ui));
        }
        // Declared after the side panels so it spans the canvas only.
        let chips = self.active_filters();
        if !chips.is_empty() {
            egui::Panel::top("filter-bar").show(ui, |ui| self.draw_filter_bar(ui, &chips));
        }
        egui::CentralPanel::default().show(ui, |ui| draw_graph(ui, self));
    }
}

fn draw_graph(ui: &mut egui::Ui, app: &mut TodoGui) {
    let (rect, response) = ui.allocate_exact_size(ui.available_size(), Sense::drag());
    let painter = ui.painter_at(rect);
    // Dragging empty canvas moves the view. Cards are interacted with after this,
    // so a drag that starts on a card moves the card rather than the canvas.
    if response.dragged() {
        app.graph_pan += response.drag_delta();
    }
    if ui.input(|input| input.pointer.middle_down()) {
        app.graph_pan += ui.input(|input| input.pointer.delta());
    }
    // Scrolling zooms about the pointer rather than the viewport center, so the
    // card under the cursor stays under it. Gated on hover, or scrolling either
    // sidebar would move the graph.
    if response.hovered()
        && let Some(pointer) = ui.input(|input| input.pointer.hover_pos())
    {
        // `smooth_scroll_delta` intentionally continues after the device stops.
        // Graph zoom should follow only physical wheel events, otherwise it
        // feels like inertial zoom after the wheel or trackpad is released.
        // `zoom_delta` is a multiplicative factor, so its neutral value is 1;
        // use ln (not ln_1p) to keep an idle frame at zero zoom movement.
        let scroll = ui.input(|input| {
            let wheel = input
                .raw
                .events
                .iter()
                .filter_map(|event| match event {
                    egui::Event::MouseWheel { delta, .. } => Some(delta.y),
                    _ => None,
                })
                .sum::<f32>();
            let pinch = input
                .raw
                .events
                .iter()
                .filter_map(|event| match event {
                    egui::Event::Zoom(scale) => Some(scale.ln()),
                    _ => None,
                })
                .sum::<f32>();
            wheel + pinch
        });
        if scroll.abs() > f32::EPSILON {
            let previous = app.graph_zoom;
            app.graph_zoom = (previous * (scroll * 0.004).exp()).clamp(0.2, 4.0);
            let ratio = app.graph_zoom / previous;
            app.graph_pan = geometry::zoom_about(app.graph_pan, rect.center(), pointer, ratio);
        }
    }
    if app.normalize_requested {
        app.normalize_requested = false;
        app.normalize_layout(rect);
    }
    if app.reset_view_requested {
        app.reset_view_requested = false;
        app.frame_visible_nodes(rect);
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
    if app.group_requested {
        app.group_requested = false;
        app.group_layout(rect);
    }
    if app.timeline_requested {
        app.timeline_requested = false;
        app.timeline_layout(rect);
    }
    let items = app.visible_items();
    // The wheel divides hues between the tags actually on screen, so the palette is
    // rebuilt whenever the visible set changes.
    let effective = app.effective_tag_index(&items);
    let holding_active = if app.theme.focus.parents_of_active_read_active {
        ids_holding_active_work(&items)
    } else {
        std::collections::HashSet::new()
    };
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
    // A family's connectors use the same fill its frame is painted with, so an
    // open family keeps the color its box had. Resolved once: every link into a
    // parent asks for the same value.
    let family_fill = items
        .iter()
        .map(|item| {
            let fill = node_tag_color(item, &app.theme, &palette, &effective)
                .map_or(Color32::from_gray(150), |accent| family_colors(accent).0);
            (item.item.id.to_string(), fill)
        })
        .collect::<HashMap<_, _>>();
    let fill_of = |id: &str| {
        family_fill
            .get(id)
            .copied()
            .unwrap_or(Color32::from_gray(150))
    };
    let connector_style = app.theme.parent.connector_style;
    let show_hierarchy = app.show_parent_graph;
    let show_dependencies = app.show_dependency_graph;
    let show_blockers = app.show_blocker_graph;
    let show_relations = app.show_relations;
    // The head needs clear air in front of the card it points at.
    let edge_gap = app.theme.edge.arrowhead_length * 0.5 + 2.0;
    if let Some(origin) = app.timeline_origin
        && app.show_timeline_ruler
        && app.theme.timeline.show_ruler
        && app.theme.timeline.axis == geometry::TimelineAxis::LeftRight
    {
        draw_timeline_ruler(&painter, rect, origin, app);
    }
    for (name, at) in &app.group_labels {
        let center = rect.center();
        let drawn = center + (*at - center) * app.graph_zoom + app.graph_pan;
        painter.text(
            drawn,
            Align2::LEFT_BOTTOM,
            name,
            FontId::proportional(13.0),
            Color32::from_gray(170),
        );
    }
    let child_index = child_index(&items);
    // Every overlay must use the center of the card that is actually drawn. A
    // parent container used to move its card into a header after clouds had
    // already used the old graph position, leaving a visible orphaned circle.
    let layouts = node_layouts(&painter, app, &items, &positions, &child_index);
    let boxes = if !app.focus_mode
        && app.show_parent_regions
        && app.theme.parent.show_regions
        && app.theme.parent.region_style == ParentRegionStyle::Box
    {
        family_boxes(
            &items,
            &layouts,
            &app.theme,
            app.graph_zoom,
            &app.family_presentations,
        )
    } else {
        Vec::new()
    };
    let rendered_positions = layouts
        .iter()
        .map(|(id, layout)| (id.clone(), layout.rect.center()))
        .collect::<HashMap<_, _>>();
    if app.show_tag_constellation {
        draw_tag_constellation(
            &painter,
            &items,
            &layouts,
            app.graph_zoom,
            &app.theme,
            &palette,
            &effective,
        );
    }
    // Tag clouds are ambient context. Family containment is the structural
    // reading of the graph, so it is painted above the clouds before edges and
    // cards make the parent-to-child path explicit.
    if app.show_parent_regions && app.theme.parent.show_regions {
        if app.theme.parent.region_style == ParentRegionStyle::Hull {
            draw_parent_regions(
                &painter,
                &items,
                &positions,
                app.graph_zoom,
                &app.theme,
                &palette,
                &effective,
            );
        } else if !boxes.is_empty() {
            draw_family_boxes(&painter, app, &items, &boxes, &palette, &effective);
        }
    }
    if app.show_focus_window && app.theme.focus.show_frame {
        let today = Local::now().date_naive();
        let active = items
            .iter()
            .filter(|item| focus_is_active(item, &holding_active, &app.theme.focus, today))
            .filter_map(|item| rendered_positions.get(&item.item.id.to_string()).copied())
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
    // Cards are sized before any edge is drawn, so an arrow can be aimed at the
    // rectangle a card actually occupies rather than at an assumed size.
    let rect_of = |id: &str| layouts.get(id).map(|layout| layout.rect);
    let family_wells = boxes.iter().map(|family| family.well).collect::<Vec<_>>();
    let mut clicked = None;
    if show_hierarchy {
        let mut family_children = HashMap::<String, Vec<Rect>>::new();
        for item in &items {
            let Some(parent) = item.item.parent_id else {
                continue;
            };
            let Some(child) = rect_of(&item.item.id.to_string()) else {
                continue;
            };
            family_children
                .entry(parent.to_string())
                .or_default()
                .push(child);
        }
        for (parent_id, children) in family_children {
            let Some(parent) = rect_of(&parent_id) else {
                continue;
            };
            let depth = items
                .iter()
                .find(|item| item.item.id.to_string() == parent_id)
                .map_or(0, |item| ancestor_depth(item, &items));
            draw_family_tree(
                &painter,
                parent,
                boxes
                    .iter()
                    .find(|family| family.parent_id == parent_id && family.direct_join)
                    .map(|family| geometry::boundary_point(family.well, parent.center())),
                &children,
                depth,
                app.graph_zoom,
                app.theme.layout.tree_axis,
                fill_of(&parent_id),
                connector_style,
                &family_wells,
            );
        }
    }
    for item in &items {
        let Some(to_rect) = rect_of(&item.item.id.to_string()) else {
            continue;
        };
        if show_dependencies {
            for dependency in &item.dependencies {
                if let Some(from) = rect_of(&dependency.to_string()) {
                    let (start, end) = geometry::edge_between(from, to_rect, edge_gap);
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
                if let Some(to_target) = rect_of(&relation.target.to_string()) {
                    // A relation points from the item to what it contributes to or
                    // implements, and is drawn apart from structure and dependencies.
                    let (start, end) = geometry::edge_between(to_rect, to_target, edge_gap);
                    draw_relation_edge(
                        &painter,
                        start,
                        end,
                        relation_color(relation.relation),
                        &app.theme,
                        app.graph_zoom,
                    );
                }
            }
        }
        if show_blockers {
            for blocker in &item.blocked_by {
                if let Some(from) = rect_of(&blocker.to_string()) {
                    let (start, end) = geometry::edge_between(from, to_rect, edge_gap);
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
    let hidden_descendants = if app.theme.parent.child_depth == 0 {
        HashMap::new()
    } else {
        let filtered = app.filtered_items();
        let all_children = crate::child_index(&filtered);
        let visible_ids = items
            .iter()
            .map(|item| item.item.id.to_string())
            .collect::<std::collections::HashSet<_>>();
        all_children
            .keys()
            .map(|parent| {
                let hidden = collect_subtree(parent, &all_children)
                    .into_iter()
                    .filter(|id| id != parent && !visible_ids.contains(id))
                    .count();
                (parent.clone(), hidden)
            })
            .collect::<HashMap<_, _>>()
    };
    for item in &items {
        let id = item.item.id.to_string();
        let Some(layout) = layouts.get(&id).cloned() else {
            continue;
        };
        let NodeLayout {
            rect: node,
            title_galley,
            meta_font,
            padding,
            ..
        } = layout;
        let response = ui.interact(node, ui.make_persistent_id(&id), Sense::click_and_drag());
        response.context_menu(|ui| {
            if ui.button("Normalize connected family (G)").clicked() {
                app.normalize_family(rect, &id);
                ui.close();
            }
            ui.menu_button("Family display", |ui| {
                let selected = app.family_presentations.get(&id).copied();
                if ui
                    .selectable_label(selected.is_none(), "Inherit graph default")
                    .clicked()
                {
                    app.family_presentations.remove(&id);
                    app.save_layout();
                    ui.close();
                }
                for presentation in [
                    FamilyPresentation::Dynamic,
                    FamilyPresentation::Box,
                    FamilyPresentation::Tree,
                ] {
                    if ui
                        .selectable_label(selected == Some(presentation), presentation.label())
                        .clicked()
                    {
                        app.family_presentations.insert(id.clone(), presentation);
                        app.save_layout();
                        ui.close();
                    }
                }
            });
        });
        let selected = app.selected.as_deref() == Some(id.as_str());
        let holds_active = holding_active.contains(&id);
        let accent = node_tag_color(item, &app.theme, &palette, &effective);
        let status = status_color(item, holds_active);
        let weight = item_weight(item, &child_index, &app.theme);
        let fill = channel_color(
            app.theme.node.body_encodes,
            status,
            accent,
            item,
            weight,
            &app.theme,
            true,
        )
        .unwrap_or(status);
        // A parent is the visible tab of its own family body. Sharing one opaque
        // fill joins the two pieces without a translucent overlap seam.
        let owns_family_box = boxes.iter().any(|family| family.parent_id == id);
        let fill = if owns_family_box {
            family_colors(accent.unwrap_or(Color32::from_gray(150))).0
        } else {
            fill
        };
        // While focus mode is on, work that is not active stays drawn but quiet, so
        // it reads as context rather than as part of what is being worked on.
        // A search result reuses the focus-mode treatment: non-matches stay drawn
        // but quiet, so edges and structure survive the query instead of vanishing.
        let exiled = (app.focus_mode
            && !focus_is_active(
                item,
                &holding_active,
                &app.theme.focus,
                Local::now().date_naive(),
            ))
            || !app.search_matches(item);
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
        let bar = channel_color(
            app.theme.node.bar_encodes,
            status,
            accent,
            item,
            weight,
            &app.theme,
            false,
        )
        .map(dim);

        // Selection overrides the border, since which task is selected must never
        // be ambiguous whatever the border is otherwise saying.
        let border = channel_color(
            app.theme.node.outline_encodes,
            status,
            accent,
            item,
            weight,
            &app.theme,
            false,
        )
        .map(dim)
        .unwrap_or(dim(Color32::LIGHT_GRAY));
        // A child that shares its enclosing family's fill does not need to draw
        // the same tag border twice. Selection keeps its gold outline, and a
        // differently colored child keeps the normal outline for contrast.
        let shares_family_fill = accent.is_some_and(|accent| {
            let own_family_fill = family_colors(accent).0;
            boxes.iter().any(|family| {
                family.parent_id != id
                    && family.well.contains(node.center())
                    && fill_of(&family.parent_id) == own_family_fill
            })
        });
        let border = if !selected && shares_family_fill {
            if app.theme.node.hide_outline_in_family {
                Color32::TRANSPARENT
            } else if app.theme.node.soften_outline_in_family {
                blend_color(border, fill, 18)
            } else {
                border
            }
        } else {
            border
        };
        let outline = Stroke::new(
            if selected {
                3.0
            } else {
                app.theme.node.outline_width
            },
            if selected { Color32::GOLD } else { border },
        );
        // An estimate is drawn as a block along the time axis, so a long task reads
        // as occupying time rather than as another card of the same size.
        if app.timeline_origin.is_some() && app.theme.timeline.show_durations {
            let length = geometry::duration_length(
                item.item.estimate_minutes,
                app.theme.timeline.day_width * app.graph_zoom,
                app.theme.timeline.hours_per_day,
            );
            if length > 1.0 {
                let block = match app.theme.timeline.axis {
                    geometry::TimelineAxis::LeftRight => Rect::from_min_max(
                        node.left_bottom() + Vec2::new(0.0, 3.0),
                        node.left_bottom() + Vec2::new(length, 9.0),
                    ),
                    geometry::TimelineAxis::TopDown => Rect::from_min_max(
                        node.right_top() + Vec2::new(3.0, 0.0),
                        node.right_top() + Vec2::new(9.0, length),
                    ),
                };
                painter.rect_filled(
                    block,
                    2.0,
                    Color32::from_rgba_unmultiplied(
                        accent.unwrap_or(Color32::GRAY).r(),
                        accent.unwrap_or(Color32::GRAY).g(),
                        accent.unwrap_or(Color32::GRAY).b(),
                        150,
                    ),
                );
            }
        }
        draw_node_body(
            &painter,
            node,
            fill,
            outline,
            bar,
            &app.theme,
            app.graph_zoom,
        );
        if owns_family_box
            && app.theme.parent.header_style == FamilyHeaderStyle::AttachedTab
            && app.theme.node.shape == NodeShape::RoundedRect
        {
            // The parent card is the tab itself. Remove its lower seam where
            // the opaque family body rises behind it.
            painter.line_segment(
                [
                    node.left_bottom() + Vec2::new(app.theme.node.rounding() * app.graph_zoom, 0.0),
                    node.right_bottom()
                        - Vec2::new(app.theme.node.rounding() * app.graph_zoom, 0.0),
                ],
                Stroke::new(outline.width + 1.0, fill),
            );
        }
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
                node_meta(item, holds_active),
                meta_font,
                dim(Color32::LIGHT_GRAY),
            );
        } else {
            let anchor = node.center() - title_galley.size() / 2.0;
            painter.galley(anchor, title_galley, dim(Color32::WHITE));
        }
        if let Some(hidden) = hidden_descendants.get(&id)
            && *hidden > 0
        {
            // This is deliberately at the card's child-side edge: an ellipsis
            // reads as a continuation of the family tree, not a floating mode
            // control in the card's corner.
            let badge = Rect::from_center_size(
                node.center_bottom() + Vec2::new(0.0, 5.0),
                Vec2::new(24.0, 12.0),
            );
            let tooltip = format!("Show {hidden} hidden task(s)");
            let toggle = ui
                .interact(
                    badge,
                    ui.make_persistent_id(("family-expand", &id)),
                    Sense::click(),
                )
                .on_hover_text(tooltip);
            painter.rect_filled(badge, 6.0, Color32::from_rgb(61, 68, 89));
            painter.rect_stroke(
                badge,
                6.0,
                Stroke::new(1.0, Color32::LIGHT_GRAY),
                egui::StrokeKind::Outside,
            );
            painter.text(
                badge.center(),
                Align2::CENTER_CENTER,
                "…",
                FontId::proportional(14.0),
                Color32::WHITE,
            );
            if toggle.clicked() {
                app.toggle_family_expanded(&id, rect);
                ui.ctx().request_repaint();
            }
        }
        if response.drag_started()
            && let Some(pointer) = response.interact_pointer_pos()
        {
            // A container carries its subtree, so every member's starting offset is
            // captured here and each frame places them relative to it.
            let moving = if owns_family_box {
                collect_subtree(&id, &child_index)
            } else {
                vec![id.clone()]
            };
            let offsets_start = moving
                .into_iter()
                .map(|member| {
                    let at = app.node_offsets.get(&member).copied().unwrap_or(Vec2::ZERO);
                    (member, at)
                })
                .collect();
            app.drag_anchor = Some(DragAnchor {
                id: id.clone(),
                pointer_start: pointer,
                offsets_start,
            });
        }
        if let Some(anchor) = &app.drag_anchor
            && anchor.id == id
            && let Some(pointer) = response.interact_pointer_pos()
        {
            let travel = (pointer - anchor.pointer_start) / app.graph_zoom.max(f32::EPSILON);
            let moved = anchor
                .offsets_start
                .iter()
                .map(|(member, start)| (member.clone(), *start + travel))
                .collect::<Vec<_>>();
            for (member, at) in moved {
                app.node_offsets.insert(member.clone(), at);
                // A dragged node follows the pointer exactly; easing it would feel
                // like lag rather than like motion.
                app.drawn_offsets.insert(member, at);
            }
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
    if !ui.ctx().egui_wants_keyboard_input()
        && ui.input(|input| input.key_pressed(egui::Key::G))
        && let Some(pointer) = ui.input(|input| input.pointer.hover_pos())
        && rect.contains(pointer)
    {
        if let Some((id, _)) = layouts
            .iter()
            .find(|(_, layout)| layout.rect.contains(pointer))
        {
            app.normalize_family(rect, id);
        } else {
            app.normalize_layout(rect);
        }
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

/// The shared blue every family connector carries, so a link reads as a link
/// before it reads as a particular family's link.
const FAMILY_BLUE: Color32 = Color32::from_rgb(147, 174, 218);

/// Returns the casing and core strokes a family connector is drawn with.
///
/// The casing is laid down first and the core over it, so the core is what the
/// eye lands on. A family fill can be dark enough to disappear against the
/// canvas on its own, which is why it is never drawn without the other stroke
/// behind it.
fn connector_strokes(
    fill: Color32,
    style: ConnectorStyle,
    width: f32,
    zoom: f32,
) -> (Stroke, Stroke) {
    let casing = width + 2.0 * zoom;
    match style {
        ConnectorStyle::FillCore => (Stroke::new(casing, FAMILY_BLUE), Stroke::new(width, fill)),
        ConnectorStyle::BlueCore => (Stroke::new(casing, fill), Stroke::new(width, FAMILY_BLUE)),
    }
}

/// Draws one shared family trunk, junction, and branches for a parent's direct
/// children. Dependency arrows deliberately use a separate drawing path, so a
/// true dependency can coexist without a duplicate structural diagonal.
#[expect(
    clippy::too_many_arguments,
    reason = "one drawing call taking its geometry, its depth, and its two colors"
)]
fn draw_family_tree(
    painter: &egui::Painter,
    parent: Rect,
    source: Option<Pos2>,
    children: &[Rect],
    depth: usize,
    zoom: f32,
    axis: geometry::TreeAxis,
    fill: Color32,
    style: ConnectorStyle,
    wells: &[Rect],
) {
    if children.is_empty() {
        return;
    }
    let generation = f32::from(u8::try_from(depth.saturating_sub(1)).unwrap_or(u8::MAX));
    let width = (6.0 - generation * 1.5).max(1.5) * zoom;
    let radius = 4.0 * zoom;
    let (casing, core) = connector_strokes(fill, style, width, zoom);
    let color = core.color;
    let (start, junction, ends, branch_axis) =
        family_tree_geometry(parent, source, children, radius, axis);
    draw_family_path(painter, &[start, junction], casing, core, wells);
    painter.circle_stroke(junction, radius, Stroke::new(1.6 * zoom, color));
    for end in ends {
        let path = match branch_axis {
            geometry::TreeAxis::Down => vec![junction, egui::pos2(end.x, junction.y), end],
            geometry::TreeAxis::Right => vec![junction, egui::pos2(junction.x, end.y), end],
        };
        draw_family_path(painter, &path, casing, core, wells);
        painter.circle_stroke(end, radius, Stroke::new(1.6 * zoom, color));
    }
}

/// Returns angular tree geometry for any visible parent/child arrangement.
///
/// Normalized hierarchy layouts use the configured axis and meet child cards
/// on their top/left edge. A manually moved child can end up on the opposite
/// side of its parent; that remains an angular family route rather than
/// silently degrading into the direct diagonal used by dependency arrows.
fn family_tree_geometry(
    parent: Rect,
    source: Option<Pos2>,
    children: &[Rect],
    radius: f32,
    axis: geometry::TreeAxis,
) -> (Pos2, Pos2, Vec<Pos2>, geometry::TreeAxis) {
    let gap = radius + 2.0;
    match axis {
        geometry::TreeAxis::Down => {
            let start = source.unwrap_or_else(|| parent.center_bottom());
            let forward = children.iter().all(|child| child.top() > start.y);
            if forward {
                let ends = children
                    .iter()
                    .map(|child| child.center_top() - Vec2::new(0.0, gap))
                    .collect::<Vec<_>>();
                let first_end = ends
                    .iter()
                    .map(|end| end.y)
                    .min_by(f32::total_cmp)
                    .unwrap_or(start.y + gap);
                let junction = egui::pos2(start.x, (start.y + first_end) * 0.5);
                return (start, junction, ends, axis);
            }
            family_tree_fallback_geometry(parent, source, children, axis)
        }
        geometry::TreeAxis::Right => {
            let start = source.unwrap_or_else(|| parent.right_center());
            let forward = children.iter().all(|child| child.left() > start.x);
            if forward {
                let ends = children
                    .iter()
                    .map(|child| child.left_center() - Vec2::new(gap, 0.0))
                    .collect::<Vec<_>>();
                let first_end = ends
                    .iter()
                    .map(|end| end.x)
                    .min_by(f32::total_cmp)
                    .unwrap_or(start.x + gap);
                let junction = egui::pos2((start.x + first_end) * 0.5, start.y);
                return (start, junction, ends, axis);
            }
            family_tree_fallback_geometry(parent, source, children, axis)
        }
    }
}

/// Routes a manually displaced family through a shared axis-aligned junction.
/// This preserves the family vocabulary even when a user has moved cards into
/// a shape that is not a conventional downward/rightward hierarchy.
fn family_tree_fallback_geometry(
    parent: Rect,
    source: Option<Pos2>,
    children: &[Rect],
    preferred_axis: geometry::TreeAxis,
) -> (Pos2, Pos2, Vec<Pos2>, geometry::TreeAxis) {
    let centers = children.iter().map(Rect::center).collect::<Vec<_>>();
    let count = f32::from(u16::try_from(centers.len().max(1)).unwrap_or(u16::MAX));
    let average = centers
        .iter()
        .fold(Vec2::ZERO, |sum, center| sum + center.to_vec2())
        / count;
    let center = Pos2::new(average.x, average.y);
    let start = source.unwrap_or_else(|| geometry::boundary_point(parent, center));
    let ends = children
        .iter()
        .map(|child| geometry::boundary_point(*child, parent.center()))
        .collect::<Vec<_>>();
    let end_average = ends.iter().fold(Vec2::ZERO, |sum, end| sum + end.to_vec2()) / count;
    let branch_axis = if (end_average.x - start.x).abs() > (end_average.y - start.y).abs() {
        geometry::TreeAxis::Right
    } else {
        preferred_axis
    };
    let junction = match branch_axis {
        geometry::TreeAxis::Down => egui::pos2(start.x, (start.y + end_average.y) * 0.5),
        geometry::TreeAxis::Right => egui::pos2((start.x + end_average.x) * 0.5, start.y),
    };
    (start, junction, ends, branch_axis)
}

fn draw_family_path(
    painter: &egui::Painter,
    path: &[Pos2],
    casing: Stroke,
    core: Stroke,
    wells: &[Rect],
) {
    // The family-colored core continues through its child space. The shared blue
    // casing only separates the family from the canvas outside an infill.
    for segment in path.windows(2) {
        let [start, end] = segment else {
            continue;
        };
        let mut visible = vec![(*start, *end)];
        for well in wells {
            visible = visible
                .into_iter()
                .flat_map(|(from, to)| segment_outside_rect(from, to, *well))
                .collect();
        }
        for (from, to) in visible {
            painter.line_segment([from, to], casing);
        }
    }
    painter.add(egui::Shape::line(path.to_vec(), core));
}

/// Returns the portions of a segment outside one axis-aligned child well.
/// This is Liang-Barsky clipping expressed as parameter intervals, so a casing
/// cannot peek through the middle of an infill merely because its midpoint lies
/// elsewhere.
fn segment_outside_rect(start: Pos2, end: Pos2, rect: Rect) -> Vec<(Pos2, Pos2)> {
    let delta = end - start;
    let mut enter = 0.0_f32;
    let mut exit = 1.0_f32;
    for (p, q) in [
        (-delta.x, start.x - rect.left()),
        (delta.x, rect.right() - start.x),
        (-delta.y, start.y - rect.top()),
        (delta.y, rect.bottom() - start.y),
    ] {
        if p.abs() <= f32::EPSILON {
            if q < 0.0 {
                return vec![(start, end)];
            }
            continue;
        }
        let ratio = q / p;
        if p < 0.0 {
            enter = enter.max(ratio);
        } else {
            exit = exit.min(ratio);
        }
    }
    if enter > exit || exit <= 0.0 || enter >= 1.0 {
        return vec![(start, end)];
    }
    let mut outside = Vec::new();
    if enter > 0.0 {
        outside.push((start, start.lerp(end, enter.clamp(0.0, 1.0))));
    }
    if exit < 1.0 {
        outside.push((start.lerp(end, exit.clamp(0.0, 1.0)), end));
    }
    outside
}

/// Returns the color a typed relation is drawn in.
fn relation_color(relation: RelationKind) -> Color32 {
    match relation {
        RelationKind::ContributesTo => Color32::from_rgb(171, 130, 205),
        RelationKind::Implements => Color32::from_rgb(198, 164, 92),
        RelationKind::RelatesTo => Color32::from_rgb(120, 160, 200),
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
/// Returns the ids of tasks that hold active work somewhere beneath them.
///
/// Derived on every read rather than stored, so it follows reparenting and clears
/// itself when the last active descendant finishes. See TE-0013.
fn ids_holding_active_work(items: &[StoredItem]) -> std::collections::HashSet<String> {
    let by_id = items
        .iter()
        .map(|item| (item.item.id.to_string(), item))
        .collect::<HashMap<_, _>>();
    let mut holders = std::collections::HashSet::new();
    for item in items {
        if item.item.state != ItemState::Active {
            continue;
        }
        let mut current = item.item.parent_id;
        let mut guard = 0;
        while let Some(parent) = current {
            let id = parent.to_string();
            if !holders.insert(id.clone()) {
                // This ancestor chain has already been walked for another
                // active task; walking it again would repeat the same work.
                break;
            }
            let Some(found) = by_id.get(&id) else {
                break;
            };
            guard += 1;
            if guard > items.len() {
                break;
            }
            current = found.item.parent_id;
        }
    }
    holders
}

/// One classification drives placement, framing, and dimming in Focus mode.
fn focus_is_active(
    item: &StoredItem,
    holding: &std::collections::HashSet<String>,
    theme: &theme::FocusTheme,
    today: NaiveDate,
) -> bool {
    let deferred = theme.future_includes_deferred
        && (item.item.defer_until.is_some_and(|date| date > today)
            || item.item.scheduled_on.is_some_and(|date| date > today));
    !deferred
        && (item.item.state == ItemState::Active || holding.contains(&item.item.id.to_string()))
}

/// Returns hierarchy rows that satisfy containment and visible dependencies.
///
/// Parent-to-child remains a precedence edge, while a prerequisite-to-dependent
/// edge adds only the vertical room it needs. The tree renderer still chooses
/// sibling placement, preserving family readability instead of flattening the
/// graph into a dependency diagram.
fn hierarchy_levels(
    items: &[StoredItem],
    visible: &std::collections::HashSet<String>,
) -> HashMap<String, usize> {
    let mut successors: std::collections::BTreeMap<String, std::collections::BTreeSet<String>> =
        std::collections::BTreeMap::new();
    let mut indegree = visible
        .iter()
        .map(|id| (id.clone(), 0_usize))
        .collect::<std::collections::BTreeMap<_, _>>();
    for item in items {
        let id = item.item.id.to_string();
        let mut predecessors = item
            .item
            .parent_id
            .map(|parent| parent.to_string())
            .into_iter()
            .chain(item.dependencies.iter().map(ToString::to_string))
            .filter(|predecessor| visible.contains(predecessor) && predecessor != &id)
            .collect::<Vec<_>>();
        predecessors.sort();
        predecessors.dedup();
        for predecessor in predecessors {
            if successors
                .entry(predecessor)
                .or_default()
                .insert(id.clone())
            {
                *indegree.entry(id.clone()).or_default() += 1;
            }
        }
    }
    let mut ready = indegree
        .iter()
        .filter(|(_, count)| **count == 0)
        .map(|(id, _)| id.clone())
        .collect::<std::collections::BTreeSet<_>>();
    let mut levels = visible
        .iter()
        .map(|id| (id.clone(), 0_usize))
        .collect::<HashMap<_, _>>();
    while let Some(id) = ready.first().cloned() {
        ready.remove(&id);
        let level = levels.get(&id).copied().unwrap_or_default();
        if let Some(next_ids) = successors.get(&id) {
            for next in next_ids {
                let next_level = levels.entry(next.clone()).or_default();
                *next_level = (*next_level).max(level.saturating_add(1));
                if let Some(count) = indegree.get_mut(next) {
                    *count = count.saturating_sub(1);
                    if *count == 0 {
                        ready.insert(next.clone());
                    }
                }
            }
        }
    }
    levels
}

/// Orders whole family boards when a dependency crosses their boundaries.
/// Dependencies inside a family are already handled by `hierarchy_levels`.
fn family_dependency_bands(
    items: &[StoredItem],
    roots: &[String],
    children: &std::collections::BTreeMap<String, Vec<String>>,
) -> HashMap<String, usize> {
    let mut owner = HashMap::new();
    for root in roots {
        let mut pending = vec![root.as_str()];
        while let Some(id) = pending.pop() {
            if owner.contains_key(id) {
                continue;
            }
            owner.insert(id.to_owned(), root.clone());
            if let Some(next) = children.get(id) {
                pending.extend(next.iter().map(String::as_str));
            }
        }
    }
    let mut successors: std::collections::BTreeMap<String, std::collections::BTreeSet<String>> =
        std::collections::BTreeMap::new();
    let mut indegree = roots
        .iter()
        .map(|id| (id.clone(), 0_usize))
        .collect::<HashMap<_, _>>();
    for item in items {
        let id = item.item.id.to_string();
        let Some(to) = owner.get(&id) else { continue };
        for dependency in &item.dependencies {
            let Some(from) = owner.get(&dependency.to_string()) else {
                continue;
            };
            if from != to
                && successors
                    .entry(from.clone())
                    .or_default()
                    .insert(to.clone())
            {
                *indegree.entry(to.clone()).or_default() += 1;
            }
        }
    }
    let mut ready = roots
        .iter()
        .filter(|id| indegree.get(*id) == Some(&0))
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();
    let mut bands = HashMap::new();
    while let Some(id) = ready.pop_first() {
        let band = bands.get(&id).copied().unwrap_or(0_usize);
        if let Some(next) = successors.get(&id) {
            for dependent in next {
                let level = bands.entry(dependent.clone()).or_insert(0);
                *level = (*level).max(band + 1);
                if let Some(count) = indegree.get_mut(dependent) {
                    *count -= 1;
                    if *count == 0 {
                        ready.insert(dependent.clone());
                    }
                }
            }
        }
    }
    // A cross-family cycle cannot be drawn as a one-way hierarchy. Keep its
    // members present in the first band rather than chasing levels forever.
    for root in roots {
        bands.entry(root.clone()).or_insert(0);
    }
    bands
}

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

/// Collapses only the view, preserving active and selected ancestor paths.
/// An expanded parent reveals its complete subtree regardless of the global cap.
fn limit_child_depth(
    eligible: Vec<StoredItem>,
    limit: usize,
    expanded: &std::collections::HashSet<String>,
    selected: Option<&str>,
) -> Vec<StoredItem> {
    if limit == 0 {
        return eligible;
    }
    let present = eligible
        .iter()
        .map(|item| item.item.id.to_string())
        .collect::<std::collections::HashSet<_>>();
    let parent_of = eligible
        .iter()
        .filter_map(|item| {
            let parent = item.item.parent_id?.to_string();
            present
                .contains(&parent)
                .then(|| (item.item.id.to_string(), parent))
        })
        .collect::<HashMap<_, _>>();
    let mut revealed = std::collections::HashSet::new();
    for item in &eligible {
        let id = item.item.id.to_string();
        if item.item.state != ItemState::Active && selected != Some(id.as_str()) {
            continue;
        }
        let mut cursor = id;
        while revealed.insert(cursor.clone()) {
            let Some(parent) = parent_of.get(&cursor) else {
                break;
            };
            cursor = parent.clone();
        }
    }
    eligible
        .into_iter()
        .filter(|item| {
            let id = item.item.id.to_string();
            if revealed.contains(&id) {
                return true;
            }
            let mut cursor = id;
            let mut depth = 0;
            let mut seen = std::collections::HashSet::new();
            while let Some(parent) = parent_of.get(&cursor) {
                if !seen.insert(cursor.clone()) {
                    break;
                }
                depth += 1;
                if expanded.contains(parent) {
                    return true;
                }
                cursor = parent.clone();
            }
            depth <= limit
        })
        .collect()
}

/// Family body and the descendant space it encloses.
#[derive(Clone)]
struct FamilyBox {
    parent_id: String,
    body: Rect,
    well: Rect,
    /// A configurable-width structural strip for a nearby parent. Detached
    /// boxes deliberately have no bespoke anchor: the ordinary family tree
    /// remains the one visual language for their connection.
    attachment: Option<Rect>,
    direct_join: bool,
}

/// A family uses one body, with the real parent card projecting as its tab.
/// Each filled part is opaque, so overlapping generations keep one color.
fn draw_family_boxes(
    painter: &egui::Painter,
    app: &TodoGui,
    items: &[StoredItem],
    boxes: &[FamilyBox],
    palette: &TagPalette,
    effective: &HashMap<String, Vec<String>>,
) {
    // Complete each ancestor before its descendants. Rectangle-area ordering can
    // swap nearly equal animated boards from frame to frame, causing borders to
    // flash above and below one another; structural depth is stable instead.
    let mut ordered = boxes.iter().collect::<Vec<_>>();
    ordered.sort_by(|left, right| {
        let depth = |id: &str| {
            items
                .iter()
                .find(|item| item.item.id.to_string() == id)
                .map_or(usize::MAX, |item| ancestor_depth(item, items))
        };
        depth(&left.parent_id)
            .cmp(&depth(&right.parent_id))
            .then_with(|| left.parent_id.cmp(&right.parent_id))
            .then_with(|| left.body.min.x.total_cmp(&right.body.min.x))
    });
    let children = child_index(items);
    for family in ordered {
        let Some(item) = items
            .iter()
            .find(|candidate| candidate.item.id.to_string() == family.parent_id)
        else {
            continue;
        };
        let accent =
            node_tag_color(item, &app.theme, palette, effective).unwrap_or(Color32::from_gray(150));
        let (fill, well_fill, edge) = family_colors(accent);
        let zoom = app.graph_zoom;
        let rounding = app.theme.parent.corner_radius * zoom;
        // Parent outlines are a structural enclosure, deliberately heavier
        // than cards and the child-well seam. The independent theme weight
        // keeps that hierarchy legible without changing every other stroke.
        let stroke = Stroke::new(
            (app.theme.parent.stroke_width * app.theme.parent.outline_weight * zoom).max(1.0),
            edge,
        );
        // Once a family is open its own cards carry the structure, so the fill
        // is only clutter over them. The boundary returns as a dashed outline
        // while the family is being moved, which is when its extent matters.
        if app.expanded_families.contains(&family.parent_id) {
            if dragging_family(app, &family.parent_id, &children) {
                draw_dashed_rect(painter, family.body, rounding, stroke);
            }
            continue;
        }
        if let Some(attachment) = family.attachment {
            painter.rect_filled(attachment, rounding * 0.7, fill);
            painter.rect_stroke(
                attachment,
                rounding * 0.7,
                stroke,
                egui::StrokeKind::Outside,
            );
        }
        match app.theme.parent.well_fill {
            FamilyWellFill::Transparent => {
                draw_family_ring(painter, family.body, family.well, rounding, fill);
                // The mesh leaves a true hole; a thin path smooths its inside
                // edge without laying a dark fill over the tag clouds below.
                painter.rect_stroke(
                    family.well,
                    rounding * 0.7,
                    Stroke::new(zoom.max(1.0), fill),
                    egui::StrokeKind::Outside,
                );
            }
            FamilyWellFill::Dark => {
                painter.rect_filled(family.body, rounding, fill);
                painter.rect_filled(family.well, rounding * 0.7, well_fill);
            }
        }
        painter.rect_stroke(family.body, rounding, stroke, egui::StrokeKind::Outside);
        if app.theme.parent.child_style == FamilyChildStyle::RecessedBoard {
            painter.rect_stroke(
                family.well,
                rounding * 0.7,
                Stroke::new((1.5 * zoom).max(1.0), edge),
                egui::StrokeKind::Inside,
            );
        }
    }
}

/// True while the pointer is dragging this family's parent or any descendant.
fn dragging_family(app: &TodoGui, id: &str, children: &HashMap<String, Vec<String>>) -> bool {
    app.drag_anchor
        .as_ref()
        .is_some_and(|anchor| collect_subtree(id, children).contains(&anchor.id))
}

/// Outlines a rounded rectangle with dashes, reusing the rounded path the solid
/// frames are built from so an open family keeps the shape it had when filled.
fn draw_dashed_rect(painter: &egui::Painter, rect: Rect, rounding: f32, stroke: Stroke) {
    let mut path = rounded_rect_path(rect, rounding);
    if let Some(first) = path.first().copied() {
        path.push(first);
    }
    painter.add(egui::Shape::dashed_line(
        &path,
        stroke,
        6.0 * stroke.width,
        4.0 * stroke.width,
    ));
}

/// Tessellates the area between two rounded rectangles. The inner rectangle is
/// never painted, so transparency exposes the actual graph underneath it.
fn draw_family_ring(
    painter: &egui::Painter,
    body: Rect,
    well: Rect,
    rounding: f32,
    color: Color32,
) {
    painter.add(egui::Shape::mesh(family_ring_mesh(
        body, well, rounding, color,
    )));
}

fn family_ring_mesh(body: Rect, well: Rect, rounding: f32, color: Color32) -> egui::Mesh {
    let outer = rounded_rect_path(body, rounding);
    let inner = rounded_rect_path(well, rounding * 0.7);
    let mut mesh = egui::Mesh::default();
    let Some(count) = u32::try_from(outer.len()).ok() else {
        return mesh;
    };
    for (outside, inside) in outer.into_iter().zip(inner) {
        mesh.colored_vertex(outside, color);
        mesh.colored_vertex(inside, color);
    }
    for index in 0..count {
        let current = index * 2;
        let next = ((index + 1) % count) * 2;
        mesh.add_triangle(current, current + 1, next);
        mesh.add_triangle(current + 1, next + 1, next);
    }
    mesh
}

fn rounded_rect_path(rect: Rect, rounding: f32) -> Vec<Pos2> {
    let radius = rounding
        .max(0.0)
        .min(rect.width() / 2.0)
        .min(rect.height() / 2.0);
    let corners = [
        (
            egui::pos2(rect.right() - radius, rect.top() + radius),
            -std::f32::consts::FRAC_PI_2,
        ),
        (
            egui::pos2(rect.right() - radius, rect.bottom() - radius),
            0.0,
        ),
        (
            egui::pos2(rect.left() + radius, rect.bottom() - radius),
            std::f32::consts::FRAC_PI_2,
        ),
        (
            egui::pos2(rect.left() + radius, rect.top() + radius),
            std::f32::consts::PI,
        ),
    ];
    let mut path = Vec::with_capacity(20);
    for (center, start) in corners {
        for step in 0_u8..=4 {
            let angle = start + f32::from(step) * std::f32::consts::FRAC_PI_2 / 4.0;
            path.push(center + Vec2::new(angle.cos(), angle.sin()) * radius);
        }
    }
    path
}

fn family_colors(accent: Color32) -> (Color32, Color32, Color32) {
    let blend = |base: u8, tint: u8| -> u8 {
        u8::try_from((u16::from(base) * 2 + u16::from(tint)) / 3).unwrap_or(u8::MAX)
    };
    (
        Color32::from_rgb(
            blend(34, accent.r()),
            blend(43, accent.g()),
            blend(60, accent.b()),
        ),
        Color32::from_rgb(28, 36, 50),
        Color32::from_rgb(
            blend(184, accent.r()),
            blend(199, accent.g()),
            blend(219, accent.b()),
        ),
    )
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

/// One disconnected body of a tag family in the current visible graph.
#[derive(Clone)]
struct TagCloud {
    path: String,
    points: Vec<Pos2>,
    /// The actual measured card bounds, retained so card clouds cover wrapped
    /// cards instead of a generic nominal card size.
    members: Vec<TagCloudMember>,
    /// Cards that do not carry this hierarchy path. Their nearest valid tag
    /// field is restored when this cloud crosses them, rather than exposing the
    /// canvas and making an apparent untagged hole.
    foreign_members: Vec<TagCloudFallback>,
    color: Color32,
}

#[derive(Clone)]
struct TagCloudFallback {
    rect: Rect,
    colors: Vec<Color32>,
}

#[derive(Clone)]
struct TagCloudMember {
    rect: Rect,
    /// Independent leaf-tag colors carried by this card. Several colors are
    /// rendered as cloud bands, never as a new card decoration.
    colors: Vec<Color32>,
}

fn draw_tag_constellation(
    painter: &egui::Painter,
    items: &[StoredItem],
    layouts: &HashMap<String, NodeLayout>,
    zoom: f32,
    theme: &Theme,
    palette: &TagPalette,
    effective: &HashMap<String, Vec<String>>,
) {
    let mut groups: HashMap<String, Vec<TagCloudMember>> = HashMap::new();
    let mut cards = Vec::<(Rect, Vec<String>, Vec<Color32>)>::new();
    for item in items {
        let id = item.item.id.to_string();
        let Some(layout) = layouts.get(&id) else {
            continue;
        };
        let empty = Vec::new();
        let carried = effective.get(&id).unwrap_or(&empty);
        let carried = carried.clone();
        let colors = independent_tag_colors(&carried, palette, &theme.tag);
        cards.push((layout.rect, carried.clone(), colors.clone()));
        if carried.is_empty() {
            groups
                .entry("(untagged)".to_owned())
                .or_default()
                .push(TagCloudMember {
                    rect: layout.rect,
                    colors,
                });
            continue;
        }
        let mut paths = std::collections::HashSet::new();
        for tag in &carried {
            let mut prefix = String::new();
            for (depth, segment) in tag.split('/').enumerate() {
                if !prefix.is_empty() {
                    prefix.push('/');
                }
                prefix.push_str(segment);
                let shown =
                    theme.tag.cloud_hierarchy_depth == 0 || depth < theme.tag.cloud_hierarchy_depth;
                if shown {
                    paths.insert(prefix.clone());
                }
            }
        }
        for path in paths {
            groups.entry(path).or_default().push(TagCloudMember {
                rect: layout.rect,
                colors: colors.clone(),
            });
        }
    }
    let mut paths = groups.keys().cloned().collect::<Vec<_>>();
    paths.sort();
    let half_size = {
        let (width, height) = theme.node.size();
        Vec2::new(width / 2.0 + 14.0, height / 2.0 + 10.0) * zoom
    };
    let mut clouds = Vec::new();
    for path in paths {
        let Some(members) = groups.get(&path) else {
            continue;
        };
        let color = if path == "(untagged)" {
            Color32::from_gray(118)
        } else {
            palette.color(&path, &theme.tag)
        };
        let points = members
            .iter()
            .map(|member| member.rect.center())
            .collect::<Vec<_>>();
        for cluster in geometry::cluster_points(&points, theme.tag.cluster_distance * zoom) {
            if cluster.len() < theme.tag.min_region_members
                && theme.tag.region_shape != RegionShape::Cloud
            {
                continue;
            }
            clouds.push(TagCloud {
                path: path.clone(),
                members: members
                    .iter()
                    .filter(|member| cluster.contains(&member.rect.center()))
                    .cloned()
                    .collect(),
                points: cluster,
                foreign_members: cards
                    .iter()
                    .filter_map(|(rect, tags, colors)| {
                        let carries_path = tags
                            .iter()
                            .any(|tag| tag == &path || tag.starts_with(&(path.clone() + "/")));
                        (!carries_path).then(|| TagCloudFallback {
                            rect: *rect,
                            colors: fallback_tag_colors(tags, colors, &path, palette, &theme.tag),
                        })
                    })
                    .collect(),
                color,
            });
        }
    }
    if clouds.is_empty() {
        return;
    }
    if theme.tag.region_shape != RegionShape::Cloud {
        draw_legacy_tag_regions(painter, &clouds, half_size, theme, zoom);
        return;
    }
    clouds.sort_by(|left, right| {
        left.path
            .matches('/')
            .count()
            .cmp(&right.path.matches('/').count())
            .then_with(|| right.members.len().cmp(&left.members.len()))
            .then_with(|| left.path.cmp(&right.path))
    });
    draw_clean_tag_clouds(painter, &clouds, theme, zoom);
    if theme.tag.cloud_cut_out_unrelated {
        // A deep cloud can legitimately cut through a different hierarchy's
        // shallow cloud. Redraw leaf clouds last so every task keeps the cloud
        // of the most specific tag it actually carries.
        let mut leaves = clouds
            .iter()
            .filter(|cloud| {
                !clouds
                    .iter()
                    .any(|other| other.path.starts_with(&(cloud.path.clone() + "/")))
            })
            .cloned()
            .collect::<Vec<_>>();
        leaves.sort_by(|left, right| {
            right
                .members
                .len()
                .cmp(&left.members.len())
                .then_with(|| left.path.cmp(&right.path))
        });
        draw_clean_tag_clouds(painter, &leaves, theme, zoom);
    }
    // A mixed field is an item-level fact, not one fact per inherited ancestor.
    // Draw it once above the solid hierarchy layers so parent circles stay
    // readable rather than accumulating repeated stripe textures.
    draw_multi_tag_cloud_members(painter, &cards, theme, zoom);
}

/// Draws the older rectangle and hull modes without changing their saved meaning.
fn draw_legacy_tag_regions(
    painter: &egui::Painter,
    clouds: &[TagCloud],
    half_size: Vec2,
    theme: &Theme,
    zoom: f32,
) {
    for cloud in clouds {
        let Some(bounds) = bounds_for_points(&cloud.points, half_size) else {
            continue;
        };
        let fill = Color32::from_rgba_unmultiplied(
            cloud.color.r(),
            cloud.color.g(),
            cloud.color.b(),
            theme.tag.region_fill_alpha,
        );
        let stroke = Stroke::new(
            1.0,
            Color32::from_rgba_unmultiplied(
                cloud.color.r(),
                cloud.color.g(),
                cloud.color.b(),
                theme.tag.region_stroke_alpha,
            ),
        );
        if theme.tag.region_shape == RegionShape::Hull {
            let corners = node_corners(&cloud.points, half_size);
            let hull = geometry::convex_hull(&corners);
            let padded = geometry::expand_path(&hull, theme.tag.region_padding);
            let outline = geometry::smooth_closed_path(&padded, theme.tag.region_smoothing);
            if outline.len() >= 3 {
                painter.add(egui::Shape::convex_polygon(outline, fill, stroke));
            }
        } else {
            painter.rect_filled(bounds.expand(16.0), 18.0, fill);
            painter.rect_stroke(bounds.expand(16.0), 18.0, stroke, egui::StrokeKind::Outside);
        }
        draw_region_label(painter, bounds, &cloud.path, cloud.color, theme, zoom);
    }
}

/// Draws a configurable card-sized or circular cloud for one tag family.
///
/// Links are painted first and members cap their ends. Card clouds deliberately
/// follow the task silhouette, so tags can read as a squarer rail through work
/// without becoming a family frame around the child subgraph.
fn draw_clean_tag_clouds(painter: &egui::Painter, clouds: &[TagCloud], theme: &Theme, zoom: f32) {
    let context = painter.ctx();
    let backdrop = context.style_of(context.theme()).visuals.panel_fill;
    for cloud in clouds {
        let fill = cloud_pastel(cloud.color, backdrop, &theme.tag);
        let padding = cloud_member_padding(&cloud.path, theme, zoom);
        let members = cloud
            .members
            .iter()
            .map(|member| (member.rect.expand(padding), member.colors.as_slice()))
            .collect::<Vec<_>>();
        let radius = members
            .iter()
            .map(|(member, _)| member.size().length() / 2.0)
            .fold(0.0_f32, f32::max);
        let rail_width = match theme.tag.cloud_member_shape {
            CloudMemberShape::Circle => radius * 1.15,
            CloudMemberShape::Card => {
                members
                    .iter()
                    .map(|(member, _)| member.height())
                    .sum::<f32>()
                    / f32::from(u16::try_from(members.len()).unwrap_or(u16::MAX)).max(1.0)
            }
        };
        if theme.tag.cloud_fill_gaps && cloud.points.len() >= 3 {
            let outline = geometry::convex_hull(&cloud.points);
            if outline.len() >= 3 {
                painter.add(egui::Shape::convex_polygon(outline, fill, Stroke::NONE));
            }
        }
        // The opaque pastel is pre-blended with the canvas once, so overlapping
        // circles and their tube stay one continuous color rather than darkening.
        let tube = Stroke::new(rail_width, fill);
        for (from, to) in tag_cloud_links(&cloud.points, theme, zoom) {
            // Clouds are now behind the family layer and cards, so a tag can span
            // one broad body of work without being mistaken for its foreground
            // relationship arrow. Separate truly distant constellations instead.
            if (to - from).length() <= radius * 7.0 {
                painter.line_segment([from, to], tube);
            }
        }
        for (member, _) in &members {
            draw_cloud_member(painter, *member, fill, &[], theme, zoom);
        }
        if theme.tag.cloud_cut_out_unrelated {
            let member_rects = members
                .iter()
                .map(|(member, _)| *member)
                .collect::<Vec<_>>();
            let cloud_bounds =
                union_rects(&member_rects).map(|bounds| bounds.expand(rail_width * 0.5));
            for fallback in &cloud.foreign_members {
                let foreign = fallback.rect.expand(padding);
                if !cloud_bounds.is_some_and(|bounds| bounds.intersects(foreign)) {
                    continue;
                }
                let fallback_fill = fallback
                    .colors
                    .first()
                    .map_or(backdrop, |color| cloud_pastel(*color, backdrop, &theme.tag));
                draw_cloud_member(
                    painter,
                    foreign,
                    fallback_fill,
                    &fallback.colors,
                    theme,
                    zoom,
                );
                if theme.tag.cloud_cutout_outline {
                    draw_cloud_member_outline(
                        painter,
                        foreign,
                        fallback.colors.first().copied().unwrap_or(cloud.color),
                        theme,
                        zoom,
                    );
                }
            }
        }
        let member_rects = members
            .iter()
            .map(|(member, _)| *member)
            .collect::<Vec<_>>();
        if let Some(bounds) = union_rects(&member_rects) {
            draw_cloud_label(painter, bounds, &cloud.path, cloud.color, theme, zoom);
        }
    }
}

/// Draws one foreground mixed cloud per multi-tag task after the hierarchy has
/// established its solid ancestor fields. Repeating this texture for every
/// inherited path makes nested clouds read as a rendering glitch.
fn draw_multi_tag_cloud_members(
    painter: &egui::Painter,
    cards: &[(Rect, Vec<String>, Vec<Color32>)],
    theme: &Theme,
    zoom: f32,
) {
    let backdrop = painter
        .ctx()
        .style_of(painter.ctx().theme())
        .visuals
        .panel_fill;
    for (rect, tags, colors) in cards {
        if colors.len() < 2 {
            continue;
        }
        let Some(deepest) = tags.iter().max_by_key(|tag| tag.matches('/').count()) else {
            continue;
        };
        let member = rect.expand(cloud_member_padding(deepest, theme, zoom));
        let fill = colors
            .first()
            .map_or(backdrop, |color| cloud_pastel(*color, backdrop, &theme.tag));
        draw_cloud_member(painter, member, fill, colors, theme, zoom);
    }
}

/// Selects cloud rails from a configurable local-neighborhood policy.
fn tag_cloud_links(points: &[Pos2], theme: &Theme, zoom: f32) -> Vec<(Pos2, Pos2)> {
    match theme.tag.cloud_link_mode {
        TagCloudLinkMode::Nearest => {
            if theme.tag.cloud_neighbor_count == 0 {
                return Vec::new();
            }
            let mut links = std::collections::BTreeSet::new();
            for (index, point) in points.iter().enumerate() {
                let mut candidates = points
                    .iter()
                    .enumerate()
                    .filter(|(other, _)| *other != index)
                    .map(|(other, candidate)| (point.distance(*candidate), other))
                    .collect::<Vec<_>>();
                candidates
                    .sort_by(|left, right| left.0.total_cmp(&right.0).then(left.1.cmp(&right.1)));
                for (_, other) in candidates.into_iter().take(theme.tag.cloud_neighbor_count) {
                    links.insert((index.min(other), index.max(other)));
                }
            }
            links
                .into_iter()
                .filter_map(|(from, to)| Some((*points.get(from)?, *points.get(to)?)))
                .collect()
        }
        TagCloudLinkMode::WithinRadius => {
            let radius = theme.tag.cloud_link_radius * zoom;
            let mut links = Vec::new();
            for (index, from) in points.iter().enumerate() {
                for to in points.iter().skip(index + 1) {
                    if from.distance(*to) <= radius {
                        links.push((*from, *to));
                    }
                }
            }
            links
        }
    }
}

fn union_rects(rects: &[Rect]) -> Option<Rect> {
    let first = *rects.first()?;
    Some(
        rects
            .iter()
            .skip(1)
            .fold(first, |bounds, rect| bounds.union(*rect)),
    )
}

fn app_cloud_rounding(theme: &Theme, zoom: f32) -> f32 {
    theme.node.rounding().min(18.0) * zoom
}

/// Ancestors deliberately occupy more space than their descendants. This keeps
/// a parent field readable behind its specific child field instead of making
/// every level look like the same-sized overlapping circle.
fn cloud_member_padding(path: &str, theme: &Theme, zoom: f32) -> f32 {
    let depth = path.matches('/').count();
    let ancestor_levels = 4_usize.saturating_sub(depth.min(4));
    let ancestor_bonus = f32::from(u8::try_from(ancestor_levels).unwrap_or(u8::MAX)) * 8.0;
    (theme.tag.region_padding + ancestor_bonus) * zoom
}

/// Paints one cloud island. When a card carries independent leaf tags, its
/// surrounding circular cloud receives repeated diagonal color stripes rather
/// than blended colors or a decoration on the card itself.
fn draw_cloud_member(
    painter: &egui::Painter,
    member: Rect,
    fill: Color32,
    colors: &[Color32],
    theme: &Theme,
    zoom: f32,
) {
    if colors.len() <= 1 {
        match theme.tag.cloud_member_shape {
            CloudMemberShape::Circle => {
                painter.circle_filled(member.center(), member.size().length() / 2.0, fill);
            }
            CloudMemberShape::Card => {
                painter.rect_filled(member, app_cloud_rounding(theme, zoom), fill);
            }
        }
        return;
    }
    match theme.tag.cloud_member_shape {
        CloudMemberShape::Circle => draw_diagonal_cloud_stripes(painter, member, colors, theme),
        CloudMemberShape::Card => {
            // Card-clouds keep a simple horizontal split. Their squared rails
            // make diagonal strokes look like accidental connector fragments;
            // the circular cloud is the intended stripe treatment.
            let count = f32::from(u16::try_from(colors.len()).unwrap_or(u16::MAX));
            let band_height = member.height() / count;
            let backdrop = painter
                .ctx()
                .style_of(painter.ctx().theme())
                .visuals
                .panel_fill;
            for (index, color) in colors.iter().enumerate() {
                let index = f32::from(u16::try_from(index).unwrap_or(u16::MAX));
                let band = Rect::from_min_max(
                    egui::pos2(member.left(), member.top() + index * band_height),
                    egui::pos2(
                        member.right(),
                        (member.top() + (index + 1.0) * band_height).min(member.bottom()),
                    ),
                );
                let clipped = painter.with_clip_rect(band);
                clipped.rect_filled(
                    member,
                    app_cloud_rounding(theme, zoom),
                    cloud_pastel(*color, backdrop, &theme.tag),
                );
            }
        }
    }
}

/// Fills a circular cloud with diagonal stripe polygons clipped against a
/// many-sided circular boundary. Filled geometry avoids stroke caps creating a
/// saw-toothed edge.
fn draw_diagonal_cloud_stripes(
    painter: &egui::Painter,
    member: Rect,
    colors: &[Color32],
    theme: &Theme,
) {
    let center = member.center();
    let radius = member.size().length() / 2.0;
    let backdrop = painter
        .ctx()
        .style_of(painter.ctx().theme())
        .visuals
        .panel_fill;
    let palette = colors
        .iter()
        .map(|color| cloud_pastel(*color, backdrop, &theme.tag))
        .collect::<Vec<_>>();
    // This is a texture, not a small number of competing wedges. Keep the
    // stripes narrow enough that a two-tag cloud reads as one circular field.
    let stripe_width = (radius * 0.09).clamp(5.0, 14.0);
    let Some(base) = palette.first().copied() else {
        return;
    };
    painter.circle_filled(center, radius, base);
    // A narrow solid rim gives the alternating fills a clean termination. It
    // keeps color changes from making the cloud's circular silhouette look
    // serrated at every stripe endpoint.
    let stripe_radius = (radius - stripe_width * 0.75).max(0.0);
    let direction = Vec2::new(0.707_106_77, 0.707_106_77);
    let normal = Vec2::new(-direction.y, direction.x);
    let circle = (0_u8..48)
        .map(|index| {
            let angle = -std::f32::consts::TAU * f32::from(index) / 48.0;
            center + Vec2::new(angle.cos(), angle.sin()) * stripe_radius
        })
        .collect::<Vec<_>>();
    let mut offset = -stripe_radius + stripe_width / 2.0;
    let mut stripe = 0_usize;
    while offset < stripe_radius {
        let start = center + normal * (offset - stripe_width / 2.0);
        let end = center + normal * (offset + stripe_width / 2.0);
        let length = stripe_radius * 2.5;
        let band = [
            start - direction * length,
            start + direction * length,
            end + direction * length,
            end - direction * length,
        ];
        let clipped = clip_convex_polygon(&band, &circle);
        if clipped.len() >= 3
            && let Some(color) = palette.get(stripe % palette.len()).copied()
        {
            painter.add(egui::Shape::convex_polygon(clipped, color, Stroke::NONE));
        }
        offset += stripe_width;
        stripe = stripe.saturating_add(1);
    }
}

/// Clips one convex polygon to another with Sutherland-Hodgman clipping.
/// `clip` is clockwise in screen coordinates, so its right side is in.
fn clip_convex_polygon(subject: &[Pos2], clip: &[Pos2]) -> Vec<Pos2> {
    let mut output = subject.to_vec();
    for (index, edge_start) in clip.iter().enumerate() {
        let Some(edge_end) = clip.get((index + 1) % clip.len()) else {
            return Vec::new();
        };
        let input = std::mem::take(&mut output);
        let Some(mut previous) = input.last().copied() else {
            break;
        };
        let mut previous_inside = left_of(*edge_start, *edge_end, previous);
        for current in input {
            let current_inside = left_of(*edge_start, *edge_end, current);
            if current_inside != previous_inside
                && let Some(intersection) =
                    line_intersection(previous, current, *edge_start, *edge_end)
            {
                output.push(intersection);
            }
            if current_inside {
                output.push(current);
            }
            previous = current;
            previous_inside = current_inside;
        }
    }
    output
}

fn left_of(start: Pos2, end: Pos2, point: Pos2) -> bool {
    (end.x - start.x) * (point.y - start.y) - (end.y - start.y) * (point.x - start.x) <= 0.0
}

fn line_intersection(start: Pos2, end: Pos2, clip_start: Pos2, clip_end: Pos2) -> Option<Pos2> {
    let line = end - start;
    let edge = clip_end - clip_start;
    let denominator = edge.x * line.y - edge.y * line.x;
    if denominator.abs() <= f32::EPSILON {
        return None;
    }
    let relative = start - clip_start;
    let factor = -(edge.x * relative.y - edge.y * relative.x) / denominator;
    Some(start + line * factor)
}

fn draw_cloud_member_outline(
    painter: &egui::Painter,
    member: Rect,
    color: Color32,
    theme: &Theme,
    zoom: f32,
) {
    match theme.tag.cloud_member_shape {
        CloudMemberShape::Circle => painter.circle_stroke(
            member.center(),
            member.size().length() / 2.0,
            Stroke::new(1.5 * zoom, color),
        ),
        CloudMemberShape::Card => painter.rect_stroke(
            member,
            app_cloud_rounding(theme, zoom),
            Stroke::new(1.5 * zoom, color),
            egui::StrokeKind::Outside,
        ),
    };
}

/// Keeps a cloud label within its own colored member space instead of floating
/// separately above a parent board where it could look like a family heading.
fn draw_cloud_label(
    painter: &egui::Painter,
    bounds: Rect,
    path: &str,
    color: Color32,
    theme: &Theme,
    zoom: f32,
) {
    let label_bounds = Rect::from_min_size(
        bounds.left_top() + Vec2::splat(5.0 * zoom),
        Vec2::new(
            bounds.width() - 10.0 * zoom,
            theme.tag.label_font_size * zoom + 4.0,
        ),
    );
    draw_region_label_inside(painter, label_bounds, path, color, theme, zoom);
}

/// Returns a fully opaque, low-saturation version of a tag color over `backdrop`.
///
/// Opacity would be composited once per circle and make a shared cloud darker at
/// its joins. Pre-blending once keeps the deliberately clean circle geometry while
/// making every overlap read as the same continuous body.
fn cloud_pastel(color: Color32, backdrop: Color32, tag: &TagTheme) -> Color32 {
    blend_color(backdrop, color, tag.cloud_mix_over(backdrop))
}

/// Moves a redundant outline toward its surrounding fill without removing the
/// card's selection and status cues.
fn blend_color(from: Color32, toward: Color32, toward_percent: u8) -> Color32 {
    let toward_percent = u16::from(toward_percent.min(100));
    let from_percent = 100_u16.saturating_sub(toward_percent);
    let blend = |first: u8, second: u8| {
        u8::try_from((u16::from(first) * from_percent + u16::from(second) * toward_percent) / 100)
            .map_or(u8::MAX, |value| value)
    };
    Color32::from_rgba_unmultiplied(
        blend(from.r(), toward.r()),
        blend(from.g(), toward.g()),
        blend(from.b(), toward.b()),
        blend(from.a(), toward.a()),
    )
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

/// Draws the same clipped tag label inside a cloud member instead of above it.
fn draw_region_label_inside(
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
    let per_character = (font_size * 0.55).max(1.0);
    let mut used = 0.0;
    let mut shown = String::new();
    let mut clipped = false;
    for character in text.chars() {
        if used + per_character > width {
            clipped = true;
            break;
        }
        shown.push(character);
        used += per_character;
    }
    if shown.is_empty() {
        return;
    }
    if clipped {
        shown.pop();
        shown.push('…');
    }
    painter.text(
        bounds.left_top(),
        Align2::LEFT_TOP,
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

/// A card's measured rectangle and the text laid out to fit it.
#[derive(Clone)]
struct NodeLayout {
    rect: Rect,
    title_galley: std::sync::Arc<egui::Galley>,
    meta_font: FontId,
    padding: f32,
}

/// Measures every visible card once, before anything is drawn.
///
/// Sizing has to happen before edges so an arrow can aim at the rectangle a card
/// occupies; doing it twice would lay out every title twice a frame.
fn node_layouts(
    painter: &egui::Painter,
    app: &TodoGui,
    items: &[StoredItem],
    positions: &HashMap<String, Pos2>,
    child_index: &HashMap<String, Vec<String>>,
) -> HashMap<String, NodeLayout> {
    let mut layouts = HashMap::new();
    for item in items {
        let id = item.item.id.to_string();
        let Some(center) = positions.get(&id) else {
            continue;
        };
        let (node_width, node_height) = app.theme.node.size();
        let scale = app
            .theme
            .weight
            .scale(item_weight(item, child_index, &app.theme));
        let zoom = app.graph_zoom * scale;
        let title_font = FontId::proportional(app.theme.node.title_font_size * zoom);
        let meta_font = FontId::proportional(app.theme.node.meta_font_size * zoom);
        // The title is laid out before the card is sized, so the card can grow to
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
        let rect = Rect::from_center_size(*center, Vec2::new(node_width * zoom, height));
        layouts.insert(
            id,
            NodeLayout {
                rect,
                title_galley,
                meta_font,
                padding,
            },
        );
    }
    layouts
}

/// Builds each family around the measured cards, from innermost to outermost.
///
/// A child family occupies space in its parent's well. The parent card keeps its
/// graph position and projects above the body as the real attached tab.
fn family_boxes(
    items: &[StoredItem],
    layouts: &HashMap<String, NodeLayout>,
    theme: &Theme,
    zoom: f32,
    presentations: &HashMap<String, FamilyPresentation>,
) -> Vec<FamilyBox> {
    let children = child_index(items);
    let mut boxes: Vec<FamilyBox> = Vec::new();
    let mut ordered = items.iter().collect::<Vec<_>>();
    ordered.sort_by_key(|item| std::cmp::Reverse(ancestor_depth(item, items)));
    for item in ordered {
        let id = item.item.id.to_string();
        let direct_children = children.get(&id).cloned().unwrap_or_default();
        if direct_children.is_empty() {
            continue;
        }
        let Some(parent) = layouts.get(&id).map(|layout| layout.rect) else {
            continue;
        };
        let view = match presentations.get(&id).copied() {
            Some(FamilyPresentation::Dynamic) => FamilyView::Adaptive,
            Some(FamilyPresentation::Box) => FamilyView::Boxed,
            Some(FamilyPresentation::Tree) => FamilyView::Connected,
            None => theme.parent.family_view,
        };
        if view == FamilyView::Connected {
            continue;
        }
        let clusters = match view {
            FamilyView::Adaptive => dynamic_child_clusters(
                &direct_children,
                layouts,
                theme.parent.dynamic_cluster_distance * zoom,
            ),
            FamilyView::Boxed => vec![direct_children],
            FamilyView::Connected => Vec::new(),
        };
        for cluster in clusters {
            if view == FamilyView::Adaptive
                && cluster.len() < theme.parent.dynamic_min_children.max(1)
            {
                continue;
            }
            let child_bounds = cluster
                .iter()
                .filter_map(|member| layouts.get(member).map(|layout| layout.rect))
                .fold(Rect::NOTHING, |bounds, rect| bounds.union(rect));
            if child_bounds == Rect::NOTHING {
                continue;
            }
            let child_bounds = if theme.parent.child_style == FamilyChildStyle::NestedFamily {
                cluster.iter().fold(child_bounds, |bounds, child| {
                    boxes
                        .iter()
                        .filter(|family| family.parent_id == *child)
                        .fold(bounds, |nested, family| nested.union(family.body))
                })
            } else {
                child_bounds
            };
            // Build the plain family first so containment is judged against the
            // actual rim, including padding—not merely the child-card bounds.
            // A card already inside its own frame must not grow a second bridge
            // through that frame.
            let unjoined = family_box(id.clone(), parent, child_bounds, theme, zoom, false);
            let parent_inside = unjoined.body.contains(parent.center());
            let aligned =
                parent.left() <= unjoined.body.right() && parent.right() >= unjoined.body.left();
            let near = rect_gap(parent, unjoined.body) <= theme.parent.parent_join_max_gap * zoom;
            let direct_join = !parent_inside && aligned && near;
            boxes.push(if direct_join {
                family_box(id.clone(), parent, child_bounds, theme, zoom, true)
            } else {
                unjoined
            });
        }
    }
    boxes
}

/// Finds connected components of direct children. The transitive rule keeps a
/// short chain readable as one local subgraph without turning remote work into
/// one giant containment frame.
fn dynamic_child_clusters(
    child_ids: &[String],
    layouts: &HashMap<String, NodeLayout>,
    range: f32,
) -> Vec<Vec<String>> {
    let mut seen = std::collections::HashSet::new();
    let mut groups = Vec::new();
    for start in child_ids {
        if !seen.insert(start.clone()) {
            continue;
        }
        let mut group = vec![start.clone()];
        let mut cursor = 0;
        while let Some(current) = group.get(cursor) {
            let Some(center) = layouts.get(current).map(|layout| layout.rect.center()) else {
                cursor += 1;
                continue;
            };
            for candidate in child_ids {
                if seen.contains(candidate) {
                    continue;
                }
                let Some(other) = layouts.get(candidate).map(|layout| layout.rect.center()) else {
                    continue;
                };
                if center.distance(other) <= range {
                    seen.insert(candidate.clone());
                    group.push(candidate.clone());
                }
            }
            cursor += 1;
        }
        groups.push(group);
    }
    groups
}

/// The physical clear air between two card/frame rectangles.
fn rect_gap(left: Rect, right: Rect) -> f32 {
    let horizontal = (left.left() - right.right())
        .max(right.left() - left.right())
        .max(0.0);
    let vertical = (left.top() - right.bottom())
        .max(right.top() - left.bottom())
        .max(0.0);
    Vec2::new(horizontal, vertical).length()
}

/// The visible band is the gap between `body` and `well`, so `body` is grown
/// from the well alone and is one `padding` thick on every side.
///
/// Taking the union with the parent card unconditionally, as an earlier version
/// did, made that band as thick as the parent-to-child distance: a card placed
/// far from its children turned the frame into an opaque slab spanning the gap.
/// The card is folded in only when it is close enough to read as the frame's
/// tab; at any greater distance the attachment stalk carries the relationship
/// and the frame stays a tight border around the children.
fn family_box(
    parent_id: String,
    parent: Rect,
    children: Rect,
    theme: &Theme,
    zoom: f32,
    direct_join: bool,
) -> FamilyBox {
    let mut well = children.expand((theme.parent.padding + theme.parent.nest_padding) * zoom);
    let side = theme.parent.padding * zoom;
    let body = well.expand(side);
    let overlap = (parent.height() * 0.18).min(14.0 * zoom);
    let attachment = (direct_join && theme.parent.header_style == FamilyHeaderStyle::AttachedTab)
        .then(|| {
            let top = parent.bottom() - overlap;
            let bottom = body.top().max(top + 1.0 * zoom);
            let half_width =
                (parent.width() * theme.parent.parent_join_width_ratio * 0.5).max(1.0 * zoom);
            Rect::from_min_max(
                egui::pos2(parent.center().x - half_width, top),
                egui::pos2(parent.center().x + half_width, bottom),
            )
        });
    // The attached card is the cap of this family, not part of the recessed
    // child space. Clipping the well below its tab prevents a dark inset from
    // bleeding through the parent-card area when a child sits very close.
    if theme.parent.header_style == FamilyHeaderStyle::AttachedTab
        && theme.parent.clip_well_below_attached_tab
    {
        let tab_bottom = parent.bottom() - overlap;
        well.set_top(well.top().max(tab_bottom).min(well.bottom()));
    }
    FamilyBox {
        parent_id,
        body,
        well,
        attachment,
        direct_join,
    }
}

/// Orders two tasks by the chosen sort, breaking ties on title for stability.
fn sort_items(
    sort: SortBy,
    left: &StoredItem,
    right: &StoredItem,
    weights: &HashMap<String, f32>,
    sequence_rank: &HashMap<String, usize>,
) -> std::cmp::Ordering {
    let title = || left.item.title.cmp(&right.item.title);
    match sort {
        SortBy::Title => title(),
        // Most urgent first, which is the order these are usually scanned in.
        SortBy::Priority => right
            .item
            .priority
            .rank()
            .cmp(&left.item.priority.rank())
            .then_with(title),
        SortBy::Due => match (left.item.due_on, right.item.due_on) {
            (Some(first), Some(second)) => first.cmp(&second).then_with(title),
            // A task with no due date sorts after one that has one.
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => title(),
        },
        SortBy::Weight => {
            let weight = |item: &StoredItem| {
                weights
                    .get(&item.item.id.to_string())
                    .copied()
                    .unwrap_or_default()
            };
            weight(right).total_cmp(&weight(left)).then_with(title)
        }
        SortBy::State => left
            .item
            .state
            .to_string()
            .cmp(&right.item.state.to_string())
            .then_with(title),
        SortBy::Sequence => {
            let rank = |item: &StoredItem| {
                sequence_rank
                    .get(&item.item.id.to_string())
                    .copied()
                    // Work outside every sequence follows the planned run.
                    .unwrap_or(usize::MAX)
            };
            rank(left).cmp(&rank(right)).then_with(title)
        }
        // Identifiers are time-ordered, so comparing them is capture order.
        SortBy::Created => left.item.id.to_string().cmp(&right.item.id.to_string()),
    }
}

/// Orders the labels a grouping exposes in the order people expect to scan them.
///
/// Group keys are rendered strings because they become column headings. Their
/// lexical order is fine for names, but it makes a time board read "later" before
/// "overdue" and a priority board put "critical" after "high".
fn compare_group_labels(grouping: GroupBy, left: &str, right: &str) -> std::cmp::Ordering {
    let rank = |label: &str| match grouping {
        GroupBy::Time => match label {
            "overdue" => 0,
            "today" => 1,
            "this week" => 2,
            "later" => 3,
            "(undated)" => 4,
            _ => 5,
        },
        GroupBy::Priority => match label {
            "critical" => 0,
            "high" => 1,
            "normal" => 2,
            "low" => 3,
            _ => 4,
        },
        GroupBy::State => match label {
            "active" => 0,
            "inbox" => 1,
            "done" => 2,
            "canceled" => 3,
            "void" => 4,
            _ => 5,
        },
        GroupBy::None | GroupBy::Hierarchy | GroupBy::Tag | GroupBy::Sequence => 0,
    };
    rank(left).cmp(&rank(right)).then_with(|| left.cmp(right))
}

/// Returns the lifecycle color for a task.
fn status_color(item: &StoredItem, holds_active: bool) -> Color32 {
    if !item.blocked_by.is_empty() {
        return Color32::from_rgb(119, 57, 51);
    }
    match item.item.state {
        ItemState::Active => Color32::from_rgb(47, 104, 72),
        ItemState::Done => Color32::from_gray(80),
        ItemState::Canceled => Color32::from_rgb(100, 61, 61),
        // Void work is drawn flattest of all: noise kept only so it can be
        // brought back if it was voided by mistake.
        ItemState::Void => Color32::from_gray(52),
        // A task holding active work is drawn as underway, in a quieter shade
        // than work that was picked up in its own right.
        ItemState::Inbox if holds_active => Color32::from_rgb(44, 82, 64),
        ItemState::Inbox => Color32::from_rgb(59, 79, 94),
    }
}

/// Returns the color a visual channel should carry, or `None` for neutral.
///
/// `tint` asks for a color dark enough to sit behind text, which is what the body
/// needs and the bar and border do not.
fn channel_color(
    channel: Channel,
    status: Color32,
    accent: Option<Color32>,
    item: &StoredItem,
    weight: f32,
    theme: &Theme,
    tint: bool,
) -> Option<Color32> {
    let color = match channel {
        Channel::None => return None,
        Channel::Status => return Some(status),
        Channel::Tag => accent?,
        Channel::Priority => {
            let rank = f32::from(item.item.priority.rank().min(3)) / 3.0;
            theme.tag.shade(rank, 0)
        }
        Channel::Weight => theme.tag.shade(weight.clamp(0.0, 1.0), 0),
    };
    Some(if tint {
        tinted(color, theme.node.body_tint_value)
    } else {
        color
    })
}

/// Returns a color darkened to a fixed brightness, keeping its hue.
///
/// A bright tag color cannot sit behind white text; darkening it to a common value
/// keeps the family recognizable while the title stays readable.
fn tinted(color: Color32, value: f32) -> Color32 {
    let mut hsva = egui::ecolor::Hsva::from(color);
    hsva.v = value.clamp(0.05, 1.0);
    hsva.s = (hsva.s * 0.9).clamp(0.0, 1.0);
    Color32::from(hsva)
}

/// Returns the tag colors a card carries, ordered from most specific to broadest.
///
/// The card never takes a color from a region it merely sits inside. Independent
/// tags at the same depth break alphabetically, so the dominant accent remains
/// stable across redraws.
fn node_tag_colors(
    item: &StoredItem,
    theme: &Theme,
    palette: &TagPalette,
    effective: &HashMap<String, Vec<String>>,
) -> Vec<Color32> {
    let carried = effective.get(&item.item.id.to_string());
    let mut tags = match carried {
        Some(tags) => tags.iter().map(String::as_str).collect::<Vec<_>>(),
        None => item
            .namespaces
            .iter()
            .map(NamespacePath::as_str)
            .collect::<Vec<_>>(),
    };
    tags.sort_unstable_by(|left, right| {
        right
            .matches('/')
            .count()
            .cmp(&left.matches('/').count())
            .then_with(|| left.cmp(right))
    });
    tags.dedup();
    tags.into_iter()
        .map(|tag| palette.color(tag, &theme.tag))
        .collect()
}

/// Returns a node's dominant tag accent, its own deepest tag.
fn node_tag_color(
    item: &StoredItem,
    theme: &Theme,
    palette: &TagPalette,
    effective: &HashMap<String, Vec<String>>,
) -> Option<Color32> {
    node_tag_colors(item, theme, palette, effective)
        .into_iter()
        .next()
}

/// Returns colors for tags that are not ancestors of another carried tag.
/// These are the independent memberships a cloud needs to show as bands.
fn independent_tag_colors(
    carried: &[String],
    palette: &TagPalette,
    theme: &theme::TagTheme,
) -> Vec<Color32> {
    let mut leaves = carried
        .iter()
        .map(String::as_str)
        .filter(|tag| {
            !carried
                .iter()
                .any(|other| other != tag && other.starts_with(&format!("{tag}/")))
        })
        .collect::<Vec<_>>();
    leaves.sort_unstable_by(|left, right| {
        right
            .matches('/')
            .count()
            .cmp(&left.matches('/').count())
            .then_with(|| left.cmp(right))
    });
    leaves
        .into_iter()
        .map(|tag| palette.color(tag, theme))
        .collect()
}

/// Chooses the most specific tag a card carries that is an ancestor of the
/// cloud crossing it. That restores a valid lower hierarchy field when a deeper
/// sibling cloud has to yield. For unrelated fields, retain every independent
/// tag color so their surrounding cloud can be banded rather than blank.
fn fallback_tag_colors(
    carried: &[String],
    independent_colors: &[Color32],
    cloud_path: &str,
    palette: &TagPalette,
    theme: &theme::TagTheme,
) -> Vec<Color32> {
    let mut matching = carried
        .iter()
        .map(String::as_str)
        .filter(|tag| cloud_path == *tag || cloud_path.starts_with(&format!("{tag}/")))
        .collect::<Vec<_>>();
    matching.sort_unstable_by(|left, right| {
        right
            .matches('/')
            .count()
            .cmp(&left.matches('/').count())
            .then_with(|| left.cmp(right))
    });
    matching
        .into_iter()
        .next()
        .map(|tag| vec![palette.color(tag, theme)])
        .unwrap_or_else(|| independent_colors.to_vec())
}

fn load_layout(path: &std::path::Path) -> LocalGraphLayout {
    fs::read_to_string(path)
        .ok()
        .and_then(|contents| serde_json::from_str::<LocalGraphLayout>(&contents).ok())
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

/// The card's meta line: state, then priority or a note that it holds active
/// work, then `agent` when an agent created the task.
fn node_meta(item: &StoredItem, holds_active: bool) -> String {
    let mut meta = if holds_active && item.item.state != ItemState::Active {
        format!("{} · holds active work", item.item.state)
    } else {
        format!("{} · {}", item.item.state, item.item.priority)
    };
    if item.agent.is_some() {
        meta.push_str(" · agent");
    }
    meta
}

/// An actor's name without the `actor/` kind.
/// How a time field reads in the inspector, or `None` when it is unset.
fn time_value(item: &StoredItem, edit: Edit, today: NaiveDate) -> Option<String> {
    match edit {
        Edit::Due => item.item.due_on.map(|date| day_label(date, today)),
        Edit::Defer => item.item.defer_until.map(|date| day_label(date, today)),
        Edit::Scheduled => item.item.scheduled_on.map(|date| day_label(date, today)),
        Edit::Estimate => item.item.estimate_minutes.map(minutes_label),
        Edit::Difficulty => item.item.difficulty.map(|value| format!("{value} / 5")),
        Edit::Title | Edit::Person => None,
    }
}

/// A date as a person says it, with how far away it is: "Nov 1 · in 12 days".
fn day_label(date: NaiveDate, today: NaiveDate) -> String {
    let format = if date.year() == today.year() {
        "%b %-d"
    } else {
        "%b %-d %Y"
    };
    let days = (date - today).num_days();
    let distance = match days {
        0 => "today".to_owned(),
        1 => "tomorrow".to_owned(),
        -1 => "yesterday".to_owned(),
        2.. => format!("in {days} days"),
        _ => format!("{} days ago", -days),
    };
    format!("{} · {distance}", date.format(format))
}

/// Minutes as hours and minutes: "45 min", "2 h", "1 h 30 min".
fn minutes_label(minutes: u32) -> String {
    match (minutes / 60, minutes % 60) {
        (0, rest) => format!("{rest} min"),
        (hours, 0) => format!("{hours} h"),
        (hours, rest) => format!("{hours} h {rest} min"),
    }
}

/// The dim, fixed-width key at the start of an inspector row.
fn inspector_key(ui: &mut egui::Ui, key: &str) {
    let size = Vec2::new(76.0, ui.spacing().interact_size.y);
    ui.allocate_ui_with_layout(
        size,
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            ui.set_min_size(size);
            ui.weak(key);
        },
    );
}

/// One read-first inspector row: a key, its value, and `✕` while hovered.
/// Returns whether the value and whether `✕` were clicked.
fn inspector_row(
    ui: &mut egui::Ui,
    key: &str,
    value: impl Into<egui::WidgetText>,
    removable: bool,
) -> (bool, bool) {
    ui.horizontal(|ui| {
        let hovered = ui.rect_contains_pointer(ui.max_rect());
        inspector_key(ui, key);
        let value = ui.add(egui::Label::new(value).sense(Sense::click()).truncate());
        let remove = removable && hovered && ui.small_button("✕").on_hover_text("Remove").clicked();
        (value.clicked(), remove)
    })
    .inner
}

/// A one-line in-place editor. Returns `Some(true)` on Enter or `✓`,
/// `Some(false)` on Escape, and `None` while still editing.
fn inline_text(
    ui: &mut egui::Ui,
    draft: &mut String,
    focus: &mut bool,
    hint: &str,
) -> Option<bool> {
    let field = ui.add(
        egui::TextEdit::singleline(draft)
            .desired_width(140.0)
            .hint_text(hint),
    );
    if std::mem::take(focus) {
        field.request_focus();
    }
    if ui.input(|input| input.key_pressed(egui::Key::Escape)) {
        return Some(false);
    }
    let entered = field.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));
    let confirmed = ui.small_button("✓").clicked();
    (entered || confirmed).then_some(true)
}

fn actor_name(path: &NamespacePath) -> &str {
    path.as_str()
        .strip_prefix("actor/")
        .unwrap_or(path.as_str())
}

fn short_title(title: &str) -> String {
    title.chars().take(22).collect()
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    reason = "test fixtures use direct setup and assertion values"
)]
mod tests {
    use super::{
        AgentFilter, FamilyPresentation, FilterChip, GroupBy, LedgerAccess, LocalGraphLayout,
        Runtime, TodoGui, collect_subtree, compare_group_labels, fallback_tag_colors, family_box,
        family_dependency_bands, family_ring_mesh, family_tree_geometry, file_stamp,
        focus_is_active, hierarchy_levels, limit_child_depth, matches_query, node_meta,
        segment_outside_rect,
    };
    use eframe::egui::{Color32, Pos2, Rect, pos2, vec2};
    use std::collections::{HashMap, HashSet};
    use todo_core::{ItemState, Priority};
    use todo_store::StoredItem;

    #[test]
    fn a_file_stamp_sees_edits_and_repointed_links() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("theme.toml");
        assert_eq!(file_stamp(&path), None);
        std::fs::write(&path, "a = 1\n").unwrap();
        let first = file_stamp(&path);
        assert!(first.is_some());
        assert_eq!(file_stamp(&path), first);
        std::fs::write(&path, "a = 12\n").unwrap();
        assert_ne!(file_stamp(&path), first);

        // Nix store files share one modification time, so only the link target
        // tells two generations apart.
        let epoch = std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1);
        let targets = ["old", "new"].map(|name| {
            let target = dir.path().join(name);
            std::fs::write(&target, "same\n").unwrap();
            std::fs::File::options()
                .write(true)
                .open(&target)
                .unwrap()
                .set_modified(epoch)
                .unwrap();
            target
        });
        let link = dir.path().join("palette");
        std::os::unix::fs::symlink(&targets[0], &link).unwrap();
        let before = file_stamp(&link);
        std::fs::remove_file(&link).unwrap();
        std::os::unix::fs::symlink(&targets[1], &link).unwrap();
        assert_ne!(file_stamp(&link), before);
    }

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

    /// Builds visible items with a given parent and state, for activity tests.
    fn task(id: &str, parent: Option<&str>, state: ItemState) -> StoredItem {
        let parse = |value: &str| {
            let padded = format!("0198f58a-1c00-7000-8000-0000000000{value:0>2}");
            todo_core::ItemId::parse(&padded).unwrap()
        };
        StoredItem {
            item: todo_core::Item {
                id: parse(id),
                title: format!("task {id}"),
                state,
                project_id: None,
                parent_id: parent.map(parse),
                priority: Priority::Normal,
                due_on: None,
                defer_until: None,
                scheduled_on: None,
                difficulty: None,
                estimate_minutes: None,
            },
            namespaces: Vec::new(),
            dependencies: Vec::new(),
            blocked_by: Vec::new(),
            relations: Vec::new(),
            agent: None,
        }
    }

    fn holds(items: &[StoredItem], id: &str) -> bool {
        let holders = super::ids_holding_active_work(items);
        items
            .iter()
            .find(|item| item.item.title == format!("task {id}"))
            .is_some_and(|item| holders.contains(&item.item.id.to_string()))
    }

    #[test]
    fn attached_parent_uses_the_configured_width_not_a_wide_body_band() {
        let parent = Rect::from_min_size(pos2(100.0, 20.0), vec2(150.0, 58.0));
        let child = Rect::from_min_size(pos2(300.0, 150.0), vec2(150.0, 70.0));
        let theme = super::Theme::default();
        let family = family_box("parent".to_owned(), parent, child, &theme, 1.0, true);
        assert!(family.body.top() > parent.bottom());
        assert!(family.attachment.is_some());
        let attachment = family.attachment.unwrap();
        assert_eq!(
            attachment.width(),
            parent.width() * theme.parent.parent_join_width_ratio
        );
        assert!(attachment.contains(parent.center_bottom()));
        assert!(family.well.contains(child.min));
        assert!(family.well.contains(child.max));
        assert!(family.body.contains(family.well.min));
        assert!(family.body.contains(family.well.max));
    }

    #[test]
    fn attached_tab_keeps_the_child_well_out_of_the_parent_card() {
        let parent = Rect::from_min_size(pos2(100.0, 20.0), vec2(150.0, 58.0));
        // A closely placed child used to pull its inset behind the attached tab.
        let child = Rect::from_min_size(pos2(110.0, 72.0), vec2(140.0, 70.0));
        let theme = super::Theme::default();
        let family = family_box("parent".to_owned(), parent, child, &theme, 1.0, true);
        let overlap = (parent.height() * 0.18).min(14.0);
        assert!(family.well.top() >= parent.bottom() - overlap);
        assert!(family.well.contains(child.center()));
    }

    #[test]
    fn a_connector_always_carries_both_its_family_and_the_shared_blue() {
        let fill = Color32::from_rgb(90, 60, 55);
        let (casing, core) =
            super::connector_strokes(fill, super::ConnectorStyle::FillCore, 4.0, 1.0);
        assert_eq!(core.color, fill);
        assert_eq!(casing.color, super::FAMILY_BLUE);
        // The casing has to be wider, or the core would cover it completely.
        assert!(casing.width > core.width);

        let (casing, core) =
            super::connector_strokes(fill, super::ConnectorStyle::BlueCore, 4.0, 1.0);
        assert_eq!(core.color, super::FAMILY_BLUE);
        assert_eq!(casing.color, fill);
        assert!(casing.width > core.width);
    }

    #[test]
    fn deep_cloud_cutout_restores_the_closest_carried_ancestor_color() {
        let theme = super::Theme::default();
        let palette =
            super::TagPalette::build(["tag/work/cdint", "tag/work/cdint/todoapp"], &theme.tag);
        let carried = vec!["tag/work/cdint".to_owned()];
        assert_eq!(
            fallback_tag_colors(
                &carried,
                &[palette.color("tag/work/cdint", &theme.tag)],
                "tag/work/cdint/todoapp",
                &palette,
                &theme.tag,
            ),
            vec![palette.color("tag/work/cdint", &theme.tag)],
        );
    }

    #[test]
    fn a_distant_parent_does_not_thicken_the_family_band() {
        let theme = super::Theme::default();
        let side = theme.parent.padding;
        let child = Rect::from_min_size(pos2(900.0, 900.0), vec2(150.0, 70.0));
        // Far enough that an earlier version spanned the whole gap with fill.
        let parent = Rect::from_min_size(pos2(100.0, 20.0), vec2(150.0, 58.0));
        let family = family_box("parent".to_owned(), parent, child, &theme, 1.0, false);
        for band in [
            family.well.left() - family.body.left(),
            family.body.right() - family.well.right(),
            family.well.top() - family.body.top(),
            family.body.bottom() - family.well.bottom(),
        ] {
            assert!(
                (band - side).abs() < 0.01,
                "band {band} should be one padding ({side})"
            );
        }
    }

    #[test]
    fn an_adjacent_parent_gets_a_configurable_rim_join() {
        let theme = super::Theme::default();
        let child = Rect::from_min_size(pos2(100.0, 140.0), vec2(150.0, 70.0));
        let parent = Rect::from_min_size(pos2(100.0, 20.0), vec2(150.0, 58.0));
        let family = family_box("parent".to_owned(), parent, child, &theme, 1.0, true);
        assert!(family.direct_join);
        assert!(
            family
                .attachment
                .is_some_and(|attachment| attachment.contains(parent.center_bottom()))
        );
    }

    #[test]
    fn low_child_depth_can_expand_and_reveals_active_paths() {
        let items = vec![
            task("1", None, ItemState::Inbox),
            task("2", Some("1"), ItemState::Inbox),
            task("3", Some("2"), ItemState::Inbox),
            task("4", Some("2"), ItemState::Active),
        ];
        let ids = |shown: Vec<StoredItem>| {
            shown
                .iter()
                .map(|item| item.item.title.clone())
                .collect::<Vec<_>>()
        };
        let collapsed = ids(limit_child_depth(items.clone(), 1, &HashSet::new(), None));
        assert!(collapsed.contains(&"task 4".to_owned()));
        assert!(!collapsed.contains(&"task 3".to_owned()));
        let root = items.first().map(|item| item.item.id.to_string()).unwrap();
        let expanded = ids(limit_child_depth(
            items.clone(),
            1,
            &HashSet::from([root]),
            None,
        ));
        assert_eq!(expanded.len(), items.len());
        assert_eq!(
            ids(limit_child_depth(items.clone(), 0, &HashSet::new(), None)).len(),
            items.len()
        );
    }

    #[test]
    fn focus_uses_the_same_active_rule_for_holding_and_deferred_tasks() {
        let today = chrono::NaiveDate::from_ymd_opt(2026, 9, 29).unwrap();
        let mut parent = task("1", None, ItemState::Inbox);
        let id = parent.item.id.to_string();
        let holding = HashSet::from([id]);
        let theme = super::Theme::default();
        assert!(focus_is_active(&parent, &holding, &theme.focus, today));
        parent.item.defer_until = chrono::NaiveDate::from_ymd_opt(2026, 10, 1);
        assert!(!focus_is_active(&parent, &holding, &theme.focus, today));
    }

    #[test]
    fn transparent_family_ring_leaves_the_child_space_open() {
        let body = Rect::from_min_max(pos2(0.0, 0.0), pos2(300.0, 240.0));
        let well = Rect::from_min_max(pos2(25.0, 70.0), pos2(275.0, 215.0));
        let mesh = family_ring_mesh(body, well, 10.0, Color32::WHITE);
        let covers = |point: Pos2| {
            mesh.indices.chunks_exact(3).any(|triangle| {
                let vertices = triangle
                    .iter()
                    .filter_map(|index| usize::try_from(*index).ok())
                    .filter_map(|index| mesh.vertices.get(index).map(|vertex| vertex.pos))
                    .collect::<Vec<_>>();
                let [a, b, c] = vertices.as_slice() else {
                    return false;
                };
                let cross = |first: Pos2, second: Pos2| {
                    (second.x - first.x) * (point.y - first.y)
                        - (second.y - first.y) * (point.x - first.x)
                };
                let sides = [cross(*a, *b), cross(*b, *c), cross(*c, *a)];
                sides.iter().all(|side| *side >= 0.0) || sides.iter().all(|side| *side <= 0.0)
            })
        };
        assert!(covers(pos2(10.0, 120.0)));
        assert!(!covers(well.center()));
    }

    #[test]
    fn siblings_share_one_family_junction_above_their_terminals() {
        let parent = Rect::from_min_size(pos2(100.0, 20.0), vec2(150.0, 58.0));
        let left = Rect::from_min_size(pos2(40.0, 200.0), vec2(120.0, 60.0));
        let right = Rect::from_min_size(pos2(260.0, 240.0), vec2(120.0, 60.0));
        let (start, junction, terminals, axis) = family_tree_geometry(
            parent,
            None,
            &[left, right],
            4.0,
            super::geometry::TreeAxis::Down,
        );
        assert_eq!(terminals.len(), 2);
        assert_eq!(axis, super::geometry::TreeAxis::Down);
        assert_eq!(junction.x, start.x);
        assert!(junction.y > start.y);
        assert!(terminals.iter().all(|terminal| terminal.y > junction.y));
    }

    #[test]
    fn family_casing_is_split_at_child_well_edges() {
        let pieces = segment_outside_rect(
            pos2(0.0, 10.0),
            pos2(100.0, 10.0),
            Rect::from_min_max(pos2(30.0, 0.0), pos2(70.0, 20.0)),
        );
        assert_eq!(pieces.len(), 2);
        let mut pieces = pieces.into_iter();
        let left = pieces.next().unwrap();
        let right = pieces.next().unwrap();
        assert!((left.1.x - 30.0).abs() < 0.001);
        assert!((left.1.y - 10.0).abs() < 0.001);
        assert!((right.0.x - 70.0).abs() < 0.001);
        assert!((right.0.y - 10.0).abs() < 0.001);
    }

    #[test]
    fn family_display_overrides_round_trip_in_local_layout_only() {
        let mut family_presentations = HashMap::new();
        family_presentations.insert("parent-a".to_owned(), FamilyPresentation::Tree);
        family_presentations.insert("parent-b".to_owned(), FamilyPresentation::Box);
        let layout = LocalGraphLayout {
            node_offsets: HashMap::new(),
            family_presentations,
        };
        let encoded = serde_json::to_string(&layout).unwrap();
        let decoded: LocalGraphLayout = serde_json::from_str(&encoded).unwrap();
        assert_eq!(
            decoded.family_presentations.get("parent-a"),
            Some(&FamilyPresentation::Tree)
        );
        assert_eq!(
            decoded.family_presentations.get("parent-b"),
            Some(&FamilyPresentation::Box)
        );
    }

    #[test]
    fn displaced_children_keep_the_shared_angular_family_route() {
        let parent = Rect::from_min_size(pos2(100.0, 120.0), vec2(150.0, 58.0));
        let above = Rect::from_min_size(pos2(20.0, 20.0), vec2(120.0, 60.0));
        let beside = Rect::from_min_size(pos2(340.0, 160.0), vec2(120.0, 60.0));
        let (start, junction, terminals, axis) = family_tree_geometry(
            parent,
            None,
            &[above, beside],
            4.0,
            super::geometry::TreeAxis::Down,
        );
        assert_eq!(terminals.len(), 2);
        assert!(matches!(
            axis,
            super::geometry::TreeAxis::Down | super::geometry::TreeAxis::Right
        ));
        match axis {
            super::geometry::TreeAxis::Down => assert_eq!(junction.x, start.x),
            super::geometry::TreeAxis::Right => assert_eq!(junction.y, start.y),
        }
    }

    #[test]
    fn direct_rim_join_can_be_the_source_of_a_family_tree() {
        let parent = Rect::from_min_size(pos2(100.0, 20.0), vec2(150.0, 58.0));
        let well = Rect::from_min_size(pos2(90.0, 150.0), vec2(320.0, 190.0));
        let child = Rect::from_min_size(pos2(180.0, 230.0), vec2(150.0, 58.0));
        let source = super::geometry::boundary_point(well, parent.center());
        let (start, _, _, _) = family_tree_geometry(
            parent,
            Some(source),
            &[child],
            4.0,
            super::geometry::TreeAxis::Down,
        );
        assert_eq!(start, source);
    }

    #[test]
    fn a_parent_of_active_work_holds_active_work() {
        let items = vec![
            task("1", None, ItemState::Inbox),
            task("2", Some("1"), ItemState::Active),
        ];
        assert!(holds(&items, "1"));
    }

    #[test]
    fn hierarchy_levels_place_dependents_after_their_prerequisites() {
        let root = task("1", None, ItemState::Inbox);
        let prerequisite = task("2", Some("1"), ItemState::Inbox);
        let mut dependent = task("3", None, ItemState::Inbox);
        dependent.dependencies = vec![prerequisite.item.id];
        let prerequisite_id = prerequisite.item.id.to_string();
        let dependent_id = dependent.item.id.to_string();
        let items = vec![root, prerequisite, dependent];
        let visible = items
            .iter()
            .map(|item| item.item.id.to_string())
            .collect::<std::collections::HashSet<_>>();
        let levels = hierarchy_levels(&items, &visible);
        let prerequisite_level = levels.get(&prerequisite_id);
        let dependent_level = levels.get(&dependent_id);
        assert!(matches!(
            (prerequisite_level, dependent_level),
            (Some(prerequisite), Some(dependent)) if dependent > prerequisite
        ));
    }

    #[test]
    fn cross_family_dependency_places_the_dependent_family_later() {
        let root = task("1", None, ItemState::Inbox);
        let prerequisite = task("2", Some("1"), ItemState::Inbox);
        let mut dependent = task("3", None, ItemState::Inbox);
        dependent.dependencies = vec![prerequisite.item.id];
        let first = root.item.id.to_string();
        let second = dependent.item.id.to_string();
        let children = std::collections::BTreeMap::from([(
            first.clone(),
            vec![prerequisite.item.id.to_string()],
        )]);
        let bands = family_dependency_bands(
            &[root, prerequisite, dependent],
            &[first.clone(), second.clone()],
            &children,
        );
        assert!(
            matches!((bands.get(&first), bands.get(&second)), (Some(before), Some(after)) if after > before)
        );
    }

    #[test]
    fn time_columns_follow_the_working_day() {
        let mut labels = vec!["later", "(undated)", "today", "overdue", "this week"];
        labels.sort_by(|left, right| compare_group_labels(GroupBy::Time, left, right));
        assert_eq!(
            labels,
            vec!["overdue", "today", "this week", "later", "(undated)"]
        );
    }

    #[test]
    fn priority_columns_put_critical_work_first() {
        let mut labels = vec!["low", "critical", "normal", "high"];
        labels.sort_by(|left, right| compare_group_labels(GroupBy::Priority, left, right));
        assert_eq!(labels, vec!["critical", "high", "normal", "low"]);
    }

    #[test]
    fn activity_reaches_every_ancestor() {
        let items = vec![
            task("1", None, ItemState::Inbox),
            task("2", Some("1"), ItemState::Inbox),
            task("3", Some("2"), ItemState::Active),
        ];
        assert!(holds(&items, "1"));
        assert!(holds(&items, "2"));
    }

    #[test]
    fn a_parent_of_finished_work_holds_nothing() {
        let items = vec![
            task("1", None, ItemState::Inbox),
            task("2", Some("1"), ItemState::Done),
        ];
        assert!(!holds(&items, "1"));
    }

    #[test]
    fn an_active_task_is_not_counted_as_holding_its_own_activity() {
        let items = vec![task("1", None, ItemState::Active)];
        assert!(!holds(&items, "1"));
    }

    #[test]
    fn a_parent_cycle_does_not_loop_when_deriving_activity() {
        let items = vec![
            task("1", Some("2"), ItemState::Inbox),
            task("2", Some("1"), ItemState::Active),
        ];
        let _ = super::ids_holding_active_work(&items);
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

    #[test]
    fn empty_query_matches_everything() {
        assert!(matches_query("   ", "anything", &[], &[]));
    }

    #[test]
    fn query_matches_title_id_and_tag_case_insensitively() {
        let ids = vec!["0a1b-2c3d".to_owned(), "lusab-babad".to_owned()];
        let tags = vec!["tag/work/cdint".to_owned()];
        assert!(matches_query("WRITE", "Write the report", &ids, &tags));
        assert!(matches_query("2c3d", "Write the report", &ids, &tags));
        assert!(matches_query("BABAD", "Write the report", &ids, &tags));
        assert!(matches_query("cdint", "Write the report", &ids, &tags));
        assert!(!matches_query("missing", "Write the report", &ids, &tags));
    }

    #[test]
    fn agent_tasks_are_marked_and_filtered() {
        let mine = task("1", None, ItemState::Inbox);
        let mut bot = task("2", None, ItemState::Inbox);
        bot.agent = Some("agent-assistant".to_owned());
        assert_eq!(node_meta(&mine, false), "inbox · normal");
        assert_eq!(node_meta(&bot, false), "inbox · normal · agent");
        let hide = AgentFilter::from_config(Some("hide"));
        assert!(hide.keeps(&mine) && !hide.keeps(&bot));
        let only = AgentFilter::from_config(Some("only"));
        assert!(!only.keeps(&mine) && only.keeps(&bot));
        assert_eq!(AgentFilter::from_config(None), AgentFilter::Show);
    }

    #[test]
    fn the_structure_band_shares_the_family_blue_and_bad_colors_fall_back() {
        use crate::theme::{BandColors, band_color};
        assert_eq!(
            band_color(&BandColors::default().structure),
            super::FAMILY_BLUE
        );
        assert_eq!(band_color("not a color"), Color32::GRAY);
    }

    #[test]
    fn time_values_read_as_spoken_dates_and_durations() {
        use super::{day_label, minutes_label};
        use chrono::NaiveDate;
        let today = NaiveDate::from_ymd_opt(2026, 10, 20).unwrap();
        let day = |m, d| NaiveDate::from_ymd_opt(2026, m, d).unwrap();
        assert_eq!(day_label(day(11, 1), today), "Nov 1 · in 12 days");
        assert_eq!(day_label(today, today), "Oct 20 · today");
        assert_eq!(day_label(day(10, 21), today), "Oct 21 · tomorrow");
        assert_eq!(day_label(day(10, 17), today), "Oct 17 · 3 days ago");
        let next_year = NaiveDate::from_ymd_opt(2027, 1, 2).unwrap();
        assert_eq!(day_label(next_year, today), "Jan 2 2027 · in 74 days");
        assert_eq!(minutes_label(45), "45 min");
        assert_eq!(minutes_label(120), "2 h");
        assert_eq!(minutes_label(90), "1 h 30 min");
    }

    /// An editor over paths that do not exist, so nothing is read or written.
    fn app(agent_filter: AgentFilter) -> TodoGui {
        let missing = std::env::temp_dir().join("todo-gui-test-missing");
        TodoGui::new(
            Runtime::new().unwrap(),
            LedgerAccess {
                path: missing.join("ledger.db"),
                actor_id: "test".to_owned(),
            },
            missing.join("layout.json"),
            missing.join("theme.toml"),
            agent_filter,
        )
    }

    #[test]
    fn each_changed_filter_is_one_chip_that_clears_itself() {
        let mut app = app(AgentFilter::Hide);
        assert_eq!(app.active_filters(), vec![FilterChip::Agent]);
        app.show_completed = true;
        app.tag_states.cycle("work");
        app.show_relations = false;
        app.show_timeline_ruler = false;
        app.search = "sync".to_owned();
        let chips = app.active_filters();
        assert_eq!(chips.len(), 6);
        for chip in &chips {
            app.clear_filter(chip);
        }
        assert!(app.active_filters().is_empty());
        app.show_completed = true;
        app.reset_filters();
        // Reset returns to the configured agent default, which still hides.
        assert_eq!(app.active_filters(), vec![FilterChip::Agent]);
    }
}
