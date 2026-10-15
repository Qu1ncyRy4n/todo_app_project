//! Configurable presentation knobs for the graph editor.
//!
//! Every field is a matter of taste rather than correctness, so each one has an
//! opinionated default and can be overridden without touching code. Values that
//! encode protocol or data meaning never belong here.

use crate::{
    geometry::{FlowDirection, TimelineAxis, TimelineScale, TreeAxis},
    palette::Colormap,
};
use anyhow::{Context, Result};
use eframe::egui::{Color32, ecolor::Hsva};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    fs,
    path::{Path, PathBuf},
};
use todo_core::WeightFactors;

/// Presentation settings for the whole graph editor.
#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Theme {
    pub weight: WeightTheme,
    #[serde(skip_serializing)]
    pub safety: SafetyTheme,
    pub inspector: InspectorTheme,
    pub focus: FocusTheme,
    pub timeline: TimelineTheme,
    pub parent: ParentTheme,
    pub layout: LayoutTheme,
    pub node: NodeTheme,
    pub tag: TagTheme,
    pub edge: EdgeTheme,
    pub colors: ColorsTheme,
}

impl Theme {
    /// Loads a theme from `path`, falling back to the defaults when absent.
    ///
    /// A missing file is normal and not an error, so a fresh install renders with
    /// the opinionated defaults. A malformed file is an error rather than a silent
    /// fallback, so a typo in a knob name is visible instead of ignored.
    pub fn load(path: &Path) -> Result<Self> {
        let contents = match fs::read_to_string(path) {
            Ok(contents) => contents,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self::default());
            }
            Err(error) => {
                return Err(error).with_context(|| format!("read {}", path.display()));
            }
        };
        toml::from_str(&contents).with_context(|| format!("parse {}", path.display()))
    }

    /// Renders the theme as TOML, for writing a starting point a developer can edit.
    pub fn to_toml(&self) -> Result<String> {
        toml::to_string_pretty(self).context("serialize theme")
    }
}

/// How Normalize arranges the graph.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NormalizeStyle {
    /// Hang each task's children beneath it, active work leading.
    #[default]
    Hierarchy,
    /// Layer the whole graph by longest path over parents and dependencies.
    Layered,
}

/// Controls how the Normalize command arranges the graph.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct LayoutTheme {
    /// How Normalize arranges the graph.
    pub style: NormalizeStyle,
    /// Which way a normalized hierarchy grows.
    ///
    /// Down by default: work reads from its head downward through what it holds,
    /// which is the direction time runs in the rest of the editor.
    pub tree_axis: TreeAxis,
    /// Distance between neighboring leaves, across the direction of growth.
    pub leaf_gap: f32,
    /// Vertical distance between a task and its children in a hierarchy.
    pub row_gap: f32,
    /// Vertical distance from active work up to the finished work it depended on.
    pub done_band_gap: f32,
    /// Whether finished prerequisites are lifted into a band above.
    pub done_above: bool,
    /// Whether hierarchy layout packs complete task families into rows.
    /// The legacy config key stays stable so existing theme files still load.
    pub compact_active: bool,
    /// Legacy config key for family spacing (one quarter is the visual gap).
    pub active_head_gap: f32,
    /// Horizontal distance between grouped columns.
    pub group_column_gap: f32,
    /// Vertical distance between members of a grouped column.
    pub group_row_gap: f32,
    /// Whether a normalized graph flows downward or rightward.
    pub direction: FlowDirection,
    /// Distance between one layer and the next.
    pub layer_gap: f32,
    /// Distance between neighbors inside a layer.
    pub node_gap: f32,
    /// Whether parent relationships order the layers.
    pub follow_parents: bool,
    /// Whether planned `depends_on` relationships order the layers.
    pub follow_dependencies: bool,
    /// How many relaxation passes the tag separation runs.
    pub separation_iterations: u8,
    /// How strongly separation draws a node toward its own tag group.
    pub separation_attraction: f32,
    /// How strongly separation pushes a node out of a foreign tag group.
    pub separation_repulsion: f32,
    /// How close a foreign tag group must be before separation pushes at all.
    pub separation_radius: f32,
    /// Whether normalize and focus layouts run the separation pass afterwards.
    pub separate_after_layout: bool,
    /// Whether nodes glide to new positions instead of jumping there.
    pub animate: bool,
    /// Time a node takes to cover a move, in milliseconds.
    pub transition_ms: f32,
    /// Whether layouts finish by pushing overlapping nodes apart.
    pub relax_overlaps_after_layout: bool,
    /// Clear space left between two node boxes when overlaps are relaxed.
    pub overlap_gap: f32,
    /// How many relaxation passes to run.
    pub overlap_iterations: u8,
    /// How much of an overlap is resolved per pass, from 0.0 to 1.0.
    pub overlap_strength: f32,
    /// Whether active `blocked_by` relationships order the layers.
    ///
    /// Off by default, because an active blocker is a subset of the planned
    /// dependency it came from and would only duplicate that edge.
    pub follow_blockers: bool,
}

impl Default for LayoutTheme {
    fn default() -> Self {
        Self {
            style: NormalizeStyle::default(),
            tree_axis: TreeAxis::default(),
            leaf_gap: 190.0,
            row_gap: 165.0,
            done_band_gap: 190.0,
            done_above: true,
            compact_active: true,
            active_head_gap: 240.0,
            group_column_gap: 280.0,
            group_row_gap: 96.0,
            direction: FlowDirection::default(),
            layer_gap: 120.0,
            node_gap: 185.0,
            separation_iterations: 14,
            separation_attraction: 0.10,
            separation_repulsion: 14.0,
            separation_radius: 420.0,
            separate_after_layout: true,
            animate: true,
            transition_ms: 420.0,
            relax_overlaps_after_layout: true,
            overlap_gap: 26.0,
            overlap_iterations: 60,
            overlap_strength: 0.65,
            follow_parents: true,
            follow_dependencies: true,
            follow_blockers: false,
        }
    }
}

/// Controls the banded Focus Window arrangement.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct FocusTheme {
    /// Distance from the active band to the completed and future bands.
    pub band_gap: f32,
    /// Distance between neighbors inside a band.
    pub node_gap: f32,
    /// Distance between wrapped rows of the same band.
    pub row_gap: f32,
    /// How many nodes a band holds before it wraps.
    pub band_width: usize,
    /// Whether a task holding active work reads as active itself.
    ///
    /// Derived when read, never stored: see TE-0013. A parent is drawn as active
    /// and joins the Focus Window while a descendant is active, and stops on its
    /// own when that descendant finishes.
    pub parents_of_active_read_active: bool,
    /// Whether the frame and its label are drawn around the active band.
    pub show_frame: bool,
    /// Whether tasks deferred or scheduled ahead of today join the future band.
    pub future_includes_deferred: bool,
    /// How far outside the active band non-active work is pushed, as a multiplier
    /// of the band gap.
    ///
    /// Large by default: focus is only focus if what is not active is out of the
    /// way rather than merely below.
    pub exile_multiplier: f32,
    /// Opacity of exiled work, out of 255, so context stays readable but quiet.
    pub exiled_alpha: u8,
    /// Whether arranging the Focus Window also frames it in the viewport.
    pub zoom_to_active: bool,
    /// Space left around the active band when framing it.
    pub zoom_margin: f32,
}

