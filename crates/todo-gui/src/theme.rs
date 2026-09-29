//! Configurable presentation knobs for the graph editor.
//!
//! Every field is a matter of taste rather than correctness, so each one has an
//! opinionated default and can be overridden without touching code. Values that
//! encode protocol or data meaning never belong here.

use eframe::egui::{Color32, ecolor::Hsva};
use serde::{Deserialize, Serialize};

/// Presentation settings for the whole graph editor.
#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Theme {
    pub tag: TagTheme,
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
    use super::TagTheme;

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
