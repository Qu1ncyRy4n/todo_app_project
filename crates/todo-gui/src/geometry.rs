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

/// Spacing for a banded focus layout.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BandSpacing {
    /// Distance between the completed, active, and future bands.
    pub band_gap: f32,
    /// Distance between neighbors inside one band.
    pub node_gap: f32,
    /// Distance between rows when a band wraps.
    pub row_gap: f32,
    /// How many nodes a band holds before it wraps to another row.
    pub band_width: usize,
}

/// Arranges nodes into three bands: completed above, active centered, future below.
///
/// Each band is centered on the same vertical axis so the active band reads as the
/// present moment, with finished work behind it and upcoming work ahead. A band
/// wider than `band_width` wraps into extra rows that grow away from the center,
/// so a long list of finished work never pushes into the active band.
pub fn banded_positions(
    completed: &[String],
    active: &[String],
    future: &[String],
    spacing: BandSpacing,
) -> HashMap<String, Vec2> {
    let mut positions = HashMap::new();
    place_band(completed, -1.0, spacing, &mut positions);
    place_band(active, 0.0, spacing, &mut positions);
    place_band(future, 1.0, spacing, &mut positions);
    positions
}

/// Places one band of nodes, growing away from the center as it wraps.
fn place_band(
    ids: &[String],
    direction: f32,
    spacing: BandSpacing,
    positions: &mut HashMap<String, Vec2>,
) {
    let width = spacing.band_width.max(1);
    for (index, id) in ids.iter().enumerate() {
        let column = index % width;
        let row = index / width;
        let row_count = ids.len().div_ceil(width);
        let in_row = if row + 1 == row_count && !ids.len().is_multiple_of(width) {
            ids.len() % width
        } else {
            width
        };
        let centered = f32::from(u16::try_from(column).unwrap_or(u16::MAX))
            - f32::from(u16::try_from(in_row.saturating_sub(1)).unwrap_or(u16::MAX)) / 2.0;
        let row_step = f32::from(u16::try_from(row).unwrap_or(u16::MAX)) * spacing.row_gap;
        let band = direction * (spacing.band_gap + row_step);
        positions.insert(id.clone(), Vec2::new(centered * spacing.node_gap, band));
    }
}

/// Returns the convex hull of `points`, counter-clockwise, without repeating the first.
///
/// Uses Andrew's monotone chain. Fewer than three points cannot bound an area, so
/// they are returned as given and the caller can fall back to a simpler shape.
pub fn convex_hull(points: &[Pos2]) -> Vec<Pos2> {
    if points.len() < 3 {
        return points.to_vec();
    }
    let mut sorted = points.to_vec();
    sorted.sort_by(|left, right| {
        left.x
            .total_cmp(&right.x)
            .then_with(|| left.y.total_cmp(&right.y))
    });
    sorted.dedup_by(|left, right| {
        (left.x - right.x).abs() < f32::EPSILON && (left.y - right.y).abs() < f32::EPSILON
    });
    if sorted.len() < 3 {
        return sorted;
    }
    let mut hull: Vec<Pos2> = Vec::with_capacity(sorted.len() * 2);
    for pass in 0..2 {
        let lower_mark = hull.len();
        let ordered: Vec<Pos2> = if pass == 0 {
            sorted.clone()
        } else {
            sorted.iter().rev().copied().collect()
        };
        for point in ordered {
            while hull.len() > lower_mark + 1 {
                let last = hull.last().copied();
                let previous = hull.len().checked_sub(2).and_then(|index| hull.get(index));
                let turns_right = match (previous, last) {
                    (Some(previous), Some(last)) => cross(*previous, last, point) <= 0.0,
                    _ => false,
                };
                if turns_right {
                    hull.pop();
                } else {
                    break;
                }
            }
            hull.push(point);
        }
        hull.pop();
    }
    hull
}