impl Default for FocusTheme {
    fn default() -> Self {
        Self {
            band_gap: 235.0,
            node_gap: 185.0,
            row_gap: 95.0,
            band_width: 4,
            parents_of_active_read_active: true,
            show_frame: true,
            future_includes_deferred: true,
            exile_multiplier: 3.2,
            exiled_alpha: 70,
            zoom_to_active: true,
            zoom_margin: 90.0,
        }
    }
}

/// What a visual channel on a node stands for.
///
/// Separating the channels from their meanings is what lets the same card say
/// different things: a border can carry the tag family while the bar carries
/// status, or the two can be swapped, without either being hardcoded.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Channel {
    /// Nothing; the channel is drawn neutral.
    None,
    /// Lifecycle: inbox, active, done, canceled, blocked, holding active work.
    #[default]
    Status,
    /// The task's own deepest tag.
    Tag,
    /// Priority, from low to critical along the ramp.
    Priority,
    /// Computed weight, from light to heavy along the ramp.
    Weight,
}

/// The outline a task node is drawn with.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeShape {
    /// A rectangle with rounded corners, holding a title and a status line.
    #[default]
    RoundedRect,
    /// A fully rounded rectangle, wider than it is tall.
    Capsule,
    /// A circle, for a more organic constellation reading.
    Circle,
}

/// Controls the size and outline of task nodes.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct NodeTheme {
    pub shape: NodeShape,
    /// Node width for rectangular shapes.
    pub width: f32,
    /// Node height for rectangular shapes.
    pub height: f32,
    /// Node radius for the circle shape.
    pub radius: f32,
    /// Corner rounding for the rounded rectangle shape.
    pub corner_radius: f32,
    /// Font size of the task title.
    pub title_font_size: f32,
    /// Font size of the state and priority line.
    pub meta_font_size: f32,
    /// Whether the state and priority line is drawn at all.
    pub show_meta_line: bool,
    /// What the accent bar on the leading edge stands for.
    pub bar_encodes: Channel,
    /// What the body fill stands for.
    ///
    /// A body carrying anything other than status is tinted down to
    /// `body_tint_value`, because a bright fill cannot sit behind readable text.
    pub body_encodes: Channel,
    /// What the border stands for.
    pub outline_encodes: Channel,
    /// Brightness a non-status body is tinted to, so text stays legible on it.
    pub body_tint_value: f32,
    /// Outline thickness of an unselected node.
    pub outline_width: f32,
    /// Reduce a card's redundant tag outline inside a same-color family well.
    pub soften_outline_in_family: bool,
    /// Omit that redundant outline entirely; selected cards still keep theirs.
    pub hide_outline_in_family: bool,
    /// Whether a long title wraps onto more lines instead of being cut short.
    pub wrap_titles: bool,
    /// Space between a node's edge and its text.
    pub text_padding: f32,
    /// Tallest a node may grow to fit its title, in points.
    ///
    /// A very long title stops growing the node and is cut short instead, so one
    /// runaway task cannot take over the canvas.
    pub max_height: f32,
}

impl Default for NodeTheme {
    fn default() -> Self {
        Self {
            shape: NodeShape::RoundedRect,
            width: 150.0,
            height: 58.0,
            radius: 34.0,
            corner_radius: 7.0,
            title_font_size: 14.0,
            meta_font_size: 11.0,
            show_meta_line: true,
            // The bar says where the work stands; the body and border say what it
            // belongs to. Swapping these is a theme edit, not a code change.
            bar_encodes: Channel::Status,
            body_encodes: Channel::Tag,
            outline_encodes: Channel::Tag,
            body_tint_value: 0.26,
            outline_width: 1.6,
            soften_outline_in_family: true,
            hide_outline_in_family: false,
            wrap_titles: true,
            text_padding: 9.0,
            max_height: 190.0,
        }
    }
}

impl NodeTheme {
    /// Returns the unzoomed size of a node for the configured shape.
    pub fn size(&self) -> (f32, f32) {
        match self.shape {
            NodeShape::Circle => (self.radius * 2.0, self.radius * 2.0),
            NodeShape::RoundedRect | NodeShape::Capsule => (self.width, self.height),
        }
    }

    /// Returns the corner rounding to draw for the configured shape.
    pub fn rounding(&self) -> f32 {
        match self.shape {
            NodeShape::RoundedRect => self.corner_radius,
            NodeShape::Capsule => self.height / 2.0,
            NodeShape::Circle => self.radius,
        }
    }
}

/// Window color mode, and an optional terminal theme to borrow colors from.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct ColorsTheme {
    /// `dark`, `light`, or `system`.
    ///
    /// Dark by default: cards are drawn for a dark canvas and read poorly on
    /// egui's light panels.
    pub mode: ColorMode,
    /// A Ghostty-format theme file whose background, foreground, selection, and
    /// cursor colors restyle the panels and canvas. Empty keeps egui's palette.
    ///
    /// A leading `~/` is the home directory, so a terminal theme or a matugen
    /// template output can be pointed at directly.
    pub palette_file: String,
}

impl Default for ColorsTheme {
    fn default() -> Self {
        Self {
            mode: ColorMode::Dark,
            palette_file: String::new(),
        }
    }
}

impl ColorsTheme {
    /// Resolves `palette_file`, or `None` when no palette is configured.
    pub fn palette_path(&self) -> Option<PathBuf> {
        let file = self.palette_file.trim();
        if file.is_empty() {
            return None;
        }
        match (file.strip_prefix("~/"), std::env::var_os("HOME")) {
            (Some(rest), Some(home)) => Some(PathBuf::from(home).join(rest)),
            _ => Some(PathBuf::from(file)),
        }
    }
}

/// Which egui visuals the window uses.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ColorMode {
    #[default]
    Dark,
    Light,
    /// Follow the operating system's appearance.
    System,
}

/// Colors borrowed from a terminal theme. Each is optional, since a theme file
/// may set only some of them.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TerminalPalette {
    pub background: Option<Color32>,
    pub foreground: Option<Color32>,
    pub selection: Option<Color32>,
    pub accent: Option<Color32>,
}

