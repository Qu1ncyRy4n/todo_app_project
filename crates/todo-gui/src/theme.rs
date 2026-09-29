//! Configurable presentation knobs for the graph editor.
//!
//! Every field is a matter of taste rather than correctness, so each one has an
//! opinionated default and can be overridden without touching code. Values that
//! encode protocol or data meaning never belong here.

use crate::geometry::FlowDirection;
use anyhow::{Context, Result};
use eframe::egui::{Color32, ecolor::Hsva};
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

/// Presentation settings for the whole graph editor.
#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Theme {
    pub focus: FocusTheme,
    pub layout: LayoutTheme,
    pub node: NodeTheme,
    pub tag: TagTheme,
    pub edge: EdgeTheme,
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

/// Controls how the Normalize command arranges the graph.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct LayoutTheme {
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
    /// Whether active `blocked_by` relationships order the layers.
    ///
    /// Off by default, because an active blocker is a subset of the planned
    /// dependency it came from and would only duplicate that edge.
    pub follow_blockers: bool,
}

impl Default for LayoutTheme {
    fn default() -> Self {
        Self {
            direction: FlowDirection::default(),
            layer_gap: 120.0,
            node_gap: 185.0,
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
    /// Whether the frame and its label are drawn around the active band.
    pub show_frame: bool,
    /// Whether tasks deferred or scheduled ahead of today join the future band.
    pub future_includes_deferred: bool,
}

impl Default for FocusTheme {
    fn default() -> Self {
        Self {
            band_gap: 235.0,
            node_gap: 185.0,
            row_gap: 95.0,
            band_width: 4,
            show_frame: true,
            future_includes_deferred: true,
        }
    }
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
    /// Fill opacity of a tag region, out of 255.
    pub region_fill_alpha: u8,
    /// Outline opacity of a tag region, out of 255.
    pub region_stroke_alpha: u8,
    /// Width of the tag accent stripe drawn on a node's leading edge.
    pub node_stripe_width: f32,
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
            region_fill_alpha: 26,
            region_stroke_alpha: 110,
            node_stripe_width: 7.0,
        }
    }
}

impl TagTheme {
    /// Colors a tag path from its top-level group hue, darkened once per depth.
    ///
    /// A child tag is a deeper shade of its parent: `work` and `work/cdint` share
    /// one hue and differ only by depth. The mapping is a pure function of the
    /// path, never of an item id or random state, so it is stable across runs.
    pub fn color(&self, path: &str) -> Color32 {
        let group = path.split('/').next().unwrap_or(path);
        let hash = group.bytes().fold(0_u32, |hash, byte| {
            hash.wrapping_mul(31).wrapping_add(u32::from(byte))
        });
        let hue = f32::from(u16::try_from(hash % 360).unwrap_or(0)) / 360.0;
        let depth = self.depth_of(path);
        // ponytail: one shade step per depth level; the hierarchy-aware hue wheel
        // that divides a parent sector between visible siblings is the upgrade.
        let saturation =
            (self.base_saturation + self.depth_saturation_step * depth).min(self.max_saturation);
        let value = (self.base_value - self.depth_value_step * depth).max(self.min_value);
        Color32::from(Hsva::new(hue, saturation, value, 1.0))
    }

    /// Returns the tag depth of a path, where a top-level group is depth zero.
    fn depth_of(&self, path: &str) -> f32 {
        f32::from(u8::try_from(path.matches('/').count()).unwrap_or(u8::MAX))
    }
}

#[cfg(test)]
mod tests {
    use super::{EdgeTheme, NodeShape, NodeTheme, TagTheme, Theme};
    use std::path::Path;

    #[test]
    fn a_missing_theme_file_yields_the_defaults() {
        let theme = Theme::load(Path::new("/nonexistent/todo-ledger/theme.toml"));
        assert!(matches!(theme, Ok(theme) if theme == Theme::default()));
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
        let parent = theme.color("work");
        let child = theme.color("work/cdint");
        let grandchild = theme.color("work/cdint/ingest");
        assert!(child.g() < parent.g() || child.r() < parent.r() || child.b() < parent.b());
        assert!(grandchild.g() <= child.g());
    }

    #[test]
    fn separate_groups_receive_separate_colors() {
        let theme = TagTheme::default();
        assert_ne!(theme.color("work"), theme.color("home"));
    }

    #[test]
    fn a_child_keeps_the_hue_of_its_group() {
        let theme = TagTheme::default();
        let shallow = theme.color("work/cdint");
        let deep = theme.color("work/cdint/ingest/api");
        let other = theme.color("home/cdint");
        assert_ne!(shallow, other);
        assert_ne!(shallow, deep);
    }

    #[test]
    fn coloring_is_deterministic() {
        let theme = TagTheme::default();
        assert_eq!(theme.color("work/cdint"), theme.color("work/cdint"));
    }

    #[test]
    fn deep_tags_stay_above_the_brightness_floor() {
        let theme = TagTheme::default();
        let very_deep = theme.color("work/a/b/c/d/e/f/g/h");
        assert!(very_deep.r() > 0 || very_deep.g() > 0 || very_deep.b() > 0);
    }
}
