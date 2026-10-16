//! One-line typed entry: `kind:path` links and `key=value` fields mixed into a
//! title, shared by the CLI and the GUI quick-entry bar. Source: TE-0016.

use crate::{NamespacePath, NamespacePathError, Priority, validate_segment};
use chrono::NaiveDate;
use std::fmt;
use thiserror::Error;

/// Which optional spellings the parser accepts besides the always-on slugs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EntrySyntax {
    /// `#tag`, `@actor`, and `+project` as short forms of `tag:`, `by:`, and `in:`.
    pub ascii: bool,
    /// `key=value` sets a field. Only namespace nodes carry fields, so item
    /// entry turns this off and such words stay in the title.
    pub fields: bool,
}

/// What a line of typed entry asks for.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Entry {
    /// The words that were not directives, joined by single spaces.
    pub title: String,
    pub directives: Vec<Directive>,
}

/// One typed word. `remove` is set by a leading `-`, as in `-tag:old`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Directive {
    pub remove: bool,
    pub kind: DirectiveKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DirectiveKind {
    /// `tag:work/apps`, a full `tag/…` namespace path.
    Tag(NamespacePath),
    /// `in:todo`, a project name as `todo project` accepts it.
    Project(String),
    /// `by:quincy` and friends. `actor` is the name under `actor/`, or `me`.
    Role {
        role: String,
        actor: String,
    },
    Due(NaiveDate),
    Defer(NaiveDate),
    Priority(Priority),
    EstimateMinutes(u32),
    /// `dep:<id>`, an item reference.
    Depends(String),
    /// `see:<id>`, an item reference for `relates-to`.
    Relates(String),
    Field {
        key: String,
        value: String,
    },
}

/// Writes the canonical slug form, which also parses back.
impl fmt::Display for DirectiveKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Tag(path) => {
                let name = path.as_str().strip_prefix("tag/").unwrap_or(path.as_str());
                write!(formatter, "tag:{name}")
            }
            Self::Project(name) => write!(formatter, "in:{name}"),
            Self::Role { role, actor } => write!(formatter, "as:{role}/{actor}"),
            Self::Due(date) => write!(formatter, "due:{date}"),
            Self::Defer(date) => write!(formatter, "defer:{date}"),
            Self::Priority(priority) => write!(formatter, "p:{priority}"),
            Self::EstimateMinutes(minutes) => write!(formatter, "est:{minutes}m"),
            Self::Depends(item) => write!(formatter, "dep:{item}"),
            Self::Relates(item) => write!(formatter, "see:{item}"),
            Self::Field { key, value } => write!(formatter, "{key}={value}"),
        }
    }
}

pub const AUTHOR: &str = "author";
pub const CONTRIBUTOR: &str = "contributor";
pub const ASSIGNEE: &str = "assignee";
pub const REQUESTER: &str = "requester";

#[derive(Clone, Debug, Eq, PartialEq, Error)]
pub enum EntryError {
    #[error("{word:?}: {reason}; put -- before it to keep it in the title")]
    Invalid { word: String, reason: String },
}

/// Parses typed entry from shell words.
///
/// A word containing whitespace was quoted as a phrase and is always title. A
/// word `--` ends parsing; everything after it is title. A recognized prefix
/// with a value that does not parse is an error rather than silent title, since
/// the prefix shows what was meant.
pub fn parse_entry(words: &[String], syntax: EntrySyntax) -> Result<Entry, EntryError> {
    let mut title = Vec::new();
    let mut directives = Vec::new();
    let mut literal = false;
    for word in words {
        if literal || word.contains(char::is_whitespace) {
            title.push(word.as_str());
            continue;
        }
        if word == "--" {
            literal = true;
            continue;
        }
        match parse_word(word, syntax) {
            Ok(Some(directive)) => directives.push(directive),
            Ok(None) => title.push(word.as_str()),
            Err(reason) => {
                return Err(EntryError::Invalid {
                    word: word.clone(),
                    reason,
                });
            }
        }
    }
    Ok(Entry {
        title: title.join(" "),
        directives,
    })
}

fn parse_word(word: &str, syntax: EntrySyntax) -> Result<Option<Directive>, String> {
    let (remove, body) = match word.strip_prefix('-') {
        Some(rest) if !rest.is_empty() => (true, rest),
        _ => (false, word),
    };
    let Some(kind) = parse_kind(body, syntax)? else {
        return Ok(None);
    };
    Ok(Some(Directive { remove, kind }))
}

