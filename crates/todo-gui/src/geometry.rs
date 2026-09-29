//! Pure geometry helpers for the graph editor.
//!
//! Everything here is a function of its inputs so it can be tested without a
//! window, a ledger, or a frame of rendering.

use eframe::egui::{Pos2, Vec2};

/// Returns the three corners of an arrowhead that ends at `end`.
///
/// The head keeps the requested size no matter how long the edge is or how far
/// the graph is zoomed, so callers that want a constant-size head simply pass a
/// constant length. Returns `None` when the two points coincide and no direction
/// can be derived.
pub fn arrowhead_points(start: Pos2, end: Pos2, length: f32, width: f32) -> Option<[Pos2; 3]> {
    let delta = end - start;
    let distance = delta.length();
    if distance <= f32::EPSILON {
        return None;
    }
    let direction = delta / distance;
    let perpendicular = Vec2::new(-direction.y, direction.x);
    let base = end - direction * length;
    let half = perpendicular * (width / 2.0);
    Some([end, base + half, base - half])
}

/// Returns the point where an edge's shaft should stop so the head is not doubled.
pub fn shaft_end(start: Pos2, end: Pos2, head_length: f32) -> Pos2 {
    let delta = end - start;
    let distance = delta.length();
    if distance <= f32::EPSILON {
        return end;
    }
    let trimmed = head_length.min(distance);
    end - (delta / distance) * trimmed
}

#[cfg(test)]
mod tests {
    use super::{arrowhead_points, shaft_end};
    use eframe::egui::pos2;

    /// Returns an arrowhead's drawn length, or `None` when no head was produced.
    fn head_length(from: (f32, f32), to: (f32, f32)) -> Option<f32> {
        arrowhead_points(pos2(from.0, from.1), pos2(to.0, to.1), 10.0, 8.0)
            .map(|[tip, wing, _]| (tip - wing).length())
    }

    #[test]
    fn an_arrowhead_keeps_its_length_on_edges_of_any_length() {
        let short = head_length((0.0, 0.0), (20.0, 0.0));
        let long = head_length((0.0, 0.0), (900.0, 0.0));
        assert!(matches!(
            (short, long),
            (Some(short), Some(long)) if (short - long).abs() < 0.001
        ));
    }

    #[test]
    fn an_arrowhead_points_along_the_edge() {
        let head = arrowhead_points(pos2(0.0, 0.0), pos2(0.0, 50.0), 10.0, 8.0);
        assert!(matches!(
            head,
            Some([tip, left, right])
                if (tip.y - 50.0).abs() < 0.001
                    && (left.y - 40.0).abs() < 0.001
                    && (right.y - 40.0).abs() < 0.001
        ));
    }

    #[test]
    fn a_zero_length_edge_has_no_arrowhead() {
        assert!(arrowhead_points(pos2(5.0, 5.0), pos2(5.0, 5.0), 10.0, 8.0).is_none());
    }

    #[test]
    fn a_shaft_stops_short_of_the_head() {
        let end = shaft_end(pos2(0.0, 0.0), pos2(100.0, 0.0), 10.0);
        assert!((end.x - 90.0).abs() < 0.001);
    }

    #[test]
    fn a_shaft_never_reverses_on_a_short_edge() {
        let end = shaft_end(pos2(0.0, 0.0), pos2(4.0, 0.0), 10.0);
        assert!((end.x - 0.0).abs() < 0.001);
    }
}
