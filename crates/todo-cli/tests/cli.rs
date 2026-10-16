#![allow(
    clippy::indexing_slicing,
    clippy::unwrap_used,
    reason = "integration-test fixtures use direct setup and assertion values"
)]

use std::{
    path::Path,
    process::{Command, Output, Stdio},
};
use tempfile::TempDir;

struct Harness {
    _root: TempDir,
    config: std::path::PathBuf,
    data: std::path::PathBuf,
    work: std::path::PathBuf,
}

impl Harness {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let config = root.path().join("config");
        let data = root.path().join("data");
        let work = root.path().join("work");
        std::fs::create_dir(&work).unwrap();
        Self {
            _root: root,
            config,
            data,
            work,
        }
    }

    fn run(&self, args: &[&str]) -> Output {
        self.run_in(&self.work, args)
    }

    fn run_in(&self, cwd: &Path, args: &[&str]) -> Output {
        self.command_in(cwd, args).output().unwrap()
    }

    fn command_in(&self, cwd: &Path, args: &[&str]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_todo"));
        command
            .args(args)
            .current_dir(cwd)
            .env("TODO_CONFIG_HOME", &self.config)
            .env("TODO_DATA_HOME", &self.data);
        command
    }

    fn ok(&self, args: &[&str]) -> String {
        let output = self.run(args);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    }
}

/// The editor opens the ledger per operation so the CLI is never locked out.
/// That relies on a held `Ledger` locking other processes out and on dropping
/// it releasing the file; Turso shares an open database within one process, so
/// only a second process can observe either.
#[test]
fn a_dropped_ledger_lets_another_process_write() {
    let harness = Harness::new();
    harness.ok(&["init"]);
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let held = runtime
        .block_on(todo_store::Ledger::open(&harness.data.join("ledger.db")))
        .unwrap();
    assert!(!harness.run(&["add", "Locked out"]).status.success());
    drop(held);
    harness.ok(&["add", "Written after release"]);
}

#[test]
fn simultaneous_readers_wait_for_the_local_ledger() {
    let harness = Harness::new();
    harness.ok(&["init"]);
    harness.ok(&["add", "Visible to every reader"]);

    // DI-0007: short-lived human and agent invocations must coexist during
    // stage-one dogfooding instead of leaking Turso's transient WAL lock race.
    let children: Vec<_> = (0..16)
        .map(|_| {
            harness
                .command_in(&harness.work, &["ls"])
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap()
        })
        .collect();
    for child in children {
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("Visible to every reader"));
    }
}

fn id_from_add(output: &str) -> &str {
    output.split_whitespace().nth(1).unwrap()
}

#[test]
fn dogfood_workflow_supports_scope_tree_notes_and_history() {
    let harness = Harness::new();
    harness.ok(&["init"]);
    harness.ok(&[
        "project",
        "attach",
        "todo",
        "--path",
        harness.work.to_str().unwrap(),
    ]);

    let parent = harness.ok(&["add", "Ship v0.1"]);
    let parent = id_from_add(&parent);
    let child = harness.ok(&[
        "add",
        "Exercise the CLI",
        "--parent",
        parent,
        "--tag",
        "topic/dogfood",
    ]);
    let child = id_from_add(&child);
    harness.ok(&["ongoing", child]);
    harness.ok(&["note", child, "First real workflow"]);
    harness.ok(&["cite", child, "docs/architecture.md", "--label", "design"]);

    let tree = harness.ok(&["ls", "--tree"]);
    assert!(tree.contains("Ship v0.1"));
    assert!(tree.contains("  [>]"));
    assert!(tree.contains("topic/dogfood"));

    harness.ok(&["done", child]);
    let default_list = harness.ok(&["ls"]);
    assert!(!default_list.contains("Exercise the CLI"));
    let full_list = harness.ok(&["ls", "--all"]);
    assert!(full_list.contains("Exercise the CLI"));
    let shown = harness.ok(&["show", child, "--history"]);
    assert!(shown.contains("First real workflow"));
    assert!(shown.contains("docs/architecture.md (design)"));
    assert!(shown.contains("item.state-changed"));
}