fn parse_kind(body: &str, syntax: EntrySyntax) -> Result<Option<DirectiveKind>, String> {
    if let Some((prefix, value)) = body.split_once(':').filter(|(_, value)| !value.is_empty()) {
        let role = |role: &str| actor(value).map(|actor| role_kind(role, actor));
        return match prefix {
            "tag" => tag(value).map(Some),
            "in" => project(value).map(Some),
            "by" => role(AUTHOR).map(Some),
            "contrib" => role(CONTRIBUTOR).map(Some),
            "to" => role(ASSIGNEE).map(Some),
            "for" => role(REQUESTER).map(Some),
            "as" => {
                let (role, name) = value.split_once('/').ok_or("expected as:<role>/<actor>")?;
                validate_segment(role).map_err(|error| error.to_string())?;
                Ok(Some(role_kind(role, actor(name)?)))
            }
            "due" => date(value).map(|date| Some(DirectiveKind::Due(date))),
            "defer" => date(value).map(|date| Some(DirectiveKind::Defer(date))),
            "p" => priority(value).map(Some),
            "est" => estimate(value).map(|minutes| Some(DirectiveKind::EstimateMinutes(minutes))),
            "dep" => Ok(Some(DirectiveKind::Depends(value.to_owned()))),
            "see" => Ok(Some(DirectiveKind::Relates(value.to_owned()))),
            _ => Ok(None),
        };
    }
    if matches!(body, "p0" | "p1" | "p2" | "p3") {
        return priority(body).map(Some);
    }
    if syntax.ascii {
        let mut chars = body.chars();
        let rest = chars.as_str().get(1..).filter(|rest| !rest.is_empty());
        if let (Some(sigil), Some(rest)) = (chars.next(), rest) {
            match sigil {
                '#' => return tag(rest).map(Some),
                '@' => return actor(rest).map(|actor| Some(role_kind(AUTHOR, actor))),
                '+' => return project(rest).map(Some),
                _ => {}
            }
        }
    }
    if syntax.fields
        && let Some((key, value)) = body.split_once('=')
        && validate_segment(key).is_ok()
    {
        return Ok(Some(DirectiveKind::Field {
            key: key.to_owned(),
            value: value.to_owned(),
        }));
    }
    Ok(None)
}

fn role_kind(role: &str, actor: String) -> DirectiveKind {
    DirectiveKind::Role {
        role: role.to_owned(),
        actor,
    }
}

fn tag(value: &str) -> Result<DirectiveKind, String> {
    let value = value.strip_prefix("tag/").unwrap_or(value);
    format!("tag/{value}")
        .parse()
        .map(DirectiveKind::Tag)
        .map_err(|error: NamespacePathError| error.to_string())
}

fn project(value: &str) -> Result<DirectiveKind, String> {
    validate_path(value)?;
    Ok(DirectiveKind::Project(value.to_owned()))
}

fn actor(value: &str) -> Result<String, String> {
    validate_path(value)?;
    Ok(value.to_owned())
}

fn validate_path(value: &str) -> Result<(), String> {
    value
        .split('/')
        .try_for_each(validate_segment)
        .map_err(|error| error.to_string())
}

fn date(value: &str) -> Result<NaiveDate, String> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d").map_err(|_| "expected YYYY-MM-DD".to_owned())
}

fn priority(value: &str) -> Result<DirectiveKind, String> {
    value
        .parse()
        .map(DirectiveKind::Priority)
        .map_err(|error: crate::item::ParsePriorityError| error.to_string())
}

/// Accepts `90`, `30m`, `2h`, and `1h30m`, all in minutes.
fn estimate(value: &str) -> Result<u32, String> {
    let invalid = || "expected minutes such as 30m, 2h, or 1h30m".to_owned();
    if let Ok(minutes) = value.parse() {
        return Ok(minutes);
    }
    let (hours, rest) = value.split_once('h').unwrap_or(("0", value));
    let minutes = rest.strip_suffix('m').unwrap_or(rest);
    let hours: u32 = hours.parse().map_err(|_| invalid())?;
    let minutes: u32 = if minutes.is_empty() {
        0
    } else {
        minutes.parse().map_err(|_| invalid())?
    };
    hours
        .checked_mul(60)
        .and_then(|total| total.checked_add(minutes))
        .filter(|total| *total > 0)
        .ok_or_else(invalid)
}

#[cfg(test)]
#[allow(
    clippy::indexing_slicing,
    clippy::unwrap_used,
    reason = "test fixtures use direct setup and assertion values"
)]
mod tests {
    use super::{DirectiveKind, EntrySyntax, parse_entry};
    use crate::Priority;

