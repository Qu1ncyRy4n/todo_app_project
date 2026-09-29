//! Pure geometry helpers for the graph editor.
//!
//! Everything here is a function of its inputs so it can be tested without a
//! window, a ledger, or a frame of rendering.

use eframe::egui::{Pos2, Vec2};
use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};

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

/// The axis a normalized graph flows along.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FlowDirection {
    /// Dependencies above, dependents below.
    #[default]
    Down,
    /// Dependencies left, dependents right.
    Right,
}

/// Spacing and direction for a normalized layout.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlowSpacing {
    pub direction: FlowDirection,
    /// Distance between one layer and the next.
    pub layer_gap: f32,
    /// Distance between neighbors within a layer.
    pub node_gap: f32,
}

/// Arranges nodes into layers so every edge points down, or right.
///
/// Layering is longest-path: a node sits one layer below its deepest predecessor,
/// which keeps every edge flowing the same way. Nodes left over in a cycle are
/// placed in a final layer rather than dropped, so a cyclic graph still lays out
/// and stays visible instead of vanishing.
///
/// Positions are returned in graph units relative to the first node, so the caller
/// decides where the arrangement sits on screen.
pub fn layered_positions(
    ids: &[String],
    edges: &[(String, String)],
    spacing: FlowSpacing,
) -> HashMap<String, Vec2> {
    let present = ids.iter().cloned().collect::<BTreeSet<_>>();
    let mut successors: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    let mut indegree: BTreeMap<&str, usize> = ids.iter().map(|id| (id.as_str(), 0)).collect();
    for (from, to) in edges {
        if from == to || !present.contains(from) || !present.contains(to) {
            continue;
        }
        if successors
            .entry(from.as_str())
            .or_default()
            .insert(to.as_str())
        {
            *indegree.entry(to.as_str()).or_insert(0) += 1;
        }
    }

    let mut layer: BTreeMap<&str, usize> = BTreeMap::new();
    let mut queue = indegree
        .iter()
        .filter(|(_, degree)| **degree == 0)
        .map(|(id, _)| *id)
        .collect::<VecDeque<_>>();
    for id in &queue {
        layer.insert(id, 0);
    }
    let mut remaining = indegree.clone();
    while let Some(id) = queue.pop_front() {
        let current = layer.get(id).copied().unwrap_or(0);
        let Some(children) = successors.get(id) else {
            continue;
        };
        for child in children {
            let entry = layer.entry(child).or_insert(0);
            *entry = (*entry).max(current + 1);
            if let Some(degree) = remaining.get_mut(child) {
                *degree = degree.saturating_sub(1);
                if *degree == 0 {
                    queue.push_back(child);
                }
            }
        }
    }
    // ponytail: nodes still holding an indegree are in a cycle; they land in a
    // trailing layer. Proper cycle breaking can come if real cycles show up.
    let cycle_layer = layer.values().copied().max().map_or(0, |max| max + 1);
    for id in &present {
        layer.entry(id.as_str()).or_insert(cycle_layer);
    }

    let mut rows: BTreeMap<usize, Vec<&str>> = BTreeMap::new();
    for (id, depth) in &layer {
        rows.entry(*depth).or_default().push(id);
    }
    let mut positions = HashMap::new();
    for (depth, mut row) in rows {
        // ponytail: alphabetical within a layer keeps it deterministic; ordering by
        // predecessor barycenter would cut edge crossings if it becomes a problem.
        row.sort_unstable();
        let along = f32::from(u16::try_from(depth).unwrap_or(u16::MAX)) * spacing.layer_gap;
        for (index, id) in row.into_iter().enumerate() {
            let across = f32::from(u16::try_from(index).unwrap_or(u16::MAX)) * spacing.node_gap;
            let position = match spacing.direction {
                FlowDirection::Down => Vec2::new(across, along),
                FlowDirection::Right => Vec2::new(along, across),
            };
            positions.insert(id.to_owned(), position);
        }
    }
    positions
}

#[cfg(test)]
mod tests {
    use super::{FlowDirection, FlowSpacing, arrowhead_points, layered_positions, shaft_end};
    use eframe::egui::pos2;

    fn spacing(direction: FlowDirection) -> FlowSpacing {
        FlowSpacing {
            direction,
            layer_gap: 100.0,
            node_gap: 200.0,
        }
    }

    fn ids(names: &[&str]) -> Vec<String> {
        names.iter().map(|name| (*name).to_owned()).collect()
    }

    fn edges(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(from, to)| ((*from).to_owned(), (*to).to_owned()))
            .collect()
    }

    #[test]
    fn a_chain_flows_one_layer_at_a_time() {
        let positions = layered_positions(
            &ids(&["a", "b", "c"]),
            &edges(&[("a", "b"), ("b", "c")]),
            spacing(FlowDirection::Down),
        );
        let depth = |id: &str| positions.get(id).map(|position| position.y);
        assert!(matches!(
            (depth("a"), depth("b"), depth("c")),
            (Some(first), Some(second), Some(third))
                if first < second && second < third
        ));
    }

    #[test]
    fn siblings_share_a_layer_and_spread_across_it() {
        let positions = layered_positions(
            &ids(&["root", "left", "right"]),
            &edges(&[("root", "left"), ("root", "right")]),
            spacing(FlowDirection::Down),
        );
        assert!(matches!(
            (positions.get("left"), positions.get("right")),
            (Some(left), Some(right))
                if (left.y - right.y).abs() < 0.001 && (left.x - right.x).abs() > 0.001
        ));
    }

    #[test]
    fn a_node_sits_below_its_deepest_predecessor() {
        let positions = layered_positions(
            &ids(&["a", "b", "c", "d"]),
            &edges(&[("a", "b"), ("b", "c"), ("a", "d"), ("c", "d")]),
            spacing(FlowDirection::Down),
        );
        assert!(matches!(
            (positions.get("c"), positions.get("d")),
            (Some(third), Some(last)) if last.y > third.y
        ));
    }

    #[test]
    fn flowing_right_swaps_the_axes() {
        let positions = layered_positions(
            &ids(&["a", "b"]),
            &edges(&[("a", "b")]),
            spacing(FlowDirection::Right),
        );
        assert!(matches!(
            (positions.get("a"), positions.get("b")),
            (Some(first), Some(second))
                if second.x > first.x && (first.y - second.y).abs() < 0.001
        ));
    }

    #[test]
    fn a_cycle_still_places_every_node() {
        let positions = layered_positions(
            &ids(&["a", "b", "c"]),
            &edges(&[("a", "b"), ("b", "c"), ("c", "a")]),
            spacing(FlowDirection::Down),
        );
        assert_eq!(positions.len(), 3);
    }

    #[test]
    fn edges_to_hidden_nodes_are_ignored() {
        let positions = layered_positions(
            &ids(&["a"]),
            &edges(&[("hidden", "a"), ("a", "hidden")]),
            spacing(FlowDirection::Down),
        );
        assert!(matches!(positions.get("a"), Some(position) if position.y.abs() < 0.001));
    }

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
