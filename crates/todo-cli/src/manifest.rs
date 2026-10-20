//! The TOML manifest of actor and project records. Design: TE-0016.

use anyhow::{Context, Result, bail};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    fmt::Write as _,
    fs,
    path::Path,
};
use todo_core::{NamespaceKind, NamespacePath, validate_segment};
use todo_store::{Ledger, MEMBER_OF};

const KINDS: [&str; 2] = ["actor", "project"];
const LINK_KEY: &str = "member-of";
/// The same link written on the org's side; export uses it for `kind = "org"`.
const MEMBERS_KEY: &str = "members";

#[derive(Default)]
struct Node {
    fields: BTreeMap<String, String>,
    member_of: Vec<NamespacePath>,
    members: Vec<NamespacePath>,
}

type Records = BTreeMap<NamespacePath, Node>;

enum Change {
    Create(NamespacePath),
    Set(NamespacePath, String, String),
    Clear(NamespacePath, String),
    Link(NamespacePath, NamespacePath),
    Unlink(NamespacePath, NamespacePath),
}

impl fmt::Display for Change {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Create(path) => write!(formatter, "+ {path}"),
            Self::Set(path, key, value) => write!(formatter, "+ {path} {key}={value}"),
            Self::Clear(path, key) => write!(formatter, "- {path} {key}"),
            Self::Link(path, target) => write!(formatter, "+ {path} {LINK_KEY} {target}"),
            Self::Unlink(path, target) => write!(formatter, "- {path} {LINK_KEY} {target}"),
        }
    }
}

pub async fn export(ledger: &Ledger, file: Option<&Path>) -> Result<()> {
    let records = current(ledger).await?;
    let text = format!(
        "# Actor and project records. Edit, then `todo manifest apply <file>`.\n\
         # A removed line clears that field; nodes left out are untouched.\n\
         based_on = \"{}\"\n\n{}",
        digest(&records),
        render(&records)
    );
    match file {
        Some(file) if file.exists() => {
            bail!("{} exists; export to a new file", file.display())
        }
        Some(file) => fs::write(file, text).with_context(|| format!("write {}", file.display())),
        None => {
            print!("{text}");
            Ok(())
        }
    }
}

pub async fn apply(ledger: &Ledger, file: &Path, dry_run: bool) -> Result<()> {
    let text = fs::read_to_string(file).with_context(|| format!("read {}", file.display()))?;
    let (based_on, wanted) = parse(&text).with_context(|| format!("in {}", file.display()))?;
    let have = current(ledger).await?;
    let changes = diff(&have, &wanted);
    if changes.is_empty() {
        println!("No changes.");
        return Ok(());
    }
    for change in &changes {
        println!("{change}");
    }
    if let Some(based_on) = based_on
        && based_on != digest(&have)
    {
        bail!(
            "actor or project records changed since {} was exported; export to a new file and carry your edits over",
            file.display()
        );
    }
    if dry_run {
        return Ok(());
    }
    for change in changes {
        match change {
            Change::Create(path) => {
                ledger.ensure_namespace(&path).await?;
            }
            Change::Set(path, key, value) => ledger.set_field(&path, &key, Some(&value)).await?,
            Change::Clear(path, key) => ledger.set_field(&path, &key, None).await?,
            Change::Link(path, target) => ledger.link_namespace(&path, MEMBER_OF, &target).await?,
            Change::Unlink(path, target) => {
                ledger.unlink_namespace(&path, MEMBER_OF, &target).await?
            }
        }
    }
    let updated = with_based_on(&text, &digest(&current(ledger).await?));
    fs::write(file, updated).with_context(|| format!("write {}", file.display()))
}

async fn current(ledger: &Ledger) -> Result<Records> {
    let mut records = Records::new();
    for entry in ledger.namespaces().await? {
        if !matches!(entry.kind, NamespaceKind::Actor | NamespaceKind::Project) {
            continue;
        }
        let fields = ledger.fields(&entry.path).await?;
        let member_of = ledger
            .namespace_links(&entry.path)
            .await?
            .into_iter()
            .filter(|(relation, _)| relation == MEMBER_OF)
            .map(|(_, target)| target)
            .collect();
        records.insert(
            entry.path,
            Node {
                fields,
                member_of,
                members: Vec::new(),
            },
        );
    }
    Ok(records)
}