#[test]
fn commands_before_init_give_a_direct_recovery_instruction() {
    let harness = Harness::new();
    let output = harness.run(&["ls"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("run `todo init`"));
}

#[test]
fn failed_creation_does_not_leave_a_partial_item() {
    let harness = Harness::new();
    harness.ok(&["init"]);
    harness.ok(&[
        "project",
        "attach",
        "todo",
        "--path",
        harness.work.to_str().unwrap(),
    ]);
    harness.ok(&["add", "Valid item"]);

    assert!(
        !harness
            .run(&["add", "Bad tag", "--tag", "INVALID"])
            .status
            .success()
    );
    assert!(
        !harness
            .run(&["add", "Bad parent", "--parent", "deadbeef"])
            .status
            .success()
    );

    let items = harness.ok(&["ls"]);
    assert!(items.contains("Valid item"));
    assert!(!items.contains("Bad tag"));
    assert!(!items.contains("Bad parent"));
}

#[test]
fn focus_workflow_respects_dependencies_defer_dates_and_explains_next() {
    let harness = Harness::new();
    harness.ok(&["init"]);
    let access = harness.ok(&["add", "Get access"]);
    let access = id_from_add(&access);
    let implementation = harness.ok(&["add", "Implement feature"]);
    let implementation = id_from_add(&implementation);
    let future = harness.ok(&["add", "Future task"]);
    let future = id_from_add(&future);

    harness.ok(&["priority", implementation, "critical"]);
    harness.ok(&["due", implementation, "2099-01-01"]);
    harness.ok(&["depends", implementation, "--on", access]);
    harness.ok(&["defer", future, "2999-01-01"]);
    let start_deferred = harness.run(&["start", future]);
    assert!(!start_deferred.status.success());
    assert!(String::from_utf8_lossy(&start_deferred.stderr).contains("clear the defer date"));

    let ready = harness.ok(&["ready"]);
    assert!(ready.contains("Get access"));
    assert!(!ready.contains("Implement feature"));
    assert!(!ready.contains("Future task"));
    let next = harness.ok(&["next", "--explain"]);
    assert!(next.contains("Get access"));
    assert!(next.contains("ready: no incomplete dependencies"));

    let cycle = harness.run(&["depends", access, "--on", implementation]);
    assert!(!cycle.status.success());
    assert!(String::from_utf8_lossy(&cycle.stderr).contains("dependency cycle"));

    harness.ok(&["done", access]);
    let next = harness.ok(&["next", "--explain"]);
    assert!(next.contains("Implement feature"));
    assert!(next.contains("priority: critical"));
    assert!(next.contains("due: 2099-01-01"));

    harness.ok(&["undepends", implementation, "--on", access]);
    harness.ok(&["defer", future, "--clear"]);
    let ready = harness.ok(&["ready"]);
    assert!(ready.contains("Future task"));
}

#[test]
fn repair_commands_correct_views_without_losing_history() {
    let harness = Harness::new();
    harness.ok(&["init"]);
    let parent = harness.ok(&["--project", "first", "add", "Parent"]);
    let parent = id_from_add(&parent);
    let child = harness.ok(&[
        "--project",
        "first",
        "add",
        "Chlid typo",
        "--parent",
        parent,
        "--tag",
        "topic/temporary",
    ]);
    let child = id_from_add(&child);

    harness.ok(&["edit", child, "Child corrected"]);
    harness.ok(&["tag", "rm", child, "topic/temporary"]);
    let guarded = harness.run(&["project", "move", child, "second"]);
    assert!(!guarded.status.success());
    harness.ok(&["parent", child, "--clear"]);
    harness.ok(&["project", "move", child, "second"]);

    let first = harness.ok(&["--project", "first", "ls", "--all"]);
    assert!(!first.contains("Child corrected"));
    let second = harness.ok(&["--project", "second", "ls", "--all"]);
    assert!(second.contains("Child corrected"));
    assert!(!second.contains("topic/temporary"));
    let history = harness.ok(&["show", child, "--history"]);
    assert!(history.contains("item.title-changed"));
    assert!(history.contains("item.parent-cleared"));
    assert!(history.contains("item.project-changed"));
    assert!(history.contains("namespace.removed"));
}

#[test]
fn markdown_import_is_dry_runnable_and_exports_all_fidelity_tiers() {
    let harness = Harness::new();
    harness.ok(&["init"]);
    let input = harness.work.join("tasks.md");
    std::fs::write(
        &input,
        "# Work\n\n- [>] Plan release\n  - [ ] Implement feature\n  - [x] Record decision\n",
    )
    .unwrap();

    let dry = harness.ok(&[
        "--project",
        "import",
        "import",
        "markdown",
        input.to_str().unwrap(),
        "--dry-run",
    ]);
    assert!(dry.contains("- [>] Plan release"));
    // DI-0006: a dry-run import writes nothing, so the project namespace
    // does not exist yet; listing it is an empty view, not an error.
    assert_eq!(
        harness.ok(&["--project", "import", "ls", "--all"]),
        "No items.\n"
    );

    harness.ok(&[
        "--project",
        "import",
        "import",
        "markdown",
        input.to_str().unwrap(),
        "--cite-source",
    ]);
    let tree = harness.ok(&["--project", "import", "ls", "--tree", "--all"]);
    assert!(tree.contains("[>]"));
    assert!(tree.contains("  [ ]"));
    assert!(tree.contains("  [x]"));

    let markdown = harness.work.join("export.md");
    let json = harness.work.join("export.json");
    let graph = harness.work.join("todo-graph.json");
    let cbor = harness.work.join("events.cbor");
    harness.ok(&[
        "--project",
        "import",
        "export",
        "markdown",
        "--all",
        "--output",
        markdown.to_str().unwrap(),
    ]);
    harness.ok(&[
        "--project",
        "import",
        "export",
        "json",
        "--all",
        "--output",
        json.to_str().unwrap(),
    ]);
    harness.ok(&[
        "export",
        "graph",
        "--global",
        "--all",
        "--output",
        graph.to_str().unwrap(),
    ]);
    harness.ok(&["export", "cbor", "--output", cbor.to_str().unwrap()]);

    assert!(
        std::fs::read_to_string(markdown)
            .unwrap()
            .contains("  - [x] Record decision")
    );
    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(json).unwrap()).unwrap();
    assert_eq!(json.as_array().unwrap().len(), 3);
    let graph: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(graph).unwrap()).unwrap();
    assert_eq!(graph["format"], "todo-graph-v1");
    assert_eq!(graph["items"].as_array().unwrap().len(), 3);
    assert_eq!(graph["projects"][0]["path"], "project/import");
    let bytes = std::fs::read(cbor).unwrap();
    let mut decoder = minicbor::Decoder::new(&bytes);
    assert!(decoder.array().unwrap().unwrap() >= 7);

    let invalid = harness.work.join("invalid.md");
    std::fs::write(&invalid, "  - [ ] orphan\n").unwrap();
    let failed = harness.run(&[
        "--project",
        "import",
        "import",
        "markdown",
        invalid.to_str().unwrap(),
    ]);
    assert!(!failed.status.success());
    let items = harness.ok(&["--project", "import", "ls", "--all"]);
    assert_eq!(items.lines().count(), 3);
}