impl TerminalPalette {
    /// Reads Ghostty's `key = #rrggbb` lines.
    ///
    /// Unknown keys, comments, and the numbered `palette = N=#…` entries are
    /// ignored; the editor only needs the four surface colors.
    pub fn parse_ghostty(text: &str) -> Self {
        let mut palette = Self::default();
        for line in text.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let hex = value.trim().trim_start_matches('#');
            let color = Color32::from_hex(&format!("#{hex}")).ok();
            match key.trim() {
                "background" => palette.background = color,
                "foreground" => palette.foreground = color,
                "selection-background" => palette.selection = color,
                "cursor-color" => palette.accent = color,
                _ => {}
            }
        }
        palette
    }
}

/// Controls how relationship edges and their arrowheads are drawn.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct EdgeTheme {
    /// Line thickness of a relationship edge.
    pub width: f32,
    /// Length of an arrowhead, in points.
    pub arrowhead_length: f32,
    /// Width of an arrowhead across its base, in points.
    pub arrowhead_width: f32,
    /// Whether typed relation edges are drawn dashed rather than solid.
    ///
    /// Dashed by default, so a relation reads as a reference rather than as
    /// structure or a dependency.
    pub dash_relations: bool,
    /// Dash length used for typed relation edges.
    pub relation_dash_length: f32,
    /// Whether arrowheads grow and shrink with the graph zoom.
    ///
    /// Off by default: a head that keeps one size stays readable when the graph
    /// is zoomed out, which is when the arrows matter most.
    pub scale_arrowheads_with_zoom: bool,
}

impl Default for EdgeTheme {
    fn default() -> Self {
        Self {
            width: 2.0,
            arrowhead_length: 11.0,
            arrowhead_width: 9.0,
            dash_relations: true,
            relation_dash_length: 9.0,
            scale_arrowheads_with_zoom: false,
        }
    }
}

impl EdgeTheme {
    /// Returns the arrowhead length and width to draw at the given zoom.
    pub fn arrowhead_size(&self, zoom: f32) -> (f32, f32) {
        let scale = if self.scale_arrowheads_with_zoom {
            zoom
        } else {
            1.0
        };
        (self.arrowhead_length * scale, self.arrowhead_width * scale)
    }
}

/// Whether a task carries the tags of the tasks above it.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TagInheritance {
    /// A task carries only what was assigned to it.
    Off,
    /// A task carries its ancestors' tags, derived when read and never stored.
    ///
    /// Untagging a parent untags the subtree at once, and a task's own tags stay
    /// distinguishable from what it inherits.
    #[default]
    Implicit,
    /// Ancestors' tags are offered for assignment, and only applied when asked.
    ///
    /// The tags then belong to the task and outlive its parent, which is sometimes
    /// what is wanted and always a decision rather than a side effect.
    Explicit,
}

/// How tag paths are assigned hues.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TagColoring {
    /// Divide the hue wheel between the tag groups that are actually visible.
    #[default]
    Wheel,
    /// Hash each tag group to a fixed hue, independent of what else is visible.
    ///
    /// Colors stay put as tags come and go, at the cost of uneven spacing.
    GroupHash,
}

/// How members of one tag-cloud body choose connection partners.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TagCloudLinkMode {
    /// Each member connects to its nearest configured number of peers.
    #[default]
    Nearest,
    /// Every pair closer than `cloud_link_radius` connects.
    WithinRadius,
}

/// A hierarchy-aware color assignment over the tags currently visible.
///
/// The hue wheel is divided evenly between visible top-level groups in
/// alphabetical order. Descendants then divide a configurable portion of their
/// top-level group's hue span; sorted siblings receive stable offsets within that
/// related range, while depth can also adjust hue, saturation, and value. Colors are
/// derived only from the visible tag tree, never from item ids or random state,
/// so the same set of tags always paints the same way.
#[derive(Clone, Debug, Default)]
pub struct TagPalette {
    colors: HashMap<String, Color32>,
}

impl TagPalette {
    /// Builds a palette over every tag path that is currently visible.
    pub fn build<'a>(paths: impl IntoIterator<Item = &'a str>, theme: &TagTheme) -> Self {
        let mut children: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        let mut roots: BTreeSet<String> = BTreeSet::new();
        let mut terminal_counts = HashMap::<String, usize>::new();
        for path in paths {
            *terminal_counts.entry(path.to_owned()).or_default() += 1;
            let mut prefix = String::new();
            for segment in path.split('/') {
                if prefix.is_empty() {
                    prefix.push_str(segment);
                    roots.insert(prefix.clone());
                } else {
                    let parent = prefix.clone();
                    prefix.push('/');
                    prefix.push_str(segment);
                    children.entry(parent).or_default().insert(prefix.clone());
                }
            }
        }
        let mut colors = HashMap::new();
        let mut weights = HashMap::new();
        for root in &roots {
            branch_weight(root, &children, &terminal_counts, &mut weights);
        }
        let total_weight = roots
            .iter()
            .map(|root| weights.get(root).copied().unwrap_or(1))
            .sum::<usize>()
            .max(1);
        let total_weight = f32::from(u16::try_from(total_weight).unwrap_or(u16::MAX));
        let mut start = 0.0;
        for root in &roots {
            let weight = f32::from(
                u16::try_from(weights.get(root).copied().unwrap_or(1)).unwrap_or(u16::MAX),
            );
            let width = weight / total_weight;
            let hue = start + width / 2.0;
            assign_branch(
                root,
                hue,
                width,
                0,
                &PaletteTree {
                    children: &children,
                    weights: &weights,
                    theme,
                },
                &mut colors,
            );
            start += width;
        }
        Self { colors }
    }

    /// Returns the color for a tag path, falling back to the group hash mapping.
    ///
    /// A path outside the palette still gets a stable color rather than nothing,
    /// which keeps a newly added tag visible before the next rebuild.
    pub fn color(&self, path: &str, theme: &TagTheme) -> Color32 {
        self.colors
            .get(path)
            .copied()
            .unwrap_or_else(|| theme.hashed_color(path))
    }
}

/// Assigns one root hue, then divides a bounded portion of its hue span between
/// descendants. The span narrows by branch, preserving relationship without
/// making sibling differences imperceptible. Sorted paths make offsets stable.
struct PaletteTree<'a> {
    children: &'a BTreeMap<String, BTreeSet<String>>,
    weights: &'a HashMap<String, usize>,
    theme: &'a TagTheme,
}

fn assign_branch(
    path: &str,
    hue: f32,
    hue_span: f32,
    depth: usize,
    tree: &PaletteTree<'_>,
    colors: &mut HashMap<String, Color32>,
) {
    colors.insert(path.to_owned(), tree.theme.shade(hue, depth));
    let Some(next) = tree.children.get(path) else {
        return;
    };
    let total_weight = next
        .iter()
        .map(|child| tree.weights.get(child).copied().unwrap_or(1))
        .sum::<usize>()
        .max(1);
    let total_weight = f32::from(u16::try_from(total_weight).unwrap_or(u16::MAX));
    let used_span = hue_span * tree.theme.branch_hue_spread;
    let mut start = hue - used_span / 2.0;
    for child in next {
        let weight = f32::from(
            u16::try_from(tree.weights.get(child).copied().unwrap_or(1)).unwrap_or(u16::MAX),
        );
        let child_span = used_span * weight / total_weight;
        let child_hue = start + child_span / 2.0 + hue_span * tree.theme.depth_hue_drift;
        assign_branch(child, child_hue, child_span, depth + 1, tree, colors);
        start += child_span;
    }
}