fn diff(have: &Records, wanted: &Records) -> Vec<Change> {
    let empty = Node::default();
    let mut changes = Vec::new();
    for (path, node) in wanted {
        let current = have.get(path).unwrap_or(&empty);
        if !have.contains_key(path) {
            changes.push(Change::Create(path.clone()));
        }
        for (key, value) in &node.fields {
            if current.fields.get(key) != Some(value) {
                changes.push(Change::Set(path.clone(), key.clone(), value.clone()));
            }
        }
        for key in current.fields.keys() {
            if !node.fields.contains_key(key) {
                changes.push(Change::Clear(path.clone(), key.clone()));
            }
        }
    }
    // A link is wanted if either end lists it. A link neither end lists any
    // more is removed when at least one end is in the file.
    let wanted_links = links(wanted);
    for (member, org) in &wanted_links {
        if !have
            .get(member)
            .is_some_and(|node| node.member_of.contains(org))
        {
            changes.push(Change::Link(member.clone(), org.clone()));
        }
    }
    for (member, org) in links(have) {
        if !wanted_links.contains(&(member.clone(), org.clone()))
            && (wanted.contains_key(&member) || wanted.contains_key(&org))
        {
            changes.push(Change::Unlink(member, org));
        }
    }
    changes
}

/// Every `member-of` link as (member, org), from both sides of the file.
fn links(records: &Records) -> BTreeSet<(NamespacePath, NamespacePath)> {
    let mut links = BTreeSet::new();
    for (path, node) in records {
        for org in &node.member_of {
            links.insert((path.clone(), org.clone()));
        }
        for member in &node.members {
            links.insert((member.clone(), path.clone()));
        }
    }
    links
}

fn is_org(records: &Records, path: &NamespacePath) -> bool {
    records
        .get(path)
        .and_then(|node| node.fields.get("kind"))
        .is_some_and(|kind| kind == "org")
}

/// The records as TOML tables, without the `based_on` header.
fn render(records: &Records) -> String {
    let mut members: BTreeMap<&NamespacePath, Vec<&NamespacePath>> = BTreeMap::new();
    for (path, node) in records {
        for org in node.member_of.iter().filter(|org| is_org(records, org)) {
            members.entry(org).or_default().push(path);
        }
    }
    let mut text = String::new();
    for kind in KINDS {
        for (path, node) in records {
            let Some(name) = name_in(path, kind) else {
                continue;
            };
            let _ = writeln!(text, "[{kind}.{}]", key(name));
            for (field, value) in &node.fields {
                let _ = writeln!(text, "{} = {}", key(field), quote(value));
            }
            let member_of: Vec<&NamespacePath> = node
                .member_of
                .iter()
                .filter(|org| !is_org(records, org))
                .collect();
            write_names(&mut text, LINK_KEY, &member_of);
            write_names(
                &mut text,
                MEMBERS_KEY,
                members.get(path).map_or(&[][..], Vec::as_slice),
            );
            text.push('\n');
        }
    }
    text
}

fn write_names(text: &mut String, key: &str, paths: &[&NamespacePath]) {
    if paths.is_empty() {
        return;
    }
    // ponytail: link lists hold actor names; a non-actor end is written as its
    // full path and would not parse back.
    let names: Vec<String> = paths
        .iter()
        .map(|path| quote(name_in(path, "actor").unwrap_or(path.as_str())))
        .collect();
    let _ = writeln!(text, "{key} = [{}]", names.join(", "));
}

fn parse(text: &str) -> Result<(Option<String>, Records)> {
    let table: toml::Table = text.parse()?;
    let mut based_on = None;
    let mut records = Records::new();
    for (top, value) in table {
        match top.as_str() {
            "based_on" => {
                based_on = Some(
                    value
                        .as_str()
                        .context("based_on: expected a string")?
                        .to_owned(),
                )
            }
            "actor" | "project" => {
                let nodes = value
                    .as_table()
                    .with_context(|| format!("{top}: expected tables like [{top}.name]"))?;
                for (name, body) in nodes {
                    let path: NamespacePath = format!("{top}/{name}")
                        .parse()
                        .with_context(|| format!("{top}.{name}: invalid name"))?;
                    records.insert(path, node(&top, name, body)?);
                }
            }
            other => bail!("unknown top-level key {other:?}; expected based_on, actor, or project"),
        }
    }
    Ok((based_on, records))
}