#[test]
fn backup_is_new_and_markdown_restorable_by_scope() {
    let source = Harness::new();
    source.ok(&["init"]);
    source.ok(&["add", "Unscoped root"]);
    let project_root = source.ok(&["--project", "alpha", "add", "Project root"]);
    let project_root = id_from_add(&project_root);
    let child = source.ok(&[
        "--project",
        "alpha",
        "add",
        "Completed child",
        "--parent",
        project_root,
    ]);
    source.ok(&["done", id_from_add(&child)]);

    let directory = source.work.join("backup-generation-1");
    source.ok(&["backup", directory.to_str().unwrap()]);
    assert!(directory.join("events.cbor").is_file());
    assert!(directory.join("items.json").is_file());
    assert!(directory.join("global-inbox.md").is_file());
    assert!(directory.join("project--alpha.md").is_file());
    let instructions = std::fs::read_to_string(directory.join("RESTORE.md")).unwrap();
    assert!(instructions.contains("todo import markdown global-inbox.md"));
    assert!(instructions.contains("todo --project project/alpha import markdown"));

    let repeated = source.run(&["backup", directory.to_str().unwrap()]);
    assert!(!repeated.status.success());

    let restored = Harness::new();
    restored.ok(&["init"]);
    restored.ok(&[
        "import",
        "markdown",
        directory.join("global-inbox.md").to_str().unwrap(),
    ]);
    restored.ok(&[
        "--project",
        "alpha",
        "import",
        "markdown",
        directory.join("project--alpha.md").to_str().unwrap(),
    ]);
    assert!(restored.ok(&["ls", "--all"]).contains("Unscoped root"));
    let project = restored.ok(&["--project", "alpha", "ls", "--tree", "--all"]);
    assert!(project.contains("Project root"));
    assert!(project.contains("  [x]"));
    assert!(project.contains("Completed child"));
}