/// Counts visible terminal tag assignments beneath a path. The palette uses this
/// as a visual population weight, letting dense visible branches occupy more hue
/// span than a lone sibling while preserving deterministic path ordering.
fn branch_weight(
    path: &str,
    children: &BTreeMap<String, BTreeSet<String>>,
    terminal_counts: &HashMap<String, usize>,
    weights: &mut HashMap<String, usize>,
) -> usize {
    if let Some(weight) = weights.get(path) {
        return *weight;
    }
    let own = terminal_counts.get(path).copied().unwrap_or(0);
    let descendants = children
        .get(path)
        .map(|next| {
            next.iter()
                .map(|child| branch_weight(child, children, terminal_counts, weights))
                .sum::<usize>()
        })
        .unwrap_or(0);
    let weight = own.saturating_add(descendants).max(1);
    weights.insert(path.to_owned(), weight);
    weight
}

/// Controls how task weight is computed and shown.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct WeightTheme {
    /// Whether node size follows the computed weight.
    pub size_nodes_by_weight: bool,
    /// Size multiplier applied to the lightest task.
    pub min_scale: f32,
    /// Size multiplier applied to the heaviest task.
    pub max_scale: f32,
    /// How each input contributes to the weight.
    pub factors: WeightFactors,
}

impl Default for WeightTheme {
    fn default() -> Self {
        Self {
            size_nodes_by_weight: false,
            min_scale: 0.85,
            max_scale: 1.45,
            factors: WeightFactors::default(),
        }
    }
}

impl WeightTheme {
    /// Returns the size multiplier for a weight between 0.0 and 1.0.
    pub fn scale(&self, weight: f32) -> f32 {
        if !self.size_nodes_by_weight {
            return 1.0;
        }
        let span = self.max_scale - self.min_scale;
        self.min_scale + weight.clamp(0.0, 1.0) * span
    }
}

/// Retained so existing theme files keep loading after deletion left the editor.
///
/// The graph editor no longer deletes tasks; `todo rm` and `todo restore` do that
/// from the terminal, where a delete is a deliberate, typed act. The section is
/// accepted but no longer written back out.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct SafetyTheme {
    pub confirm_delete: bool,
    pub show_deleted_list: bool,
}

impl Default for SafetyTheme {
    fn default() -> Self {
        Self {
            confirm_delete: true,
            show_deleted_list: true,
        }
    }
}

/// Controls which inspector sections are shown.
///
/// Hiding a section only stops it being drawn. Nothing about the task changes, so
/// a hidden relationship still exists and still draws in the graph.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct InspectorTheme {
    /// Whether each group starts expanded.
    pub open_status: bool,
    pub open_relationships: bool,
    pub open_details: bool,
    pub open_schedule: bool,
    /// Advanced starts closed: raw editing is not an everyday control.
    pub open_advanced: bool,
    pub show_parent: bool,
    pub show_dependencies: bool,
    pub show_blockers: bool,
    pub show_relations: bool,
    pub show_focus_sequence: bool,
    pub show_dates: bool,
    pub show_sessions: bool,
    pub show_notes: bool,
    pub show_links: bool,
    pub show_raw_json: bool,
}

impl Default for InspectorTheme {
    fn default() -> Self {
        Self {
            open_status: true,
            open_relationships: true,
            open_details: true,
            open_schedule: false,
            open_advanced: false,
            show_parent: true,
            show_dependencies: true,
            show_blockers: true,
            show_relations: true,
            show_focus_sequence: true,
            show_dates: true,
            show_sessions: true,
            show_notes: true,
            show_links: true,
            show_raw_json: true,
        }
    }
}

/// Controls the scheduled-date timeline.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct TimelineTheme {
    /// Which way the timeline runs.
    ///
    /// Left to right by default, which is the shape of a calendar; top down reads
    /// as a log and suits a long sparse schedule.
    pub axis: TimelineAxis,
    /// Whether days are spaced by elapsed time or by sequence.
    pub scale: TimelineScale,
    /// Whether a task's estimate is drawn as a block along the time axis.
    pub show_durations: bool,
    /// Hours of work counted as a full day, when drawing durations.
    pub hours_per_day: f32,
    /// Distance between one day and the next along the time axis.
    pub day_width: f32,
    /// Vertical distance between tasks stacked on the same day.
    pub lane_height: f32,
    /// Whether the day ruler and today marker are drawn.
    pub show_ruler: bool,
    /// How many days of ruler to draw either side of the scheduled range.
    pub ruler_margin_days: u16,
    /// Distance from the top of the canvas to the day axis.
    ///
    /// The ruler is pinned to the viewport rather than carried by the graph
    /// transform, so panning the graph can never take the axis off screen.
    pub ruler_top_offset: f32,
    /// Whether arranging a timeline also resets the pan, framing what it built.
    pub reset_view: bool,
    /// Whether unscheduled tasks are parked in a lane of their own.
    ///
    /// Scheduled dates alone drive the timeline: due is a commitment and defer
    /// controls readiness, so neither is read here.
    pub park_unscheduled: bool,
}

impl Default for TimelineTheme {
    fn default() -> Self {
        Self {
            axis: TimelineAxis::default(),
            scale: TimelineScale::default(),
            show_durations: true,
            hours_per_day: 8.0,
            day_width: 120.0,
            lane_height: 82.0,
            show_ruler: true,
            ruler_margin_days: 2,
            ruler_top_offset: 34.0,
            reset_view: true,
            park_unscheduled: true,
        }
    }
}

/// How a parent's containment region is drawn.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ParentRegionStyle {
    /// A titled rounded box, so a parent reads as a container holding a subgraph.
    #[default]
    Box,
    /// A smoothed hull, matching how tag regions are drawn.
    Hull,
}

/// How the parent task is attached to the family enclosure.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FamilyHeaderStyle {
    /// A title strip sits inside the enclosing family frame.
    Header,
    /// A compact tab projects from the frame, keeping the parent identity above
    /// the child space without consuming room inside it.
    #[default]
    AttachedTab,
}

/// How the visible descendants are presented inside a parent family.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FamilyChildStyle {
    /// A softly filled inset board, useful when the children should feel recessed.
    RecessedBoard,
    /// A strong nested frame, making the child graph read as a contained family.
    #[default]
    NestedFamily,
}