/// Returns the z component of the cross product of `a->b` and `a->c`.
fn cross(a: Pos2, b: Pos2, c: Pos2) -> f32 {
    let first = b - a;
    let second = c - a;
    first.x * second.y - first.y * second.x
}

/// Pushes every point of a closed path outward from the path's centroid.
pub fn expand_path(points: &[Pos2], padding: f32) -> Vec<Pos2> {
    let Some(centroid) = centroid(points) else {
        return points.to_vec();
    };
    points
        .iter()
        .map(|point| {
            let direction = *point - centroid;
            let length = direction.length();
            if length <= f32::EPSILON {
                *point
            } else {
                *point + direction / length * padding
            }
        })
        .collect()
}

/// Rounds a closed path by Chaikin subdivision, once per iteration.
///
/// Two iterations turn a hull into a shape that reads as organic while staying
/// cheap enough to recompute every frame.
pub fn smooth_closed_path(points: &[Pos2], iterations: u8) -> Vec<Pos2> {
    let mut path = points.to_vec();
    if path.len() < 3 {
        return path;
    }
    for _ in 0..iterations {
        let mut next = Vec::with_capacity(path.len() * 2);
        for (index, point) in path.iter().enumerate() {
            let following = path
                .get((index + 1) % path.len())
                .copied()
                .unwrap_or(*point);
            next.push(*point + (following - *point) * 0.25);
            next.push(*point + (following - *point) * 0.75);
        }
        path = next;
    }
    path
}

/// Returns the average of `points`, or `None` when there are none.
pub fn centroid(points: &[Pos2]) -> Option<Pos2> {
    if points.is_empty() {
        return None;
    }
    let count = f32::from(u16::try_from(points.len()).unwrap_or(u16::MAX));
    let sum = points
        .iter()
        .fold(Vec2::ZERO, |sum, point| sum + point.to_vec2());
    Some((sum / count).to_pos2())
}

/// Strength and reach of the tag separation pass.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SeparationSpacing {
    /// How many relaxation passes to run.
    pub iterations: u8,
    /// How strongly a node is drawn toward its own group.
    pub attraction: f32,
    /// How strongly a node is pushed out of a foreign group.
    pub repulsion: f32,
    /// How close a foreign group has to be before it pushes at all.
    pub radius: f32,
}

/// Nudges nodes toward their own tag group and out of foreign ones.
///
/// Each pass pulls a node toward the centroid of the group it belongs to and pushes
/// it away from the centroid of any other group within `radius`, so tag regions pull
/// apart instead of overlapping. Nodes with no group are left where they are.
///
/// Returns the adjusted positions; the input is not modified.
pub fn separate_groups(
    positions: &HashMap<String, Vec2>,
    groups: &HashMap<String, String>,
    spacing: SeparationSpacing,
) -> HashMap<String, Vec2> {
    let mut current = positions.clone();
    // ponytail: groups are approximated by their centroid, not their drawn hull.
    // A hull containment test is the upgrade if centroids prove too coarse.
    for _ in 0..spacing.iterations {
        let centroids = group_centroids(&current, groups);
        let mut next = current.clone();
        for (id, position) in &current {
            let Some(group) = groups.get(id) else {
                continue;
            };
            let mut shift = Vec2::ZERO;
            if let Some(own) = centroids.get(group) {
                shift += (*own - *position) * spacing.attraction;
            }
            for (other, centroid) in &centroids {
                if other == group {
                    continue;
                }
                let away = *position - *centroid;
                let distance = away.length();
                if distance >= spacing.radius {
                    continue;
                }
                let strength = (spacing.radius - distance) / spacing.radius;
                let direction = if distance <= f32::EPSILON {
                    Vec2::new(1.0, 0.0)
                } else {
                    away / distance
                };
                shift += direction * strength * spacing.repulsion;
            }
            next.insert(id.clone(), *position + shift);
        }
        current = next;
    }
    current
}