#[test]
fn exact_event_log_round_trips_between_independent_ledgers() {
    let source = Harness::new();
    source.ok(&["init"]);
    let parent = source.ok(&["--project", "sync", "add", "Transfer exact events"]);
    let parent = id_from_add(&parent);
    let child = source.ok(&[
        "--project",
        "sync",
        "add",
        "Preserve note and hierarchy",
        "--parent",
        parent,
        "--tag",
        "topic/sync",
    ]);
    let child = id_from_add(&child);
    source.ok(&["note", child, "This must not become a Markdown import"]);
    source.ok(&["priority", child, "high"]);

    let log = source.work.join("events.jsonl");
    source.ok(&["export", "log", "--output", log.to_str().unwrap()]);
    let first_line = std::fs::read_to_string(&log).unwrap();
    assert!(first_line.contains("\"cid\""));
    assert!(first_line.ends_with('\n'));

    let target = Harness::new();
    target.ok(&["init"]);
    target.ok(&["import", "log", log.to_str().unwrap()]);
    assert!(
        target
            .ok(&["--project", "sync", "ls", "--tree", "--all"])
            .contains("Preserve note and hierarchy")
    );
    let shown = target.ok(&["show", child, "--history"]);
    assert!(shown.contains("This must not become a Markdown import"));
    assert!(shown.contains("priority: high"));
    assert_eq!(
        target.ok(&["import", "log", log.to_str().unwrap()]),
        "Imported 0 exact events.\n"
    );
}

