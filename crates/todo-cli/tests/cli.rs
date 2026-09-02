use std::{
    path::Path,
    process::{Command, Output},
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
        Command::new(env!("CARGO_BIN_EXE_todo"))
            .args(args)
            .current_dir(cwd)
            .env("TODO_CONFIG_HOME", &self.config)
            .env("TODO_DATA_HOME", &self.data)
            .output()
            .unwrap()
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
fn focus_workflow_respects_blockers_defer_dates_and_explains_next() {
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
    harness.ok(&["block", implementation, "--on", access]);
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
    assert!(next.contains("ready: no incomplete blockers"));

    let cycle = harness.run(&["block", access, "--on", implementation]);
    assert!(!cycle.status.success());
    assert!(String::from_utf8_lossy(&cycle.stderr).contains("dependency cycle"));

    harness.ok(&["done", access]);
    let next = harness.ok(&["next", "--explain"]);
    assert!(next.contains("Implement feature"));
    assert!(next.contains("priority: critical"));
    assert!(next.contains("due: 2099-01-01"));

    harness.ok(&["unblock", implementation, "--on", access]);
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
    harness.ok(&["export", "cbor", "--output", cbor.to_str().unwrap()]);

    assert!(
        std::fs::read_to_string(markdown)
            .unwrap()
            .contains("  - [x] Record decision")
    );
    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(json).unwrap()).unwrap();
    assert_eq!(json.as_array().unwrap().len(), 3);
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
