//! Color ramps for tag hues.
//!
//! The rainbow ramp reproduces matplotlib's `rainbow` colormap, which is defined
//! by three closed-form curves rather than a lookup table, so it can be evaluated
//! exactly here: red is `|2x - 1/2|`, green is `sin(pi x)`, blue is `cos(pi x / 2)`,
//! each clamped to zero and one. Sampling it evenly across the visible tag groups
//! is the same thing as `cmap(i / len(values))` in a plotting script.

use eframe::egui::Color32;
use serde::{Deserialize, Serialize};

/// Which ramp turns a position on the wheel into a color.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Colormap {
    /// Matplotlib's rainbow: violet through blue, green, yellow, to red.
    #[default]
    Rainbow,
    /// A plain hue circle, which wraps around rather than ending.
    ///
    /// Useful when many groups are visible, since a circle has no ends to crowd.
    Hue,
}

/// Returns the rainbow color at `position`, which is clamped to zero and one.
pub fn rainbow(position: f32) -> Color32 {
    let x = position.clamp(0.0, 1.0);
    let red = (2.0 * x - 0.5).abs().clamp(0.0, 1.0);
    let green = (std::f32::consts::PI * x).sin().clamp(0.0, 1.0);
    let blue = (std::f32::consts::PI * x / 2.0).cos().clamp(0.0, 1.0);
    Color32::from_rgb(channel(red), channel(green), channel(blue))
}

/// Converts a zero-to-one channel into a byte without a lossy cast.
///
/// Stepping through the 256 levels avoids a float-to-integer conversion, which the
/// workspace lints forbid for being silently lossy at the edges.
fn channel(value: f32) -> u8 {
    let scaled = value.clamp(0.0, 1.0) * 255.0;
    let mut level = 0_u8;
    while level < u8::MAX && f32::from(level) + 0.5 < scaled {
        level += 1;
    }
    level
}

#[cfg(test)]
mod tests {
    use super::{Colormap, rainbow};

    #[test]
    fn the_ramp_runs_from_violet_to_red() {
        let start = rainbow(0.0);
        let end = rainbow(1.0);
        assert!(start.b() > start.r());
        assert!(end.r() > end.b());
    }

    #[test]
    fn the_middle_of_the_ramp_is_green() {
        let middle = rainbow(0.5);
        assert!(middle.g() > middle.r());
    }

    #[test]
    fn the_ramp_is_clamped_outside_its_range() {
        assert_eq!(rainbow(-4.0), rainbow(0.0));
        assert_eq!(rainbow(9.0), rainbow(1.0));
    }

    #[test]
    fn the_ramp_is_deterministic() {
        assert_eq!(rainbow(0.37), rainbow(0.37));
    }

    #[test]
    fn evenly_spaced_samples_are_all_different() {
        let samples = (0_u8..6)
            .map(|step| rainbow(f32::from(step) / 5.0))
            .collect::<Vec<_>>();
        for (index, color) in samples.iter().enumerate() {
            assert!(
                !samples.iter().skip(index + 1).any(|other| other == color),
                "sample {index} repeats a color"
            );
        }
    }

    #[test]
    fn rainbow_is_the_default_ramp() {
        assert_eq!(Colormap::default(), Colormap::Rainbow);
    }
}