/// Whether the child space covers the graph behind it.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FamilyWellFill {
    /// Leave the graph and tag clouds visible inside the family frame.
    #[default]
    Transparent,
    /// Use the dark inset from the original family study.
    Dark,
}

/// How much framing every family gets, regardless of its own geometry.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FamilyView {
    /// Contain children that sit nearby, and fall back to the family tree when
    /// the frame would be too large or detached from its parent.
    #[default]
    Adaptive,
    /// Frame every family, even if the full child span is wide.
    Boxed,
    /// Draw no frames at all. Families read only through their links, which
    /// suits a dense graph where boxes would tile over each other.
    Connected,
}

/// How a family connector carries both its relationship and its family.
///
/// A connector is drawn twice: a wider casing, then a narrower core over it.
/// One of the two is the shared family blue, which keeps every family link
/// recognizable as a link; the other is the family's own fill, which says which
/// family it belongs to. Which one goes on top is a legibility trade, because a
/// dark fill reads better as a core against blue than as a casing against a
/// pale canvas.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectorStyle {
    /// Blue casing around a family-colored core.
    #[default]
    FillCore,
    /// Family-colored casing around a blue core.
    BlueCore,
}

/// Controls the region drawn around a parent task and its descendants.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct ParentTheme {
    /// Whether parent containment regions are drawn at all.
    pub show_regions: bool,
    /// How much framing families get, independent of their own geometry.
    pub family_view: FamilyView,
    /// Which of the two connector strokes carries the family's own color.
    pub connector_style: ConnectorStyle,
    /// Whether a parent draws as a titled box or as a hull.
    pub region_style: ParentRegionStyle,
    /// How the parent identity is attached to its family frame.
    pub header_style: FamilyHeaderStyle,
    /// How the descendant compartment is rendered inside that frame.
    pub child_style: FamilyChildStyle,
    /// Visible generations below a root; zero means all generations.
    pub child_depth: usize,
    /// Direct children required before Dynamic draws a family box.
    pub dynamic_min_children: usize,
    /// Center-to-center distance that joins direct children into one Dynamic box.
    pub dynamic_cluster_distance: f32,
    /// Maximum card-to-rim gap for a Dynamic box to use a direct parent join.
    pub parent_join_max_gap: f32,
    /// Width of a direct parent join, as a fraction of parent-card width.
    pub parent_join_width_ratio: f32,
    /// Compatibility key retained so existing themes load. Dynamic grouping
    /// uses `dynamic_min_children` and `dynamic_cluster_distance` instead.
    pub max_board_span: f32,
    /// Whether the child space is open or filled with a dark inset.
    pub well_fill: FamilyWellFill,
    /// Keep an attached parent's card area out of its child-space inset.
    pub clip_well_below_attached_tab: bool,
    /// Height of the title strip on a box region.
    pub header_height: f32,
    /// Extra padding added per level of nesting, so nested boxes do not touch.
    pub nest_padding: f32,
    /// Corner rounding of a box region.
    pub corner_radius: f32,
    /// How many visible descendants a parent needs before it gets a region.
    pub min_descendants: usize,
    /// Space left between a region's edge and its member nodes.
    pub padding: f32,
    /// Chaikin rounding passes applied to the region.
    pub smoothing: u8,
    /// Fill opacity of a hull region, out of 255. Box families use opaque fills
    /// so overlapping generations keep one color.
    pub fill_alpha: u8,
    /// Outline opacity of a hull region, out of 255.
    pub stroke_alpha: u8,
    /// Outline thickness of a parent region.
    pub stroke_width: f32,
    /// Multiplier that makes the structural parent outline stronger than an
    /// ordinary card outline without making child-well seams equally heavy.
    pub outline_weight: f32,
}

impl Default for ParentTheme {
    fn default() -> Self {
        Self {
            show_regions: true,
            family_view: FamilyView::Adaptive,
            connector_style: ConnectorStyle::FillCore,
            region_style: ParentRegionStyle::Box,
            header_style: FamilyHeaderStyle::AttachedTab,
            child_style: FamilyChildStyle::NestedFamily,
            child_depth: 0,
            dynamic_min_children: 3,
            dynamic_cluster_distance: 300.0,
            parent_join_max_gap: 30.0,
            parent_join_width_ratio: 0.30,
            max_board_span: 3.0,
            well_fill: FamilyWellFill::Dark,
            clip_well_below_attached_tab: true,
            header_height: 18.0,
            nest_padding: 14.0,
            corner_radius: 10.0,
            min_descendants: 1,
            padding: 12.0,
            smoothing: 2,
            fill_alpha: 14,
            stroke_alpha: 90,
            stroke_width: 1.5,
            outline_weight: 2.75,
        }
    }
}

/// The outline a tag region is drawn with.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RegionShape {
    /// A rounded rectangle around every member node.
    Rect,
    /// A smoothed hull that follows the members. Kept for themes that prefer a
    /// single enclosing region, even though it fills concave gaps.
    Hull,
    /// Overlapping circular islands joined by a faint thread. The gaps between
    /// islands remain visible, so unrelated work does not read as tagged.
    #[default]
    Cloud,
}

/// The silhouette used for the per-card members of a cloud tag region.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CloudMemberShape {
    /// Soft circular islands, useful when tags should feel organic.
    #[default]
    Circle,
    /// Rounded card-sized islands joined by rails, preserving task-card geometry.
    Card,
}