fn node(kind: &str, name: &str, body: &toml::Value) -> Result<Node> {
    let at = |key: &str| format!("{kind}.{name}.{key}");
    let body = body
        .as_table()
        .with_context(|| format!("{kind}.{name}: expected a table"))?;
    let mut node = Node::default();
    for (key, value) in body {
        if key == LINK_KEY || key == MEMBERS_KEY {
            let items = value
                .as_array()
                .with_context(|| format!("{}: expected a list of actor names", at(key)))?;
            for item in items {
                let target = item
                    .as_str()
                    .with_context(|| format!("{}: expected actor names", at(key)))?;
                let target = format!("actor/{target}")
                    .parse()
                    .with_context(|| format!("{}: invalid actor {target:?}", at(key)))?;
                if key == LINK_KEY {
                    node.member_of.push(target);
                } else {
                    node.members.push(target);
                }
            }
            continue;
        }
        validate_segment(key).with_context(|| format!("{}: invalid field key", at(key)))?;
        let value = value
            .as_str()
            .with_context(|| format!("{}: expected a string", at(key)))?;
        node.fields.insert(key.clone(), value.to_owned());
    }
    Ok(node)
}

/// Short SHA-256 of the rendered records: changes whenever any actor or
/// project record does, and never for ordinary item events.
fn digest(records: &Records) -> String {
    let hash = Sha256::digest(render(records).as_bytes());
    hash.iter().take(8).fold(String::new(), |mut hex, byte| {
        let _ = write!(hex, "{byte:02x}");
        hex
    })
}

/// Replaces the header `based_on` line, leaving every other byte alone.
fn with_based_on(text: &str, digest: &str) -> String {
    let line = format!("based_on = \"{digest}\"");
    let mut replaced = false;
    let mut in_tables = false;
    let mut out = String::new();
    for original in text.split_inclusive('\n') {
        let trimmed = original.trim_start();
        in_tables |= trimmed.starts_with('[');
        if !replaced && !in_tables && trimmed.starts_with("based_on") {
            replaced = true;
            out.push_str(&line);
            out.push('\n');
        } else {
            out.push_str(original);
        }
    }
    if replaced {
        out
    } else {
        format!("{line}\n{out}")
    }
}

fn name_in<'a>(path: &'a NamespacePath, kind: &str) -> Option<&'a str> {
    path.as_str()
        .strip_prefix(kind)
        .and_then(|rest| rest.strip_prefix('/'))
}

fn key(name: &str) -> String {
    if name
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        name.to_owned()
    } else {
        quote(name)
    }
}

fn quote(value: &str) -> String {
    toml::Value::String(value.to_owned()).to_string()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "test assertions")]
mod tests {
    use super::{diff, parse, render, with_based_on};

    #[test]
    fn based_on_is_rewritten_in_place_and_tables_are_left_alone() {
        let text = "# keep\nbased_on = \"old\"\n\n[actor.a]\nbased_on = \"field\" # keep\n";
        assert_eq!(
            with_based_on(text, "new"),
            "# keep\nbased_on = \"new\"\n\n[actor.a]\nbased_on = \"field\" # keep\n"
        );
        assert_eq!(
            with_based_on("[actor.a]\n", "d"),
            "based_on = \"d\"\n[actor.a]\n"
        );
    }

    #[test]
    fn unknown_keys_and_non_string_fields_are_errors() {
        assert!(parse("actors = 1").is_err());
        assert!(parse("[actor.a]\nage = 3").is_err());
        let (based_on, records) =
            parse("based_on = \"x\"\n[actor.a]\nkind = \"person\"\nmember-of = [\"b\"]").unwrap();
        assert_eq!(based_on.as_deref(), Some("x"));
        assert_eq!(records.len(), 1);
    }

    #[test]
    fn membership_reads_from_either_side_and_exports_on_the_org() {
        let (_, have) = parse(
            "[actor.cdint]\nkind = \"org\"\n[actor.quincy]\nmember-of = [\"cdint\"]\n\
             [actor.steve]\nmember-of = [\"cdint\"]\n",
        )
        .unwrap();
        assert!(
            render(&have)
                .contains("[actor.cdint]\nkind = \"org\"\nmembers = [\"quincy\", \"steve\"]\n")
        );
        assert!(!render(&have).contains("member-of"));
        // steve is dropped from the roster; quincy is listed on both sides.
        let (_, wanted) = parse(
            "[actor.cdint]\nkind = \"org\"\nmembers = [\"quincy\"]\n\
             [actor.quincy]\nmember-of = [\"cdint\"]\n",
        )
        .unwrap();
        let changes: Vec<String> = diff(&have, &wanted)
            .iter()
            .map(ToString::to_string)
            .collect();
        assert_eq!(changes, ["- actor/steve member-of actor/cdint"]);
    }
}