    const ITEM: EntrySyntax = EntrySyntax {
        ascii: true,
        fields: false,
    };

    fn words(line: &[&str]) -> Vec<String> {
        line.iter().map(|word| (*word).to_owned()).collect()
    }

    fn kinds(line: &[&str], syntax: EntrySyntax) -> Vec<DirectiveKind> {
        parse_entry(&words(line), syntax)
            .unwrap()
            .directives
            .into_iter()
            .map(|directive| directive.kind)
            .collect()
    }

    #[test]
    fn bare_and_quoted_titles_read_the_same() {
        let bare = parse_entry(&words(&["Fix", "sync", "race", "by:me"]), ITEM).unwrap();
        let quoted = parse_entry(&words(&["Fix sync race", "by:me"]), ITEM).unwrap();
        assert_eq!(bare, quoted);
        assert_eq!(bare.title, "Fix sync race");
    }

    #[test]
    fn slugs_and_ascii_forms_agree() {
        let slugs = kinds(&["tag:work/gui", "by:quincy", "in:todo"], ITEM);
        let ascii = kinds(&["#work/gui", "@quincy", "+todo"], ITEM);
        assert_eq!(slugs, ascii);
        assert_eq!(
            slugs[0],
            DirectiveKind::Tag("tag/work/gui".parse().unwrap())
        );
        let off = EntrySyntax {
            ascii: false,
            ..ITEM
        };
        assert_eq!(parse_entry(&words(&["#work"]), off).unwrap().title, "#work");
    }

    #[test]
    fn roles_dates_priority_and_estimates_parse() {
        let parsed = kinds(
            &[
                "as:reviewer/steve",
                "to:quincy/agent/bot",
                "due:2026-10-20",
                "p1",
                "est:1h30m",
            ],
            ITEM,
        );
        assert_eq!(
            parsed[0],
            DirectiveKind::Role {
                role: "reviewer".to_owned(),
                actor: "steve".to_owned()
            }
        );
        assert_eq!(
            parsed[1],
            DirectiveKind::Role {
                role: "assignee".to_owned(),
                actor: "quincy/agent/bot".to_owned()
            }
        );
        assert_eq!(parsed[3], DirectiveKind::Priority(Priority::High));
        assert_eq!(parsed[4], DirectiveKind::EstimateMinutes(90));
    }

    #[test]
    fn the_canonical_form_parses_back() {
        let line = [
            "#work/gui",
            "@me",
            "+todo",
            "due:2026-10-20",
            "defer:2026-10-21",
            "p0",
            "est:2h",
            "dep:kohil",
            "see:dojul",
        ];
        let parsed = kinds(&line, ITEM);
        let canonical: Vec<String> = parsed.iter().map(ToString::to_string).collect();
        let canonical: Vec<&str> = canonical.iter().map(String::as_str).collect();
        assert_eq!(kinds(&canonical, ITEM), parsed);
    }

    #[test]
    fn a_leading_dash_removes_and_double_dash_ends_parsing() {
        let entry = parse_entry(&words(&["-tag:old", "--", "by:me", "-x"]), ITEM).unwrap();
        assert!(entry.directives[0].remove);
        assert_eq!(entry.directives.len(), 1);
        assert_eq!(entry.title, "by:me -x");
    }

    #[test]
    fn a_known_prefix_with_a_bad_value_is_an_error() {
        assert!(parse_entry(&words(&["due:tomorrow"]), ITEM).is_err());
        assert!(parse_entry(&words(&["by:Quincy"]), ITEM).is_err());
        // An unknown prefix or an empty value is ordinary title text.
        let entry = parse_entry(&words(&["note:", "http://x", "see:"]), ITEM).unwrap();
        assert!(entry.directives.is_empty());
    }

    #[test]
    fn fields_are_only_read_where_enabled() {
        let node = EntrySyntax {
            ascii: true,
            fields: true,
        };
        assert_eq!(
            kinds(&["gh=Qu1ncyRy4n", "email="], node),
            vec![
                DirectiveKind::Field {
                    key: "gh".to_owned(),
                    value: "Qu1ncyRy4n".to_owned()
                },
                DirectiveKind::Field {
                    key: "email".to_owned(),
                    value: String::new()
                },
            ]
        );
        assert_eq!(parse_entry(&words(&["a=b"]), ITEM).unwrap().title, "a=b");
    }
}