/// Controls how tag paths turn into colors and how tag regions are drawn.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct TagTheme {
    /// Saturation applied to a top-level tag group.
    pub base_saturation: f32,
    /// Saturation added for each level of tag depth.
    pub depth_saturation_step: f32,
    /// Brightness applied to a top-level tag group.
    pub base_value: f32,
    /// Brightness removed for each level of tag depth.
    pub depth_value_step: f32,
    /// Floor for brightness, so deep tags stay visible.
    pub min_value: f32,
    /// Ceiling for saturation, so deep tags do not turn garish.
    pub max_saturation: f32,
    /// Fraction of a top-level group's hue span allocated to sorted sibling branches.
    pub branch_hue_spread: f32,
    /// Additional hue drift applied once per hierarchy level, relative to that branch's span.
    pub depth_hue_drift: f32,
    /// Fill opacity of a tag region, out of 255.
    pub region_fill_alpha: u8,
    /// Outline opacity of a tag region, out of 255.
    pub region_stroke_alpha: u8,
    /// Width of the tag accent stripe drawn on a node's leading edge.
    pub node_stripe_width: f32,
    /// Whether a region is drawn as a rectangle, a hull, or separate cloud islands.
    pub region_shape: RegionShape,
    /// The member silhouette used when `region_shape` is `cloud`.
    pub cloud_member_shape: CloudMemberShape,
    /// Space left between a region's edge and its member nodes.
    pub region_padding: f32,
    /// Chaikin rounding passes applied to a hull region.
    pub region_smoothing: u8,
    /// How tag hues are assigned.
    pub coloring: TagColoring,
    /// Whether a task carries the tags of the tasks above it.
    pub inherit: TagInheritance,
    /// Which color ramp tag hues are drawn from.
    pub colormap: Colormap,
    /// Overall brightness of tag colors, as a multiplier.
    ///
    /// Above one pushes the ramp toward its most saturated reading, which is what
    /// makes tags carry across a dark canvas.
    pub brightness: f32,
    /// How far apart two tagged tasks can sit and still share one region.
    ///
    /// Without a limit, one tag draws a single region spanning everything it
    /// touches, which swallows unrelated work sitting in between.
    pub cluster_distance: f32,
    /// How many tasks a cluster needs before it is drawn as a region.
    pub min_region_members: usize,
    /// Zero shows all tag hierarchy levels; a positive value limits cloud depth.
    pub cloud_hierarchy_depth: usize,
    /// How same-path cloud members choose their neighbors.
    pub cloud_link_mode: TagCloudLinkMode,
    /// Nearest peers per member when `cloud_link_mode = "nearest"`; zero draws no rails.
    pub cloud_neighbor_count: usize,
    /// Pair distance when `cloud_link_mode = "within_radius"`.
    pub cloud_link_radius: f32,
    /// Fill the local convex gaps between three or more same-path cloud members.
    pub cloud_fill_gaps: bool,
    /// Carve unrelated cards out of inherited hierarchy clouds.
    pub cloud_cut_out_unrelated: bool,
    /// Stroke the cut-out boundary around unrelated cards.
    pub cloud_cutout_outline: bool,
    /// Whether the full tag path is drawn, rather than the last segment.
    pub label_full_path: bool,
    /// Font size of a region label.
    pub label_font_size: f32,
    /// Narrowest region, in points, that still gets a label.
    pub label_min_width: f32,
    /// Percent of the tag color in a cloud fill over a light canvas; the rest is
    /// the canvas color.
    pub cloud_mix: u8,
    /// The same percent over a dark canvas, where a faint mix all but vanishes.
    pub cloud_mix_dark: u8,
}

impl Default for TagTheme {
    fn default() -> Self {
        Self {
            base_saturation: 0.45,
            depth_saturation_step: 0.08,
            base_value: 0.72,
            depth_value_step: 0.13,
            min_value: 0.22,
            max_saturation: 0.85,
            branch_hue_spread: 0.82,
            depth_hue_drift: 0.04,
            region_fill_alpha: 26,
            region_stroke_alpha: 110,
            node_stripe_width: 7.0,
            region_shape: RegionShape::Hull,
            cloud_member_shape: CloudMemberShape::Circle,
            region_padding: 24.0,
            region_smoothing: 2,
            coloring: TagColoring::default(),
            inherit: TagInheritance::default(),
            colormap: Colormap::default(),
            brightness: 1.15,
            cluster_distance: 420.0,
            min_region_members: 2,
            cloud_hierarchy_depth: 0,
            cloud_link_mode: TagCloudLinkMode::Nearest,
            cloud_neighbor_count: 5,
            cloud_link_radius: 280.0,
            cloud_fill_gaps: true,
            cloud_cut_out_unrelated: true,
            cloud_cutout_outline: false,
            label_full_path: false,
            label_font_size: 11.0,
            label_min_width: 70.0,
            cloud_mix: 16,
            cloud_mix_dark: 40,
        }
    }
}

impl TagTheme {
    /// Returns the cloud mix for a canvas color, choosing by its luminance.
    pub fn cloud_mix_over(&self, backdrop: Color32) -> u8 {
        let luma = u32::from(backdrop.r()) * 299
            + u32::from(backdrop.g()) * 587
            + u32::from(backdrop.b()) * 114;
        if luma < 128_000 {
            self.cloud_mix_dark
        } else {
            self.cloud_mix
        }
    }

    /// Colors a tag path from its top-level group hue, darkened once per depth.
    ///
    /// A child tag is a deeper shade of its parent: `work` and `work/cdint` share
    /// one hue and differ only by depth. The mapping is a pure function of the
    /// path, never of an item id or random state, so it is stable across runs.
    /// This is the fallback used when no visible tag tree is available.
    pub fn hashed_color(&self, path: &str) -> Color32 {
        let group = path.split('/').next().unwrap_or(path);
        let hash = group.bytes().fold(0_u32, |hash, byte| {
            hash.wrapping_mul(31).wrapping_add(u32::from(byte))
        });
        let hue = f32::from(u16::try_from(hash % 360).unwrap_or(0)) / 360.0;
        self.shade(hue, path.matches('/').count())
    }

    /// Returns the color for a position on the wheel at a given tag depth.
    ///
    /// The ramp supplies the hue; depth then darkens and saturates it, so nesting
    /// reads as depth whichever ramp is in use.
    pub fn shade(&self, position: f32, depth: usize) -> Color32 {
        let depth = f32::from(u8::try_from(depth).unwrap_or(u8::MAX));
        let wrapped = position.rem_euclid(1.0);
        let base = match self.colormap {
            Colormap::Rainbow => Hsva::from(crate::palette::rainbow(wrapped)),
            Colormap::Hue => Hsva::new(wrapped, self.base_saturation, self.base_value, 1.0),
        };
        let saturation = (base.s.max(self.base_saturation) + self.depth_saturation_step * depth)
            .min(self.max_saturation);
        let value = ((base.v * self.brightness).min(1.0) - self.depth_value_step * depth)
            .max(self.min_value);
        Color32::from(Hsva::new(base.h, saturation, value, 1.0))
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CloudMemberShape, ColorMode, EdgeTheme, FamilyWellFill, NodeShape, NodeTheme,
        TagCloudLinkMode, TagColoring, TagPalette, TagTheme, TerminalPalette, Theme, branch_weight,
    };
    use eframe::egui::Color32;
    use std::{
        collections::{BTreeMap, BTreeSet, HashMap},
        path::Path,
    };

    #[test]
    fn a_ghostty_theme_supplies_the_surface_colors() {
        let palette = TerminalPalette::parse_ghostty(
            "# comment\nbackground = #1a2f1f\nforeground = c8d4aa\n\
             selection-background = #7ba428\ncursor-color = #9bc53d\npalette = 0=#3d5c43\n",
        );
        assert_eq!(
            palette.background,
            Some(Color32::from_rgb(0x1a, 0x2f, 0x1f))
        );
        assert_eq!(
            palette.foreground,
            Some(Color32::from_rgb(0xc8, 0xd4, 0xaa))
        );
        assert_eq!(palette.selection, Some(Color32::from_rgb(0x7b, 0xa4, 0x28)));
        assert_eq!(palette.accent, Some(Color32::from_rgb(0x9b, 0xc5, 0x3d)));
    }