/// Returns the mean position of each group, over the nodes that belong to it.
fn group_centroids(
    positions: &HashMap<String, Vec2>,
    groups: &HashMap<String, String>,
) -> BTreeMap<String, Vec2> {
    let mut totals: BTreeMap<String, (Vec2, u16)> = BTreeMap::new();
    for (id, position) in positions {
        let Some(group) = groups.get(id) else {
            continue;
        };
        let entry = totals.entry(group.clone()).or_insert((Vec2::ZERO, 0));
        entry.0 += *position;
        entry.1 = entry.1.saturating_add(1);
    }
    totals
        .into_iter()
        .map(|(group, (sum, count))| (group, sum / f32::from(count.max(1))))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{
        BandSpacing, FlowDirection, FlowSpacing, SeparationSpacing, arrowhead_points,
        banded_positions, centroid, convex_hull, expand_path, layered_positions, separate_groups,
        shaft_end, smooth_closed_path,
    };
    use eframe::egui::{Vec2, pos2};

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

    fn band_spacing() -> BandSpacing {
        BandSpacing {
            band_gap: 200.0,
            node_gap: 180.0,
            row_gap: 90.0,
            band_width: 4,
        }
    }

    fn separation() -> SeparationSpacing {
        SeparationSpacing {
            iterations: 12,
            attraction: 0.12,
            repulsion: 12.0,
            radius: 400.0,
        }
    }

    fn grouped(pairs: &[(&str, &str)]) -> std::collections::HashMap<String, String> {
        pairs
            .iter()
            .map(|(id, group)| ((*id).to_owned(), (*group).to_owned()))
            .collect()
    }

    fn placed(pairs: &[(&str, f32, f32)]) -> std::collections::HashMap<String, Vec2> {
        pairs
            .iter()
            .map(|(id, x, y)| ((*id).to_owned(), Vec2::new(*x, *y)))
            .collect()
    }

    #[test]
    fn interleaved_groups_pull_apart() {
        let positions = placed(&[
            ("a1", 0.0, 0.0),
            ("b1", 40.0, 0.0),
            ("a2", 80.0, 0.0),
            ("b2", 120.0, 0.0),
        ]);
        let groups = grouped(&[("a1", "a"), ("a2", "a"), ("b1", "b"), ("b2", "b")]);
        let before = spread(&positions, &groups);
        let after = separate_groups(&positions, &groups, separation());
        let after = spread(&after, &groups);
        assert!(matches!((before, after), (Some(before), Some(after)) if after > before));
    }

    /// Returns the distance between the two group centroids, for comparison.
    fn spread(
        positions: &std::collections::HashMap<String, Vec2>,
        groups: &std::collections::HashMap<String, String>,
    ) -> Option<f32> {
        let mut sums: std::collections::BTreeMap<&str, (Vec2, f32)> =
            std::collections::BTreeMap::new();
        for (id, position) in positions {
            let group = groups.get(id)?;
            let entry = sums.entry(group.as_str()).or_insert((Vec2::ZERO, 0.0));
            entry.0 += *position;
            entry.1 += 1.0;
        }
        let mut centers = sums.values().map(|(sum, count)| *sum / *count);
        let first = centers.next()?;
        let second = centers.next()?;
        Some((first - second).length())
    }

    #[test]
    fn an_untagged_node_stays_put() {
        let positions = placed(&[("lonely", 5.0, 7.0), ("a1", 0.0, 0.0)]);
        let groups = grouped(&[("a1", "a")]);
        let moved = separate_groups(&positions, &groups, separation());
        assert!(matches!(moved.get("lonely"), Some(position) if *position == Vec2::new(5.0, 7.0)));
    }

    #[test]
    fn a_single_group_keeps_every_node() {
        let positions = placed(&[("a1", 0.0, 0.0), ("a2", 50.0, 0.0)]);
        let groups = grouped(&[("a1", "a"), ("a2", "a")]);
        let moved = separate_groups(&positions, &groups, separation());
        assert_eq!(moved.len(), 2);
    }

    #[test]
    fn separation_is_deterministic() {
        let positions = placed(&[("a1", 0.0, 0.0), ("b1", 30.0, 10.0)]);
        let groups = grouped(&[("a1", "a"), ("b1", "b")]);
        let first = separate_groups(&positions, &groups, separation());
        let second = separate_groups(&positions, &groups, separation());
        assert_eq!(first, second);
    }

    #[test]
    fn a_hull_drops_points_inside_it() {
        let square = vec![
            pos2(0.0, 0.0),
            pos2(10.0, 0.0),
            pos2(10.0, 10.0),
            pos2(0.0, 10.0),
            pos2(5.0, 5.0),
        ];
        let hull = convex_hull(&square);
        assert_eq!(hull.len(), 4);
        assert!(!hull.contains(&pos2(5.0, 5.0)));
    }

    #[test]
    fn a_hull_of_two_points_is_returned_unchanged() {
        let line = vec![pos2(0.0, 0.0), pos2(4.0, 0.0)];
        assert_eq!(convex_hull(&line).len(), 2);
    }

    #[test]
    fn expanding_a_path_moves_points_away_from_the_center() {
        let square = vec![
            pos2(-5.0, -5.0),
            pos2(5.0, -5.0),
            pos2(5.0, 5.0),
            pos2(-5.0, 5.0),
        ];
        let expanded = expand_path(&square, 10.0);
        let before = centroid(&square).map(|center| (square.first().copied(), center));
        let after = centroid(&expanded).map(|center| (expanded.first().copied(), center));
        assert!(matches!(
            (before, after),
            (Some((Some(before), first_center)), Some((Some(after), second_center)))
                if (after - second_center).length() > (before - first_center).length()
        ));
    }

    #[test]
    fn smoothing_rounds_a_path_into_more_points() {
        let triangle = vec![pos2(0.0, 0.0), pos2(10.0, 0.0), pos2(5.0, 9.0)];
        assert_eq!(smooth_closed_path(&triangle, 1).len(), 6);
        assert_eq!(smooth_closed_path(&triangle, 2).len(), 12);
    }

    #[test]
    fn smoothing_keeps_a_degenerate_path_as_is() {
        let line = vec![pos2(0.0, 0.0), pos2(1.0, 0.0)];
        assert_eq!(smooth_closed_path(&line, 3).len(), 2);
    }

    #[test]
    fn a_centroid_of_nothing_is_none() {
        assert!(centroid(&[]).is_none());
    }

    #[test]
    fn completed_work_sits_above_active_work_and_future_below() {
        let positions = banded_positions(
            &ids(&["done"]),
            &ids(&["now"]),
            &ids(&["later"]),
            band_spacing(),
        );
        assert!(matches!(
            (positions.get("done"), positions.get("now"), positions.get("later")),
            (Some(done), Some(now), Some(later))
                if done.y < now.y && now.y < later.y
        ));
    }

    #[test]
    fn the_active_band_is_centered_on_the_axis() {
        let positions = banded_positions(&[], &ids(&["a", "b"]), &[], band_spacing());
        let sum = positions.values().map(|position| position.x).sum::<f32>();
        assert!(sum.abs() < 0.001);
    }

    #[test]
    fn a_wide_band_wraps_away_from_the_center() {
        let wide = ids(&["a", "b", "c", "d", "e"]);
        let positions = banded_positions(&wide, &[], &[], band_spacing());
        assert!(matches!(
            (positions.get("a"), positions.get("e")),
            (Some(first), Some(fifth)) if fifth.y < first.y
        ));
    }

    #[test]
    fn an_empty_band_places_nothing() {
        let positions = banded_positions(&[], &[], &[], band_spacing());
        assert!(positions.is_empty());
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
