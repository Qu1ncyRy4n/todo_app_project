//! A presentation-layer weight for a task.
//!
//! Nothing in the ledger derives readiness, ordering, or state from a weight.
//! It exists so a reader can size or rank nodes, and every coefficient is supplied
//! by the caller, so a different reader can weigh the same task differently without
//! a protocol change.

use serde::{Deserialize, Serialize};

/// The measurable facts a weight is computed from.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WeightInputs {
    /// Priority rank, 0 for low through 3 for critical.
    pub priority_rank: u8,
    /// Days until the due date. Negative when overdue, `None` when there is no due date.
    pub days_until_due: Option<i64>,
    /// Stated difficulty, from 1 to 5.
    pub difficulty: Option<u8>,
    /// Estimated effort in minutes.
    pub estimate_minutes: Option<u32>,
    /// How many tasks sit beneath this one.
    pub descendants: usize,
}

/// How much each input contributes to the weight.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct WeightFactors {
    pub priority: f32,
    pub urgency: f32,
    pub difficulty: f32,
    pub estimate: f32,
    pub descendants: f32,
    /// Days out at which a due date stops adding urgency.
    pub urgency_horizon_days: f32,
    /// Estimate, in hours, that counts as a full contribution.
    pub estimate_full_hours: f32,
    /// Descendant count that counts as a full contribution.
    pub descendants_full: f32,
}

impl Default for WeightFactors {
    fn default() -> Self {
        Self {
            priority: 1.0,
            urgency: 1.0,
            difficulty: 0.8,
            estimate: 0.6,
            descendants: 0.6,
            urgency_horizon_days: 14.0,
            estimate_full_hours: 8.0,
            descendants_full: 8.0,
        }
    }
}

/// Returns a weight from 0.0 to 1.0 for one task.
///
/// Each input is first normalized to 0.0..=1.0, then combined as a weighted mean,
/// so the result stays in range whatever the coefficients are and a caller can
/// zero out any input it does not believe in. A missing input contributes nothing
/// and its coefficient is left out of the mean, so absent data never reads as low
/// weight.
pub fn weight(inputs: WeightInputs, factors: WeightFactors) -> f32 {
    let mut total = 0.0_f32;
    let mut divisor = 0.0_f32;
    let mut add = |value: f32, factor: f32| {
        if factor <= 0.0 {
            return;
        }
        total += value.clamp(0.0, 1.0) * factor;
        divisor += factor;
    };

    add(
        f32::from(inputs.priority_rank.min(3)) / 3.0,
        factors.priority,
    );
    if let Some(days) = inputs.days_until_due {
        let horizon = factors.urgency_horizon_days.max(1.0);
        let remaining = f32::from(i16::try_from(days).unwrap_or(i16::MAX));
        add(1.0 - (remaining / horizon).clamp(0.0, 1.0), factors.urgency);
    }
    if let Some(difficulty) = inputs.difficulty {
        add(
            f32::from(difficulty.clamp(1, 5) - 1) / 4.0,
            factors.difficulty,
        );
    }
    if let Some(minutes) = inputs.estimate_minutes {
        let hours = f32::from(u16::try_from(minutes / 60).unwrap_or(u16::MAX));
        add(
            hours / factors.estimate_full_hours.max(1.0),
            factors.estimate,
        );
    }
    if inputs.descendants > 0 {
        let count = f32::from(u16::try_from(inputs.descendants).unwrap_or(u16::MAX));
        add(
            count / factors.descendants_full.max(1.0),
            factors.descendants,
        );
    }
    if divisor <= 0.0 {
        return 0.0;
    }
    (total / divisor).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::{WeightFactors, WeightInputs, weight};

    #[test]
    fn an_empty_task_weighs_the_least() {
        let light = weight(WeightInputs::default(), WeightFactors::default());
        assert!(light < 0.01);
    }

    #[test]
    fn priority_raises_the_weight() {
        let factors = WeightFactors::default();
        let low = weight(WeightInputs::default(), factors);
        let critical = weight(
            WeightInputs {
                priority_rank: 3,
                ..WeightInputs::default()
            },
            factors,
        );
        assert!(critical > low);
    }

    #[test]
    fn an_overdue_task_outweighs_a_distant_one() {
        let factors = WeightFactors::default();
        let overdue = weight(
            WeightInputs {
                days_until_due: Some(-3),
                ..WeightInputs::default()
            },
            factors,
        );
        let distant = weight(
            WeightInputs {
                days_until_due: Some(90),
                ..WeightInputs::default()
            },
            factors,
        );
        assert!(overdue > distant);
    }

    #[test]
    fn difficulty_estimate_and_children_each_add_weight() {
        let factors = WeightFactors::default();
        let base = WeightInputs {
            priority_rank: 1,
            ..WeightInputs::default()
        };
        let plain = weight(base, factors);
        let hard = weight(
            WeightInputs {
                difficulty: Some(5),
                ..base
            },
            factors,
        );
        let long = weight(
            WeightInputs {
                estimate_minutes: Some(60 * 8),
                ..base
            },
            factors,
        );
        let deep = weight(
            WeightInputs {
                descendants: 8,
                ..base
            },
            factors,
        );
        assert!(hard > plain);
        assert!(long > plain);
        assert!(deep > plain);
    }

    #[test]
    fn a_zeroed_factor_removes_its_input() {
        let factors = WeightFactors {
            difficulty: 0.0,
            ..WeightFactors::default()
        };
        let without = weight(
            WeightInputs {
                priority_rank: 2,
                ..WeightInputs::default()
            },
            factors,
        );
        let with_difficulty = weight(
            WeightInputs {
                priority_rank: 2,
                difficulty: Some(5),
                ..WeightInputs::default()
            },
            factors,
        );
        assert!((without - with_difficulty).abs() < f32::EPSILON);
    }

    #[test]
    fn the_weight_stays_within_range() {
        let heaviest = weight(
            WeightInputs {
                priority_rank: 9,
                days_until_due: Some(-9000),
                difficulty: Some(9),
                estimate_minutes: Some(u32::MAX),
                descendants: usize::MAX,
            },
            WeightFactors::default(),
        );
        assert!((0.0..=1.0).contains(&heaviest));
    }
}