    #[test]
    fn clouds_mix_stronger_over_a_dark_canvas() {
        let tag = TagTheme::default();
        assert_eq!(tag.cloud_mix_over(Color32::from_gray(248)), tag.cloud_mix);
        assert_eq!(
            tag.cloud_mix_over(Color32::from_rgb(0x1a, 0x2f, 0x1f)),
            tag.cloud_mix_dark
        );
    }

    #[test]
    fn colors_default_to_dark_without_a_palette() {
        let parsed: Result<Theme, _> = toml::from_str("[colors]\nmode = \"light\"\n");
        assert!(matches!(parsed, Ok(theme) if theme.colors.mode == ColorMode::Light));
        let defaults = Theme::default().colors;
        assert_eq!(defaults.mode, ColorMode::Dark);
        assert_eq!(defaults.palette_path(), None);
    }

    #[test]
    fn the_example_theme_files_parse() {
        for text in [
            include_str!("../../../docs/themes/semantic-body.toml"),
            include_str!("../../../docs/themes/status-first.toml"),
        ] {
            let parsed: Result<Theme, _> = toml::from_str(text);
            assert!(parsed.is_ok(), "example theme failed to parse: {parsed:?}");
        }
    }

    #[test]
    fn a_missing_theme_file_yields_the_defaults() {
        let theme = Theme::load(Path::new("/nonexistent/todo-ledger/theme.toml"));
        assert!(matches!(theme, Ok(theme) if theme == Theme::default()));
    }

    #[test]
    fn all_child_generations_are_visible_by_default() {
        assert_eq!(Theme::default().parent.child_depth, 0);
        let parsed: Result<Theme, _> = toml::from_str("[parent]\nshow_regions = true\n");
        assert!(matches!(parsed, Ok(theme) if theme.parent.child_depth == 0));
    }

    #[test]
    fn a_theme_round_trips_through_toml() {
        let theme = Theme::default();
        let restored = theme
            .to_toml()
            .and_then(|text| toml::from_str::<Theme>(&text).map_err(anyhow::Error::from));
        assert!(matches!(restored, Ok(restored) if restored == theme));
    }

    #[test]
    fn family_child_space_fill_is_configurable() {
        let parsed: Result<Theme, _> = toml::from_str("[parent]\nwell_fill = \"dark\"\n");
        assert!(matches!(parsed, Ok(theme) if theme.parent.well_fill == FamilyWellFill::Dark));
        assert_eq!(Theme::default().parent.well_fill, FamilyWellFill::Dark);
    }

    #[test]
    fn dynamic_family_thresholds_are_configurable_with_the_reviewed_defaults() {
        let defaults = Theme::default();
        assert_eq!(defaults.parent.dynamic_min_children, 3);
        assert_eq!(defaults.parent.dynamic_cluster_distance, 300.0);
        assert_eq!(defaults.parent.parent_join_max_gap, 30.0);
        assert_eq!(defaults.parent.parent_join_width_ratio, 0.30);

        let parsed: Result<Theme, _> = toml::from_str(
            "[parent]\ndynamic_min_children = 4\ndynamic_cluster_distance = 420.0\nparent_join_max_gap = 48.0\nparent_join_width_ratio = 0.5\n",
        );
        assert!(matches!(parsed, Ok(theme)
            if theme.parent.dynamic_min_children == 4
                && theme.parent.dynamic_cluster_distance == 420.0
                && theme.parent.parent_join_max_gap == 48.0
                && theme.parent.parent_join_width_ratio == 0.5));
    }

    #[test]
    fn hierarchical_cloud_controls_are_persisted_with_the_reviewed_defaults() {
        let defaults = Theme::default();
        assert_eq!(defaults.tag.cloud_hierarchy_depth, 0);
        assert_eq!(defaults.tag.cloud_link_mode, TagCloudLinkMode::Nearest);
        assert_eq!(defaults.tag.cloud_neighbor_count, 5);
        assert!(defaults.tag.cloud_fill_gaps);
        assert!(defaults.tag.cloud_cut_out_unrelated);
        assert!(!defaults.tag.cloud_cutout_outline);

        let parsed: Result<Theme, _> = toml::from_str(
            "[tag]\ncloud_hierarchy_depth = 3\ncloud_link_mode = \"within_radius\"\ncloud_link_radius = 340.0\ncloud_fill_gaps = false\ncloud_cut_out_unrelated = false\ncloud_cutout_outline = true\nbranch_hue_spread = 0.08\ndepth_hue_drift = -0.01\n",
        );
        assert!(matches!(parsed, Ok(theme)
            if theme.tag.cloud_hierarchy_depth == 3
                && theme.tag.cloud_link_mode == TagCloudLinkMode::WithinRadius
                && theme.tag.cloud_link_radius == 340.0
                && !theme.tag.cloud_fill_gaps
                && !theme.tag.cloud_cut_out_unrelated
                && theme.tag.cloud_cutout_outline
                && theme.tag.branch_hue_spread == 0.08
                && theme.tag.depth_hue_drift == -0.01));
    }

    #[test]
    fn card_shaped_tag_clouds_are_configurable() {
        let parsed: Result<Theme, _> =
            toml::from_str("[tag]\nregion_shape = \"cloud\"\ncloud_member_shape = \"card\"\n");
        assert!(
            matches!(parsed, Ok(theme) if theme.tag.cloud_member_shape == CloudMemberShape::Card)
        );
    }

    #[test]
    fn attached_tabs_can_clip_the_child_inset() {
        let parsed: Result<Theme, _> =
            toml::from_str("[parent]\nclip_well_below_attached_tab = false\n");
        assert!(matches!(parsed, Ok(theme) if !theme.parent.clip_well_below_attached_tab));
    }

    #[test]
    fn parent_outline_weight_is_configurable_and_structural_by_default() {
        let parsed: Result<Theme, _> = toml::from_str("[parent]\noutline_weight = 4.0\n");
        assert!(matches!(parsed, Ok(theme) if theme.parent.outline_weight == 4.0));
        assert!(Theme::default().parent.outline_weight > 1.0);
    }

    #[test]
    fn family_outline_can_be_hidden_without_affecting_other_cards() {
        let parsed: Result<Theme, _> = toml::from_str("[node]\nhide_outline_in_family = true\n");
        assert!(matches!(parsed, Ok(theme) if theme.node.hide_outline_in_family));
    }

    #[test]
    fn a_partial_theme_file_keeps_the_other_defaults() {
        let parsed: Result<Theme, _> = toml::from_str("[node]\nshape = \"circle\"\n");
        assert!(matches!(
            parsed,
            Ok(theme) if theme.node.shape == NodeShape::Circle
                && theme.edge == EdgeTheme::default()
        ));
    }

