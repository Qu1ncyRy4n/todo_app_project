//! Tag filtering: which tasks the tag sidebar lets through.
//!
//! The rules are pure functions of the tag paths and the chosen states, so the
//! behavior is testable without a window and without a ledger.

use std::collections::BTreeMap;

/// What a tag does to the tasks that carry it.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TagFilterState {
    /// The tag neither includes nor excludes on its own.
    #[default]
    Neutral,
    /// Only tasks carrying this tag are shown.
    Only,
    /// Tasks carrying this tag are hidden.
    Hidden,
}

impl TagFilterState {
    /// Returns the next state in the neutral, only, hidden cycle.
    pub fn next(self) -> Self {
        match self {
            Self::Neutral => Self::Only,
            Self::Only => Self::Hidden,
            Self::Hidden => Self::Neutral,
        }
    }

    /// Returns the short marker drawn beside a tag.
    pub fn marker(self) -> &'static str {
        match self {
            Self::Neutral => "·",
            Self::Only => "only",
            Self::Hidden => "hide",
        }
    }
}

/// Returns the ancestor tags a task should carry that it does not already.
///
/// A tag is dropped when the task holds it outright, and when the task holds
/// something beneath it: a task tagged `work/cdint` is already within `work`, so
/// inheriting `work` as well would say nothing and clutter the card. Results come
/// back sorted and deduplicated, so display is stable.
pub fn inherited_tags(own: &[String], ancestors: &[String]) -> Vec<String> {
    let mut inherited = ancestors
        .iter()
        .filter(|candidate| {
            !own.iter()
                .any(|held| held == *candidate || held.starts_with(&format!("{candidate}/")))
        })
        .cloned()
        .collect::<Vec<_>>();
    inherited.sort();
    inherited.dedup();
    inherited
}

/// One row of the tag tree, already positioned in the hierarchy.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TagRow {
    /// The full path, which is what the filter is keyed by.
    pub path: String,
    /// The last segment, which is what the row shows.
    pub label: String,
    /// How deep the tag sits, where a top-level group is zero.
    pub depth: usize,
}

/// The tag states currently chosen in the sidebar.
#[derive(Clone, Debug, Default)]
pub struct TagFilter {
    states: BTreeMap<String, TagFilterState>,
}

impl TagFilter {
    /// Returns the state explicitly set on a tag, ignoring inheritance.
    pub fn state_of(&self, path: &str) -> TagFilterState {
        self.states.get(path).copied().unwrap_or_default()
    }

    /// Advances a tag to its next state, dropping it when it returns to neutral.
    pub fn cycle(&mut self, path: &str) {
        let next = self.state_of(path).next();
        if next == TagFilterState::Neutral {
            self.states.remove(path);
        } else {
            self.states.insert(path.to_owned(), next);
        }
    }

    /// Clears every tag state.
    pub fn clear(&mut self) {
        self.states.clear();
    }

    /// Returns whether any tag is filtering at all.
    pub fn is_active(&self) -> bool {
        !self.states.is_empty()
    }

    /// Resolves what a tag does, inheriting from its nearest configured ancestor.
    ///
    /// A child with its own state overrides what it would inherit, so `work` can be
    /// included while `work/cdint` under it is hidden.
    pub fn resolve(&self, path: &str) -> TagFilterState {
        let mut prefix = String::new();
        let mut resolved = TagFilterState::Neutral;
        for segment in path.split('/') {
            if !prefix.is_empty() {
                prefix.push('/');
            }
            prefix.push_str(segment);
            if let Some(state) = self.states.get(&prefix) {
                resolved = *state;
            }
        }
        resolved
    }

    /// Returns whether a task carrying `tags` passes the filter.
    ///
    /// Hiding wins over including, because excluding a tag is the more specific
    /// request: a task tagged both an included and a hidden tag is hidden. When any
    /// tag is set to only, a task carrying none of them is hidden too, which is what
    /// makes an untagged task disappear from a narrowed view.
    pub fn accepts<'a>(&self, tags: impl IntoIterator<Item = &'a str>) -> bool {
        if !self.is_active() {
            return true;
        }
        let wants_only = self
            .states
            .values()
            .any(|state| *state == TagFilterState::Only);
        let mut included = false;
        for tag in tags {
            match self.resolve(tag) {
                TagFilterState::Hidden => return false,
                TagFilterState::Only => included = true,
                TagFilterState::Neutral => {}
            }
        }
        !wants_only || included
    }
}