fn git(cwd: &Path, args: &[&str]) {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {}: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn explicit_git_sync_converges_two_independent_ledgers() {
    let remote_root = tempfile::tempdir().unwrap();
    let remote = remote_root.path().join("todo-ledger.git");
    git(
        remote_root.path(),
        &["init", "--bare", remote.to_str().unwrap()],
    );

    let source = Harness::new();
    let source_repo = source.work.join("sync-repo");
    git(
        source.work.as_path(),
        &[
            "clone",
            remote.to_str().unwrap(),
            source_repo.to_str().unwrap(),
        ],
    );
    git(&source_repo, &["config", "user.name", "Todo Test"]);
    git(
        &source_repo,
        &["config", "user.email", "todo-test@example.invalid"],
    );
    source.ok(&["init"]);
    source.ok(&["sync", "init", "--repo", source_repo.to_str().unwrap()]);
    let first = source.ok(&["add", "Created on source"]);
    let first = id_from_add(&first);
    source.ok(&["sync"]);

    let target = Harness::new();
    let target_repo = target.work.join("sync-repo");
    git(
        target.work.as_path(),
        &[
            "clone",
            remote.to_str().unwrap(),
            target_repo.to_str().unwrap(),
        ],
    );
    git(&target_repo, &["config", "user.name", "Todo Test"]);
    git(
        &target_repo,
        &["config", "user.email", "todo-test@example.invalid"],
    );
    target.ok(&["init"]);
    target.ok(&["sync", "init", "--repo", target_repo.to_str().unwrap()]);
    target.ok(&["sync"]);
    assert!(target.ok(&["show", first]).contains("Created on source"));

    let second = target.ok(&["add", "Created on target"]);
    let second = id_from_add(&second);
    source.ok(&["add", "Created while target was offline"]);
    source.ok(&["sync"]);
    target.ok(&["sync"]);
    source.ok(&["sync"]);

    assert!(source.ok(&["show", second]).contains("Created on target"));
    assert!(
        target
            .ok(&["ls", "--all"])
            .contains("Created while target was offline")
    );
    assert!(target_repo.join("ledger.toml").is_file());
    assert!(target_repo.join("events.jsonl").is_file());
    assert!(
        std::fs::read_to_string(target_repo.join(".gitattributes"))
            .unwrap()
            .contains("events.jsonl merge=union")
    );
}

/// Returns the proquint identifier a listing prints for a task.
///
/// Identifiers are printed as hyphenated quints such as `lusab-babad`, and every
/// command takes one back, so the tests reference tasks the way a person reading
/// the terminal would.
fn spoken_id(listing: &str, title: &str) -> String {
    listing
        .lines()
        .find(|line| line.contains(title))
        .and_then(|line| {
            line.split_whitespace()
                .find(|token| {
                    let parts: Vec<&str> = token.split('-').collect();
                    parts.len() >= 2
                        && parts.iter().all(|part| {
                            part.len() == 5 && part.chars().all(|c| c.is_ascii_lowercase())
                        })
                })
                .map(str::to_owned)
        })
        .unwrap()
}

#[test]
fn deleting_an_item_is_confirmed_then_reversible() {
    let harness = Harness::new();
    harness.ok(&["init"]);
    harness.ok(&["add", "Accidental duplicate"]);
    let listed = harness.ok(&["ls"]);
    let id = spoken_id(&listed, "Accidental duplicate");

    // Without --force nothing is removed, so a mistyped delete is harmless.
    let dry_run = harness.ok(&["rm", &id]);
    assert!(dry_run.contains("Would delete"));
    assert!(harness.ok(&["ls"]).contains("Accidental duplicate"));

    let deleted = harness.ok(&["rm", &id, "--force"]);
    assert!(deleted.contains("Deleted"));
    assert!(!harness.ok(&["ls"]).contains("Accidental duplicate"));
    assert!(harness.ok(&["deleted"]).contains("Accidental duplicate"));

    let restored = harness.ok(&["restore", &id]);
    assert!(restored.contains("Restored"));
    assert!(harness.ok(&["ls"]).contains("Accidental duplicate"));
    assert!(harness.ok(&["deleted"]).contains("No deleted items."));
}

#[test]
fn typed_relations_are_set_shown_and_withdrawn() {
    let harness = Harness::new();
    harness.ok(&["init"]);
    harness.ok(&["add", "Graph specification"]);
    harness.ok(&["add", "Draw the graph"]);
    let listed = harness.ok(&["ls"]);
    let spec = spoken_id(&listed, "Graph specification");
    let work = spoken_id(&listed, "Draw the graph");

    harness.ok(&["relate", &work, "implements", &spec]);
    let shown = harness.ok(&["show", &work]);
    assert!(shown.contains("implements"));
    // A relation is not parenthood, so the item keeps no parent.
    assert!(!shown.contains("parent:"));

    let inverse = harness.ok(&["related-to", &spec]);
    assert!(inverse.contains("implements"));

    harness.ok(&["unrelate", &work, "implements", &spec]);
    assert!(!harness.ok(&["show", &work]).contains("implements"));
}

#[test]
fn a_void_item_leaves_every_listing_and_comes_back_on_request() {
    let harness = Harness::new();
    harness.ok(&["init"]);
    harness.ok(&["add", "Real work"]);
    harness.ok(&["add", "Mistyped capture"]);
    let listed = harness.ok(&["ls"]);
    let mistake = spoken_id(&listed, "Mistyped capture");

    harness.ok(&["void", &mistake]);
    // Even asking for every state leaves it out: a void item was never work.
    assert!(!harness.ok(&["ls", "--all"]).contains("Mistyped capture"));
    assert!(harness.ok(&["ls"]).contains("Real work"));
    assert!(harness.ok(&["voided"]).contains("Mistyped capture"));

    harness.ok(&["reopen", &mistake]);
    assert!(harness.ok(&["ls"]).contains("Mistyped capture"));
    assert!(harness.ok(&["voided"]).contains("No void items."));
}

#[test]
fn typed_entry_attributes_items_to_actors() {
    let harness = Harness::new();
    harness.ok(&["init"]);
    let config = harness.config.join("config.toml");
    let existing = std::fs::read_to_string(&config).unwrap();
    std::fs::write(&config, format!("me = \"quincy\"\n{existing}")).unwrap();

    harness.ok(&["set", "quincy", "kind=person", "gh=Qu1ncyRy4n"]);
    harness.ok(&["set", "cdint", "kind=org", "member=quincy"]);
    let parent = harness.ok(&["add", "Sync work"]);
    let parent = id_from_add(&parent);
    let added = harness.ok(&[
        "add",
        "Fix",
        "sync",
        "race",
        "#work/gui",
        "by:me",
        "due:2026-10-20",
        "p1",
        "--parent",
        parent,
    ]);
    assert!(added.contains("Added") && added.contains("Fix sync race\n"));
    let id = id_from_add(&added).to_owned();
    harness.ok(&[
        "set",
        &id,
        "-tag:work/gui",
        "to:steve",
        &format!("see:{parent}"),
    ]);

    let shown = harness.ok(&["show", &id]);
    for line in [
        "author: quincy",
        "assignee: steve",
        "due: 2026-10-20",
        "relates-to:",
    ] {
        assert!(shown.contains(line), "missing {line:?} in\n{shown}");
    }
    assert!(!shown.contains("tags:"));
    assert!(harness.ok(&["show", parent]).contains("relates-to:"));

    let actor = harness.ok(&["actor", "cdint"]);
    assert!(actor.contains("kind: org") && actor.contains("member: actor/quincy"));
    assert!(harness.ok(&["actor", "me"]).contains("author:"));

    // Fields belong to actors and projects, never to items.
    assert!(!harness.run(&["set", &id, "gh=x"]).status.success());
}