    #[test]
    fn an_unknown_knob_is_rejected_rather_than_ignored() {
        let parsed: Result<Theme, _> = toml::from_str("[node]\nwidht = 10.0\n");
        assert!(parsed.is_err());
    }

    #[test]
    fn a_capsule_rounds_to_half_its_height() {
        let theme = NodeTheme {
            shape: NodeShape::Capsule,
            ..NodeTheme::default()
        };
        assert!((theme.rounding() - theme.height / 2.0).abs() < 0.001);
    }

    #[test]
    fn a_circle_is_as_wide_as_it_is_tall() {
        let theme = NodeTheme {
            shape: NodeShape::Circle,
            ..NodeTheme::default()
        };
        let (width, height) = theme.size();
        assert!((width - height).abs() < 0.001);
    }

    #[test]
    fn an_empty_palette_falls_back_to_the_hash_mapping() {
        let theme = TagTheme::default();
        let palette = TagPalette::build(std::iter::empty(), &theme);
        assert_eq!(palette.color("work", &theme), theme.hashed_color("work"));
    }

    #[test]
    fn a_single_group_still_receives_a_color() {
        let theme = TagTheme::default();
        let palette = TagPalette::build(["work"], &theme);
        assert_ne!(palette.color("work", &theme), Color32::TRANSPARENT);
    }

    #[test]
    fn separate_groups_take_separate_sectors() {
        let theme = TagTheme::default();
        let palette = TagPalette::build(["work", "home", "play"], &theme);
        let work = palette.color("work", &theme);
        let home = palette.color("home", &theme);
        let play = palette.color("play", &theme);
        assert_ne!(work, home);
        assert_ne!(home, play);
        assert_ne!(work, play);
    }

    #[test]
    fn siblings_drift_further_from_the_parent_in_order() {
        let theme = TagTheme::default();
        let palette = TagPalette::build(["work/a", "work/b", "work/c"], &theme);
        let first = palette.color("work/a", &theme);
        let second = palette.color("work/b", &theme);
        let third = palette.color("work/c", &theme);
        assert_ne!(first, second);
        assert_ne!(second, third);
    }

    #[test]
    fn deep_sibling_leaves_remain_distinguishable() {
        let theme = TagTheme::default();
        let palette = TagPalette::build(
            [
                "work/cdint/api/transport/codec",
                "work/cdint/api/transport/claude",
            ],
            &theme,
        );
        assert_ne!(
            palette.color("work/cdint/api/transport/codec", &theme),
            palette.color("work/cdint/api/transport/claude", &theme),
        );
    }

    #[test]
    fn visible_terminal_assignments_weight_a_branchs_hue_budget() {
        let children = BTreeMap::from([(
            "work".to_owned(),
            BTreeSet::from(["work/dense".to_owned(), "work/sparse".to_owned()]),
        )]);
        let terminal_counts = HashMap::from([
            ("work/dense".to_owned(), 5_usize),
            ("work/sparse".to_owned(), 1_usize),
        ]);
        let mut weights = HashMap::new();
        assert_eq!(
            branch_weight("work", &children, &terminal_counts, &mut weights),
            6
        );
        let dense = weights.get("work/dense").copied();
        let sparse = weights.get("work/sparse").copied();
        assert!(matches!((dense, sparse), (Some(dense), Some(sparse)) if dense > sparse));
    }

    #[test]
    fn a_child_is_darker_than_its_parent_under_the_wheel() {
        let theme = TagTheme::default();
        let palette = TagPalette::build(["work/cdint"], &theme);
        let parent = palette.color("work", &theme);
        let child = palette.color("work/cdint", &theme);
        let brightness =
            |color: Color32| u32::from(color.r()) + u32::from(color.g()) + u32::from(color.b());
        assert!(brightness(child) < brightness(parent));
    }

    #[test]
    fn the_palette_is_deterministic_for_the_same_tags() {
        let theme = TagTheme::default();
        let first = TagPalette::build(["work/a", "home"], &theme);
        let second = TagPalette::build(["home", "work/a"], &theme);
        assert_eq!(
            first.color("work/a", &theme),
            second.color("work/a", &theme)
        );
    }

    #[test]
    fn the_coloring_mode_can_be_configured() {
        let parsed: Result<Theme, _> = toml::from_str("[tag]\ncoloring = \"group_hash\"\n");
        assert!(matches!(parsed, Ok(theme) if theme.tag.coloring == TagColoring::GroupHash));
    }

    #[test]
    fn weight_scaling_is_off_until_asked_for() {
        let theme = super::WeightTheme::default();
        assert!((theme.scale(1.0) - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn a_heavier_task_scales_larger_when_enabled() {
        let theme = super::WeightTheme {
            size_nodes_by_weight: true,
            ..super::WeightTheme::default()
        };
        assert!(theme.scale(1.0) > theme.scale(0.0));
    }

    #[test]
    fn arrowheads_ignore_zoom_by_default() {
        let theme = EdgeTheme::default();
        assert_eq!(theme.arrowhead_size(0.4), theme.arrowhead_size(2.5));
    }

    #[test]
    fn arrowheads_follow_zoom_when_configured() {
        let theme = EdgeTheme {
            scale_arrowheads_with_zoom: true,
            ..EdgeTheme::default()
        };
        let (small, _) = theme.arrowhead_size(1.0);
        let (large, _) = theme.arrowhead_size(2.0);
        assert!(large > small);
    }

    #[test]
    fn a_child_tag_is_a_deeper_shade_of_its_parent() {
        let theme = TagTheme::default();
        let parent = theme.hashed_color("work");
        let child = theme.hashed_color("work/cdint");
        let grandchild = theme.hashed_color("work/cdint/ingest");
        assert!(child.g() < parent.g() || child.r() < parent.r() || child.b() < parent.b());
        assert!(grandchild.g() <= child.g());
    }

    #[test]
    fn separate_groups_receive_separate_colors() {
        let theme = TagTheme::default();
        assert_ne!(theme.hashed_color("work"), theme.hashed_color("home"));
    }

    #[test]
    fn a_child_keeps_the_hue_of_its_group() {
        let theme = TagTheme::default();
        let shallow = theme.hashed_color("work/cdint");
        let deep = theme.hashed_color("work/cdint/ingest/api");
        let other = theme.hashed_color("home/cdint");
        assert_ne!(shallow, other);
        assert_ne!(shallow, deep);
    }

    #[test]
    fn coloring_is_deterministic() {
        let theme = TagTheme::default();
        assert_eq!(
            theme.hashed_color("work/cdint"),
            theme.hashed_color("work/cdint")
        );
    }

    #[test]
    fn deep_tags_stay_above_the_brightness_floor() {
        let theme = TagTheme::default();
        let very_deep = theme.hashed_color("work/a/b/c/d/e/f/g/h");
        assert!(very_deep.r() > 0 || very_deep.g() > 0 || very_deep.b() > 0);
    }
}