/// Builds the tag tree rows, including ancestors that hold no tasks themselves.
///
/// Ancestors are filled in so a hierarchy never appears to skip a level, and rows
/// come back in path order so the tree reads top down.
pub fn tag_rows<'a>(paths: impl IntoIterator<Item = &'a str>) -> Vec<TagRow> {
    let mut all = std::collections::BTreeSet::new();
    for path in paths {
        let mut prefix = String::new();
        for segment in path.split('/') {
            if !prefix.is_empty() {
                prefix.push('/');
            }
            prefix.push_str(segment);
            all.insert(prefix.clone());
        }
    }
    all.into_iter()
        .map(|path| {
            let depth = path.matches('/').count();
            let label = path.rsplit('/').next().unwrap_or(path.as_str()).to_owned();
            TagRow { path, label, depth }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{TagFilter, TagFilterState, inherited_tags, tag_rows};

    fn owned(paths: &[&str]) -> Vec<String> {
        paths.iter().map(|path| (*path).to_owned()).collect()
    }

    #[test]
    fn an_ancestor_tag_is_inherited() {
        let inherited = inherited_tags(&owned(&["tag/home"]), &owned(&["tag/work"]));
        assert_eq!(inherited, owned(&["tag/work"]));
    }

    #[test]
    fn a_tag_already_held_is_not_inherited_twice() {
        let inherited = inherited_tags(&owned(&["tag/work"]), &owned(&["tag/work"]));
        assert!(inherited.is_empty());
    }

    #[test]
    fn a_deeper_tag_makes_its_ancestor_redundant() {
        let inherited = inherited_tags(&owned(&["tag/work/cdint"]), &owned(&["tag/work"]));
        assert!(inherited.is_empty());
    }

    #[test]
    fn a_sibling_tag_is_still_inherited() {
        let inherited = inherited_tags(&owned(&["tag/work/cdint"]), &owned(&["tag/home"]));
        assert_eq!(inherited, owned(&["tag/home"]));
    }

    #[test]
    fn inherited_tags_come_back_sorted_and_deduplicated() {
        let inherited = inherited_tags(&[], &owned(&["tag/work", "tag/home", "tag/work"]));
        assert_eq!(inherited, owned(&["tag/home", "tag/work"]));
    }

    #[test]
    fn an_empty_filter_accepts_everything() {
        let filter = TagFilter::default();
        assert!(filter.accepts(["tag/work"]));
        assert!(filter.accepts(std::iter::empty()));
    }

    #[test]
    fn cycling_runs_neutral_only_hidden_and_back() {
        let mut filter = TagFilter::default();
        assert_eq!(filter.state_of("tag/work"), TagFilterState::Neutral);
        filter.cycle("tag/work");
        assert_eq!(filter.state_of("tag/work"), TagFilterState::Only);
        filter.cycle("tag/work");
        assert_eq!(filter.state_of("tag/work"), TagFilterState::Hidden);
        filter.cycle("tag/work");
        assert_eq!(filter.state_of("tag/work"), TagFilterState::Neutral);
        assert!(!filter.is_active());
    }

    #[test]
    fn only_hides_tasks_without_the_tag() {
        let mut filter = TagFilter::default();
        filter.cycle("tag/work");
        assert!(filter.accepts(["tag/work"]));
        assert!(!filter.accepts(["tag/fun"]));
        assert!(!filter.accepts(std::iter::empty()));
    }

    #[test]
    fn hiding_beats_including() {
        let mut filter = TagFilter::default();
        filter.cycle("tag/work");
        filter.cycle("tag/fun");
        filter.cycle("tag/fun");
        assert!(filter.accepts(["tag/work"]));
        assert!(!filter.accepts(["tag/work", "tag/fun"]));
    }

    #[test]
    fn a_child_inherits_its_parent_state() {
        let mut filter = TagFilter::default();
        filter.cycle("tag/work");
        assert!(filter.accepts(["tag/work/cdint"]));
    }

    #[test]
    fn a_child_can_override_an_inherited_state() {
        let mut filter = TagFilter::default();
        filter.cycle("tag/work");
        filter.cycle("tag/work/cdint");
        filter.cycle("tag/work/cdint");
        assert!(filter.accepts(["tag/work/cclab"]));
        assert!(!filter.accepts(["tag/work/cdint"]));
    }

    #[test]
    fn a_deeper_tag_does_not_leak_into_a_sibling() {
        let mut filter = TagFilter::default();
        filter.cycle("tag/work/cdint");
        assert!(filter.accepts(["tag/work/cdint/ingest"]));
        assert!(!filter.accepts(["tag/work/cclab"]));
    }

    #[test]
    fn rows_fill_in_missing_ancestors_and_carry_depth() {
        let rows = tag_rows(["tag/work/cdint", "tag/fun/gamedev"]);
        let paths = rows.iter().map(|row| row.path.as_str()).collect::<Vec<_>>();
        assert_eq!(
            paths,
            vec![
                "tag",
                "tag/fun",
                "tag/fun/gamedev",
                "tag/work",
                "tag/work/cdint",
            ]
        );
        assert!(
            rows.iter()
                .any(|row| row.label == "cdint" && row.depth == 2)
        );
    }
}
