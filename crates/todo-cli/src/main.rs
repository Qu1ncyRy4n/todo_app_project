mod manifest;

use anyhow::{Context, Result, anyhow, bail};
use chrono::{Local, NaiveDate};
use clap::{Args, Parser, Subcommand, ValueEnum};
use serde::Serialize;
use std::{
    collections::{BTreeSet, HashMap, HashSet},
    env, fs,
    path::{Path, PathBuf},
    process::Command as ProcessCommand,
};
use todo_core::entry::{Directive, DirectiveKind, EntrySyntax, parse_entry};
use todo_core::{ItemId, ItemState, LogLine, NamespaceKind, NamespacePath, Priority, RelationKind};
use todo_store::{
    AppConfig, AppPaths, BatchItem, EventRecord, ItemScope, Ledger, MEMBER_OF, StoredItem,
};

#[derive(Parser)]
#[command(name = "todo", version, about = "A global, local-first todo ledger")]
struct Cli {
    /// Override the project inferred from the current directory.
    #[arg(long, global = true, value_name = "PROJECT")]
    project: Option<String>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Initialize the global configuration and ledger. This does not install the binary.
    Init,
    /// Capture a new inbox item.
    Add(AddArgs),
    /// List items in the inferred or selected project.
    #[command(alias = "list")]
    Ls(ListArgs),
    /// Show an item, its notes, citations, and optionally its event history.
    Show {
        item: String,
        #[arg(long)]
        history: bool,
    },
    /// Move an item to active.
    #[command(alias = "ongoing")]
    Start { item: String },
    /// Complete an item.
    Done { item: String },
    /// Cancel an item.
    Cancel { item: String },
    /// Reopen a done or canceled item as active.
    Reopen { item: String },
    /// Relate an item to another, across projects. Relations never block readiness.
    Relate {
        item: String,
        /// contributes-to or implements.
        relation: String,
        target: String,
    },
    /// Withdraw a typed relation.
    Unrelate {
        item: String,
        relation: String,
        target: String,
    },
    /// Show what relates to an item, the derived inverse view.
    RelatedTo { item: String },
    /// Set or clear how hard an item is, from 1 to 5. Pass "clear" to remove it.
    Difficulty { item: String, value: String },
    /// Set or clear an effort estimate in minutes. Pass "clear" to remove it.
    Estimate { item: String, value: String },
    /// Mark an item as created in error, hiding it from every listing.
    ///
    /// Distinct from cancel, which records work deliberately abandoned. Reverse it
    /// with `todo reopen` or by setting any other state.
    Void { item: String },
    /// List items marked void.
    Voided,
    /// Delete an item, hiding it from every view. Reversible with `todo restore`.
    #[command(alias = "delete")]
    Rm {
        item: String,
        /// Confirm the deletion. Without it, the item is only described.
        #[arg(long)]
        force: bool,
    },
    /// Restore a deleted item.
    Restore { item: String },
    /// List deleted items.
    Deleted,
    /// Add an append-only note.
    Note { item: String, body: String },
    /// Add a file path, URL, or other citation.
    Cite {
        item: String,
        target: String,
        #[arg(long)]
        label: Option<String>,
    },
    /// Set an item's structural parent.
    Parent {
        child: String,
        parent: Option<String>,
        #[arg(long, conflicts_with = "parent")]
        clear: bool,
    },
    /// Correct an item's title while preserving history.
    Edit { item: String, title: String },
    /// Set execution priority (low/p3, normal/p2, high/p1, critical/p0).
    Priority { item: String, priority: String },
    /// Set or clear a due date.
    Due(DateArgs),
    /// Hide an item from ready work until a date.
    Defer(DateArgs),
    /// Declare an active blocker. It remains active until explicitly removed.
    Block {
        item: String,
        #[arg(long = "on")]
        blocker: String,
    },
    /// Remove an active blocking relationship.
    Unblock {
        item: String,
        #[arg(long = "on")]
        blocker: String,
    },
    /// Declare a planned prerequisite; it clears automatically once done.
    Depends {
        item: String,
        #[arg(long = "on")]
        dependency: String,
    },
    /// Remove a planned prerequisite.
    Undepends {
        item: String,
        #[arg(long = "on")]
        dependency: String,
    },
    /// Manage ordered focus sequences that reference existing tasks.
    Focus {
        #[command(subcommand)]
        command: FocusCommand,
    },
    /// Track append-only work sessions for an item.
    Session {
        #[command(subcommand)]
        command: SessionCommand,
    },
    /// List open, non-deferred items with no incomplete blockers.
    Ready(FocusArgs),
    /// Select the highest-ranked ready item.
    Next {
        #[command(flatten)]
        focus: FocusArgs,
        #[arg(long)]
        explain: bool,
    },
    /// Manage project scopes and working-directory mappings.
    Project(ProjectArgs),
    /// Manage hierarchical namespaces attached to items.
    Tag(TagArgs),
    /// Apply typed words to an item, or key=value fields to an actor or project.
    ///
    /// `todo set kohil p1 to:steve -tag:old` changes an item. `todo set quincy
    /// kind=person gh=Qu1ncyRy4n` sets fields on actor/quincy, creating it on
    /// first use; `email=` clears one. `todo set cdint member=quincy` records
    /// that quincy is a member of cdint.
    Set {
        /// An item ID, an actor name, or a full path such as project/todo.
        target: String,
        #[arg(required = true, num_args = 1.., allow_hyphen_values = true)]
        words: Vec<String>,
    },
    /// List actors, or show one with its fields, memberships, and items by role.
    Actor { name: Option<String> },
    /// Edit actor and project records in bulk as a TOML file.
    Manifest {
        #[command(subcommand)]
        command: ManifestCommand,
    },
    /// Find `todo:<id>` references in files and show each item's state.
    ///
    /// Uses `git grep`, so ignored files are skipped; outside a checkout every
    /// file is searched. `--check` exits non-zero on closed or unknown items.
    Scan {
        #[arg(default_value = ".")]
        paths: Vec<PathBuf>,
        #[arg(long)]
        check: bool,
    },
    /// Import a Markdown checkbox tree in one transaction.
    Import(ImportArgs),
    /// Export a human view, rich view, or exact event archive.
    Export(ExportArgs),
    /// Create a new tiered recovery directory for dogfooding.
    Backup { directory: PathBuf },
    /// Synchronize exact events through an explicitly configured Git repository.
    Sync(SyncArgs),
}

#[derive(Subcommand)]
enum FocusCommand {
    Create { title: String },
    Add { sequence: String, item: String },
    Ls,
}

#[derive(Subcommand)]
enum SessionCommand {
    Start { item: String },
    Stop { item: String },
    Ls { item: String },
}

#[derive(Args)]
struct AddArgs {
    /// Title words, mixed with typed words: tag:a/b, in:todo, by:me, to:steve,
    /// as:reviewer/steve, due:2026-10-20, defer:…, p1, est:30m, dep:<id>,
    /// see:<id>, and #tag, @actor, +project. A quoted phrase is always title,
    /// so `"Read by:me"` keeps a typed-looking word.
    #[arg(required = true, num_args = 1..)]
    title: Vec<String>,
    /// Attach one or more hierarchical namespace paths.
    #[arg(long = "tag", value_name = "PATH")]
    tags: Vec<String>,
    /// Set a parent using a full or unique trailing item ID.
    #[arg(long)]
    parent: Option<String>,
}

#[derive(Args)]
struct ListArgs {
    /// Render parent/child structure.
    #[arg(long)]
    tree: bool,
    /// Include done and canceled items.
    #[arg(long)]
    all: bool,
    /// Ignore project inference and list across the global ledger.
    #[arg(long, conflicts_with = "project")]
    global: bool,
}

#[derive(Args)]
struct DateArgs {
    item: String,
    /// Date in YYYY-MM-DD form.
    date: Option<String>,
    /// Remove the current date.
    #[arg(long, conflicts_with = "date")]
    clear: bool,
}

#[derive(Args)]
struct FocusArgs {
    /// Ignore project inference and consider the whole ledger.
    #[arg(long, conflicts_with = "project")]
    global: bool,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum ImportFormat {
    Markdown,
    Log,
}

#[derive(Args)]
struct ImportArgs {
    format: ImportFormat,
    file: PathBuf,
    /// Parse and display the result without writing anything.
    #[arg(long)]
    dry_run: bool,
    /// Add the canonical input path as a citation on every imported item.
    #[arg(long)]
    cite_source: bool,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum ExportFormat {
    Markdown,
    Json,
    /// Export a versioned graph-viewer projection.
    Graph,
    Cbor,
    Log,
}

#[derive(Serialize)]
struct GraphExport<'a> {
    format: &'static str,
    items: &'a [StoredItem],
    projects: Vec<GraphProject>,
}

#[derive(Serialize)]
struct GraphProject {
    id: String,
    path: String,
}

#[derive(Args)]
struct ExportArgs {
    format: ExportFormat,
    /// Write to a file instead of standard output. Required for CBOR.
    #[arg(long)]
    output: Option<PathBuf>,
    /// Include done and canceled items in Markdown and JSON views.
    #[arg(long)]
    all: bool,
    /// Export the current view across all project scopes.
    #[arg(long, conflicts_with = "project")]
    global: bool,
}

#[derive(Args)]
struct ProjectArgs {
    #[command(subcommand)]
    command: ProjectCommand,
}

#[derive(Subcommand)]
enum ProjectCommand {
    /// Create a project namespace. `todo` is shorthand for `project/todo`.
    Add { name: String },
    /// Map a directory (default: current directory) to a project.
    Attach {
        name: String,
        #[arg(long, default_value = ".")]
        path: PathBuf,
    },
    /// Record a project's Git repository so any checkout of it, on any
    /// machine, infers the project. URL defaults to this checkout's `origin`.
    Repo { name: String, url: Option<String> },
    /// Set the fallback project used outside attached directories.
    Default { name: String },
    /// List project namespaces and attached roots.
    Ls,
    /// Move an unrelated item to a project, or to the global inbox.
    Move {
        item: String,
        #[arg(value_name = "PROJECT")]
        target: Option<String>,
        #[arg(long, conflicts_with = "target")]
        global: bool,
    },
}

#[derive(Subcommand)]
enum ManifestCommand {
    /// Write every actor and project record to FILE, or print it.
    Export { file: Option<PathBuf> },
    /// Show and write the changes FILE makes. A removed line clears that
    /// field; refuses if the records changed since the file was exported.
    Apply {
        file: PathBuf,
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Args)]
struct TagArgs {
    #[command(subcommand)]
    command: TagCommand,
}

#[derive(Args)]
struct SyncArgs {
    #[command(subcommand)]
    command: Option<SyncCommand>,
}

#[derive(Subcommand)]
enum SyncCommand {
    /// Configure a dedicated local Git checkout for exact event synchronization.
    Init {
        #[arg(long)]
        repo: PathBuf,
    },
}

#[derive(Subcommand)]
enum TagCommand {
    /// Attach a namespace path such as `topic/rust/turso` to an item.
    Add { item: String, path: String },
    /// Remove a namespace assignment without deleting the namespace.
    #[command(alias = "rm")]
    Remove { item: String, path: String },
    /// Rename a namespace while preserving its stable identity and old alias.
    Mv { current: String, next: String },
    /// List namespaces.
    Ls {
        #[arg(long)]
        tree: bool,
    },
}

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("error: {error:#}");
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    let cli = Cli::parse();
    let paths = AppPaths::discover()?;
    if matches!(cli.command, Command::Init) {
        paths.create_dirs()?;
        let mut config = AppConfig::load(&paths.config())?;
        let actor_id = config.ensure_actor_id().to_owned();
        config.save(&paths.config())?;
        let _ledger = Ledger::open_with_actor(&paths.database(), actor_id).await?;
        println!(
            "Initialized global ledger at {}",
            paths.database().display()
        );
        return Ok(());
    }
    if !paths.database().exists() {
        bail!("global ledger is not initialized; run `todo init`");
    }

    let mut config = AppConfig::load(&paths.config())?;
    let actor_id = match config.actor_id.clone() {
        Some(actor_id) => actor_id,
        None => {
            let actor_id = config.ensure_actor_id().to_owned();
            config.save(&paths.config())?;
            actor_id
        }
    };
    let ledger = Ledger::open_with_actor(&paths.database(), actor_id).await?;
    let cwd = env::current_dir().context("determine current directory")?;

    match cli.command {
        Command::Init => bail!("initialization should return before command dispatch"),
        Command::Add(args) => {
            let entry = parse_entry(&args.title, entry_syntax(&config, false))?;
            if entry.title.is_empty() {
                bail!("a new item needs a title besides its typed words");
            }
            if entry.directives.iter().any(|directive| directive.remove) {
                bail!("a new item has nothing to remove; drop the leading -");
            }
            let mut project =
                resolve_project(cli.project.as_deref(), &config, &ledger, &cwd).await?;
            let mut tags = args
                .tags
                .iter()
                .map(|tag| tag.parse())
                .collect::<Result<Vec<NamespacePath>, _>>()?;
            // Tags and the project go in with the creation event; the rest follow it.
            let mut rest = Vec::new();
            for directive in entry.directives {
                match directive.kind {
                    DirectiveKind::Tag(path) => tags.push(path),
                    DirectiveKind::Project(name) => project = Some(project_path(&name)?),
                    _ => rest.push(directive),
                }
            }
            let item = ledger
                .add_item_with(
                    &entry.title,
                    project.as_ref(),
                    &tags,
                    args.parent.as_deref(),
                )
                .await?;
            println!(
                "Added {} [inbox] {}",
                short_id(item.item.id),
                item.item.title
            );
            apply_to_item(&ledger, &config, &item.item.id.to_string(), &rest).await?;
        }
        Command::Set { target, words } => match resolve_target(&ledger, &config, &target).await? {
            Target::Item(id) => {
                // Fields are read so they can be refused with a reason.
                let entry = parse_entry(&words, entry_syntax(&config, true))?;
                if !entry.title.is_empty() {
                    bail!(
                        "unrecognized words {:?}; `todo edit` changes a title",
                        entry.title
                    );
                }
                apply_to_item(&ledger, &config, &id, &entry.directives).await?;
            }
            Target::Node(path) => {
                let entry = parse_entry(&words, entry_syntax(&config, true))?;
                if !entry.title.is_empty() {
                    bail!("unrecognized words {:?}; expected key=value", entry.title);
                }
                apply_to_node(&ledger, &config, &path, &entry.directives).await?;
            }
        },
        Command::Actor { name: None } => {
            for entry in ledger.namespaces().await? {
                if entry.kind == NamespaceKind::Actor {
                    let fields = ledger.fields(&entry.path).await?;
                    let kind = fields.get("kind").map_or("", String::as_str);
                    println!("{}  {kind}", entry.path);
                }
            }
        }
        Command::Actor { name: Some(name) } => {
            show_actor(&ledger, &actor_path(&config, &name)?).await?
        }
        Command::Manifest { command } => match command {
            ManifestCommand::Export { file } => manifest::export(&ledger, file.as_deref()).await?,
            ManifestCommand::Apply { file, dry_run } => {
                manifest::apply(&ledger, &file, dry_run).await?
            }
        },
        Command::Scan { paths, check } => {
            let mut stale = 0_usize;
            let mut total = 0_usize;
            for root in &paths {
                for (file, line, reference) in todo_references(root)? {
                    total += 1;
                    let id = reference.trim_start_matches("todo:");
                    let status = match ledger.get_item(id).await {
                        Ok(stored) => {
                            if !matches!(stored.item.state, ItemState::Inbox | ItemState::Active) {
                                stale += 1;
                            }
                            format!("{} {}", state_mark(stored.item.state), stored.item.title)
                        }
                        Err(_) => {
                            stale += 1;
                            "unknown".to_owned()
                        }
                    };
                    let shown = root.join(file);
                    let shown = shown.strip_prefix(".").unwrap_or(&shown);
                    println!("{}:{line} {reference} {status}", shown.display());
                }
            }
            println!("{total} references, {stale} closed or unknown");
            if check && stale > 0 {
                bail!("{stale} references point at closed or unknown items");
            }
        }
        Command::Ls(args) => {
            let project = if args.global {
                None
            } else {
                resolve_project(cli.project.as_deref(), &config, &ledger, &cwd).await?
            };
            let scope = match (args.global, project.as_ref()) {
                (true, _) => ItemScope::All,
                (false, Some(project)) => ItemScope::Project(project),
                (false, None) => ItemScope::Unscoped,
            };
            let items = ledger.list_items(scope, args.all).await?;
            if items.is_empty() {
                println!("No items.");
            } else if args.tree {
                print_tree(&items);
            } else {
                for item in &items {
                    print_item(item, 0);
                }
            }
        }
        Command::Show { item, history } => show(&ledger, &item, history).await?,
        Command::Start { item } => print_state(&ledger.set_state(&item, ItemState::Active).await?),
        Command::Done { item } => print_state(&ledger.set_state(&item, ItemState::Done).await?),
        Command::Cancel { item } => {
            print_state(&ledger.set_state(&item, ItemState::Canceled).await?)
        }
        Command::Reopen { item } => print_state(&ledger.set_state(&item, ItemState::Active).await?),
        Command::Relate {
            item,
            relation,
            target,
        } => {
            let kind: RelationKind = relation.parse()?;
            let updated = ledger.add_relation(&item, kind, &target).await?;
            println!("{} {kind} {target}", updated.item.id);
            println!("A relation crosses projects and never blocks readiness.");
        }
        Command::Unrelate {
            item,
            relation,
            target,
        } => {
            let kind: RelationKind = relation.parse()?;
            let updated = ledger.remove_relation(&item, kind, &target).await?;
            println!("{} no longer {kind} {target}", updated.item.id);
        }
        Command::RelatedTo { item } => {
            let inverse = ledger.related_from(&item).await?;
            if inverse.is_empty() {
                println!("Nothing relates to {item}.");
            }
            for (relation, source) in inverse {
                println!("{source} {relation} {item}");
            }
        }
        Command::Difficulty { item, value } => {
            let parsed = parse_clearable(&value, "difficulty")?;
            let updated = ledger.set_difficulty(&item, parsed).await?;
            println!(
                "{} difficulty {}",
                updated.item.id,
                describe_optional(updated.item.difficulty)
            );
        }
        Command::Estimate { item, value } => {
            let parsed = parse_clearable(&value, "estimate")?;
            let updated = ledger.set_estimate(&item, parsed).await?;
            println!(
                "{} estimate {}",
                updated.item.id,
                describe_optional(updated.item.estimate_minutes)
            );
        }
        Command::Void { item } => {
            let updated = ledger.set_state(&item, ItemState::Void).await?;
            println!("{} marked void", short_id(updated.item.id));
            println!("It is hidden from every listing. `todo reopen` brings it back.");
        }
        Command::Voided => {
            let voided = ledger.void_items().await?;
            if voided.is_empty() {
                println!("No void items.");
            }
            for item in voided {
                print_state(&item);
            }
        }
        Command::Rm { item, force } => {
            // A delete without --force reports what would go and changes nothing, so
            // the destructive step is always a deliberate second command.
            let found = ledger.get_item(&item).await?;
            if force {
                let deleted = ledger.delete_item(&item).await?;
                println!("Deleted {} {}", deleted.id, deleted.title);
                println!("Restore it with: todo restore {}", deleted.id);
            } else {
                println!("Would delete {} {}", found.item.id, found.item.title);
                println!(
                    "State {} · {} dependencies · {} active blockers · {} tags",
                    found.item.state,
                    found.dependencies.len(),
                    found.blocked_by.len(),
                    found.namespaces.len()
                );
                println!("Re-run with --force to delete it. Deletion is reversible.");
            }
        }
        Command::Restore { item } => {
            let restored = ledger.restore_item(&item).await?;
            println!("Restored {} {}", restored.item.id, restored.item.title);
        }
        Command::Deleted => {
            let deleted = ledger.deleted_items().await?;
            if deleted.is_empty() {
                println!("No deleted items.");
            }
            for entry in deleted {
                println!(
                    "{} {} (deleted {})",
                    entry.id, entry.title, entry.deleted_at
                );
            }
        }
        Command::Note { item, body } => {
            ledger.add_note(&item, &body).await?;
            println!("Note added to {item}");
        }
        Command::Cite {
            item,
            target,
            label,
        } => {
            ledger
                .add_citation(&item, &target, label.as_deref())
                .await?;
            println!("Citation added to {item}");
        }
        Command::Parent {
            child,
            parent,
            clear,
        } => match (parent, clear) {
            (_, true) => {
                ledger.clear_parent(&child).await?;
                println!("Cleared parent of {child}");
            }
            (Some(parent), false) => {
                ledger.set_parent(&child, &parent).await?;
                println!("Set parent of {child} to {parent}");
            }
            (None, false) => bail!("provide a parent item or --clear"),
        },
        Command::Edit { item, title } => {
            print_item(&ledger.edit_title(&item, &title).await?, 0);
        }
        Command::Priority { item, priority } => {
            let priority: Priority = priority.parse()?;
            print_item(&ledger.set_priority(&item, priority).await?, 0);
        }
        Command::Due(args) => {
            let date = date_argument(&args)?;
            print_item(&ledger.set_due_on(&args.item, date).await?, 0);
        }
        Command::Defer(args) => {
            let date = date_argument(&args)?;
            print_item(&ledger.set_defer_until(&args.item, date).await?, 0);
        }
        Command::Block { item, blocker } => {
            ledger.add_blocker(&item, &blocker).await?;
            println!("{item} is blocked by {blocker}");
        }
        Command::Unblock { item, blocker } => {
            ledger.remove_blocker(&item, &blocker).await?;
            println!("Removed blocker {blocker} from {item}");
        }
        Command::Depends { item, dependency } => {
            ledger.add_dependency(&item, &dependency).await?;
            println!("{item} depends on {dependency}");
        }
        Command::Undepends { item, dependency } => {
            ledger.remove_dependency(&item, &dependency).await?;
            println!("Removed dependency {dependency} from {item}");
        }
        Command::Focus { command } => match command {
            FocusCommand::Create { title } => {
                let sequence = ledger.create_focus_sequence(&title).await?;
                println!("created focus sequence {} {}", sequence.id, sequence.title);
            }
            FocusCommand::Add { sequence, item } => {
                ledger.add_focus_sequence_item(&sequence, &item).await?;
                println!("added {item} to focus sequence {sequence}");
            }
            FocusCommand::Ls => {
                for sequence in ledger.focus_sequences().await? {
                    println!("{} {}", sequence.id, sequence.title);
                    for item in sequence.items {
                        println!("  {item}");
                    }
                }
            }
        },
        Command::Session { command } => match command {
            SessionCommand::Start { item } => {
                let session = ledger.start_session(&item, chrono::Utc::now()).await?;
                println!("started session {} at {}", session.id, session.started_at);
            }
            SessionCommand::Stop { item } => {
                let session = ledger.finish_session(&item, chrono::Utc::now()).await?;
                println!(
                    "finished session {} at {}",
                    session.id,
                    session.finished_at.unwrap_or_default()
                );
            }
            SessionCommand::Ls { item } => {
                let stored = ledger.get_item(&item).await?;
                for session in ledger.sessions(stored.item.id).await? {
                    println!(
                        "{} {} {}",
                        session.id,
                        session.started_at,
                        session.finished_at.unwrap_or_else(|| "active".to_owned())
                    );
                }
            }
        },
        Command::Ready(args) => {
            let project = if args.global {
                None
            } else {
                resolve_project(cli.project.as_deref(), &config, &ledger, &cwd).await?
            };
            let scope = selected_scope(args.global, project.as_ref());
            let items = ledger.list_ready(scope, Local::now().date_naive()).await?;
            if items.is_empty() {
                println!("No ready items.");
            } else {
                for item in &items {
                    print_item(item, 0);
                }
            }
        }
        Command::Next { focus, explain } => {
            let project = if focus.global {
                None
            } else {
                resolve_project(cli.project.as_deref(), &config, &ledger, &cwd).await?
            };
            let scope = selected_scope(focus.global, project.as_ref());
            match ledger.next_item(scope, Local::now().date_naive()).await? {
                Some(next) => {
                    print_item(&next.item, 0);
                    if explain {
                        println!("why:");
                        for reason in next.reasons {
                            println!("  {reason}");
                        }
                    }
                }
                None => println!("No ready items."),
            }
        }
        Command::Project(args) => match args.command {
            ProjectCommand::Add { name } => {
                let project = project_path(&name)?;
                ledger.ensure_namespace(&project).await?;
                println!("Created {project}");
            }
            ProjectCommand::Attach { name, path } => {
                let project = project_path(&name)?;
                ledger.ensure_namespace(&project).await?;
                let root = config.attach_project(&path, project.as_str())?;
                config.save(&paths.config())?;
                println!("Attached {} -> {project}", root.display());
            }
            ProjectCommand::Repo { name, url } => {
                let project = project_path(&name)?;
                let url = match url {
                    Some(url) => url,
                    None => git_origin(&cwd).ok_or_else(|| {
                        anyhow::anyhow!("no `origin` remote here; pass the repository URL")
                    })?,
                };
                ledger.set_field(&project, "repo", Some(&url)).await?;
                println!("{project} repo={url}");
            }
            ProjectCommand::Default { name } => {
                let project = project_path(&name)?;
                ledger.ensure_namespace(&project).await?;
                config.default_project = Some(project.to_string());
                config.save(&paths.config())?;
                println!("Default project: {project}");
            }
            ProjectCommand::Ls => {
                for entry in ledger
                    .namespaces()
                    .await?
                    .into_iter()
                    .filter(|entry| entry.path.as_str().starts_with("project/"))
                {
                    println!("{}", entry.path);
                    if let Some(repo) = ledger.fields(&entry.path).await?.get("repo") {
                        println!("  repo: {repo}");
                    }
                    for (root, mapped) in &config.project_paths {
                        if mapped == entry.path.as_str() {
                            println!("  {}", root.display());
                        }
                    }
                }
            }
            ProjectCommand::Move {
                item,
                target,
                global,
            } => {
                let target = if global {
                    None
                } else {
                    Some(project_path(target.as_deref().ok_or_else(|| {
                        anyhow::anyhow!("provide a project or --global")
                    })?)?)
                };
                let moved = ledger.move_project(&item, target.as_ref()).await?;
                print_item(&moved, 0);
            }
        },
        Command::Tag(args) => match args.command {
            TagCommand::Add { item, path } => {
                let path: NamespacePath = path.parse()?;
                ledger.assign_namespace(&item, &path).await?;
                println!("Attached {path} to {item}");
            }
            TagCommand::Remove { item, path } => {
                let path: NamespacePath = path.parse()?;
                ledger.remove_namespace(&item, &path).await?;
                println!("Removed {path} from {item}");
            }
            TagCommand::Mv { current, next } => {
                let current: NamespacePath = current.parse()?;
                let next: NamespacePath = next.parse()?;
                ledger.rename_namespace(&current, &next).await?;
                println!("Renamed {current} -> {next}");
            }
            TagCommand::Ls { tree } => {
                let namespaces = ledger.namespaces().await?;
                if tree {
                    print_namespace_tree(namespaces.iter().map(|entry| entry.path.as_str()));
                } else {
                    for entry in namespaces {
                        println!("{}", entry.path);
                    }
                }
            }
        },
        Command::Import(args) => match args.format {
            ImportFormat::Markdown => {
                let contents = fs::read_to_string(&args.file)
                    .with_context(|| format!("read {}", args.file.display()))?;
                let items = parse_markdown_tasks(&contents)?;
                if args.dry_run {
                    print!("{}", markdown_for_batch(&items)?);
                } else {
                    let project =
                        resolve_project(cli.project.as_deref(), &config, &ledger, &cwd).await?;
                    let source = if args.cite_source {
                        Some(
                            args.file
                                .canonicalize()
                                .with_context(|| format!("resolve {}", args.file.display()))?,
                        )
                    } else {
                        None
                    };
                    let imported = ledger
                        .import_batch(
                            project.as_ref(),
                            &items,
                            source.as_ref().and_then(|path| path.to_str()),
                        )
                        .await?;
                    println!("Imported {} items.", imported.len());
                }
            }
            ImportFormat::Log => {
                if args.cite_source || cli.project.is_some() {
                    bail!("event-log import does not accept --cite-source or --project");
                }
                let records = read_event_log(&args.file)?;
                if args.dry_run {
                    println!("Validated {} exact events.", records.len());
                } else {
                    let imported = ledger.import_events(&records).await?;
                    println!("Imported {imported} exact events.");
                }
            }
        },
        Command::Export(args) => match args.format {
            ExportFormat::Cbor => {
                if args.all || args.global {
                    bail!("CBOR always exports the complete event ledger; omit --all and --global");
                }
                let output = args
                    .output
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("CBOR export requires --output FILE"))?;
                fs::write(
                    output,
                    encode_event_archive(ledger.event_envelopes().await?)?,
                )
                .with_context(|| format!("write {}", output.display()))?;
                println!("Exported exact event ledger to {}", output.display());
            }
            ExportFormat::Log => {
                if args.all || args.global {
                    bail!("event-log export is always complete; omit --all and --global");
                }
                let output = args
                    .output
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("event-log export requires --output FILE"))?;
                fs::write(output, event_log(&ledger.event_records().await?)?)
                    .with_context(|| format!("write {}", output.display()))?;
                println!("Exported exact event log to {}", output.display());
            }
            ExportFormat::Markdown | ExportFormat::Json | ExportFormat::Graph => {
                let project = if args.global {
                    None
                } else {
                    resolve_project(cli.project.as_deref(), &config, &ledger, &cwd).await?
                };
                let scope = selected_scope(args.global, project.as_ref());
                let items = ledger.list_items(scope, args.all).await?;
                let rendered = match args.format {
                    ExportFormat::Markdown => markdown_for_stored(&items),
                    ExportFormat::Json => serde_json::to_string_pretty(&items)?,
                    ExportFormat::Graph => serde_json::to_string_pretty(&GraphExport {
                        format: "todo-graph-v1",
                        items: &items,
                        projects: ledger
                            .namespaces()
                            .await?
                            .into_iter()
                            .filter(|namespace| namespace.kind == NamespaceKind::Project)
                            .map(|namespace| GraphProject {
                                id: namespace.id.to_string(),
                                path: namespace.path.to_string(),
                            })
                            .collect(),
                    })?,
                    ExportFormat::Cbor | ExportFormat::Log => {
                        bail!("unexpected export format while rendering a view")
                    }
                };
                if let Some(output) = args.output {
                    fs::write(&output, format!("{rendered}\n"))
                        .with_context(|| format!("write {}", output.display()))?;
                    println!("Exported {} items to {}", items.len(), output.display());
                } else {
                    println!("{rendered}");
                }
            }
        },
        Command::Backup { directory } => {
            backup(&ledger, &directory).await?;
            println!("Backed up ledger to {}", directory.display());
        }
        Command::Sync(args) => match args.command {
            Some(SyncCommand::Init { repo }) => {
                let repo = init_sync_repo(&repo)?;
                config.sync_repo = Some(repo.clone());
                config.save(&paths.config())?;
                println!(
                    "Configured exact-event sync repository at {}",
                    repo.display()
                );
            }
            None => {
                let repo = config.sync_repo.as_ref().ok_or_else(|| {
                    anyhow::anyhow!("sync is not configured; run `todo sync init --repo PATH`")
                })?;
                sync(&ledger, repo).await?;
                println!("Synchronized exact events with {}", repo.display());
            }
        },
    }
    Ok(())
}

fn event_log(records: &[EventRecord]) -> Result<String> {
    let mut records = records.to_vec();
    records.sort_by_key(|record| record.cid.to_string());
    let lines = records
        .iter()
        .map(|record| LogLine::encode(&record.cid, &record.bytes))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(format!("{}\n", lines.join("\n")))
}

const SYNC_MANIFEST: &str = "format_version = 1\ntransport = \"todo-ledger-jsonl\"\n";
const SYNC_ATTRIBUTES: &str = "events.jsonl merge=union\n";

fn init_sync_repo(path: &Path) -> Result<PathBuf> {
    fs::create_dir_all(path)
        .with_context(|| format!("create sync repository {}", path.display()))?;
    let repo = path
        .canonicalize()
        .with_context(|| format!("resolve sync repository {}", path.display()))?;
    if !repo.join(".git").exists() {
        run_git(&repo, &["init"])?;
    }
    ensure_sync_layout(&repo)?;
    Ok(repo)
}

async fn sync(ledger: &Ledger, repo: &Path) -> Result<()> {
    if !repo.join(".git").exists() {
        bail!("sync repository {} is not a Git checkout", repo.display());
    }
    ensure_sync_layout(repo)?;
    if git_succeeds(
        repo,
        &["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"],
    )? {
        run_git(repo, &["pull", "--no-rebase"])?;
    }
    let imported = ledger
        .import_events(&read_event_log(&repo.join("events.jsonl"))?)
        .await?;
    fs::write(
        repo.join("events.jsonl"),
        event_log(&ledger.event_records().await?)?,
    )
    .context("write canonical event log")?;
    run_git(
        repo,
        &["add", "ledger.toml", ".gitattributes", "events.jsonl"],
    )?;
    if !git_succeeds(repo, &["diff", "--cached", "--quiet"])? {
        run_git(repo, &["commit", "-m", "Sync todo ledger events"])?;
        if git_succeeds(repo, &["remote", "get-url", "origin"])? {
            let push = if git_succeeds(
                repo,
                &["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"],
            )? {
                run_git(repo, &["push"])
            } else {
                run_git(repo, &["push", "--set-upstream", "origin", "HEAD"])
            };
            if push.is_err() {
                // A peer may have pushed after this invocation pulled. Union
                // merge, normalize the log, then retry once.
                run_git(repo, &["pull", "--no-rebase"])?;
                let recovered = ledger
                    .import_events(&read_event_log(&repo.join("events.jsonl"))?)
                    .await?;
                fs::write(
                    repo.join("events.jsonl"),
                    event_log(&ledger.event_records().await?)?,
                )
                .context("rewrite canonical event log after merge")?;
                run_git(repo, &["add", "events.jsonl"])?;
                if !git_succeeds(repo, &["diff", "--cached", "--quiet"])? {
                    run_git(repo, &["commit", "-m", "Canonicalize merged todo events"])?;
                }
                run_git(repo, &["push"])?;
                if recovered > 0 {
                    println!("Imported {recovered} exact events after concurrent push.");
                }
            }
        }
    }
    if imported > 0 {
        println!("Imported {imported} exact events.");
    }
    Ok(())
}

fn ensure_sync_layout(repo: &Path) -> Result<()> {
    let manifest = repo.join("ledger.toml");
    if manifest.exists() {
        if fs::read_to_string(&manifest)? != SYNC_MANIFEST {
            bail!(
                "sync repository {} has an unsupported ledger.toml",
                repo.display()
            );
        }
    } else {
        fs::write(&manifest, SYNC_MANIFEST).context("write ledger format manifest")?;
    }
    let attributes = repo.join(".gitattributes");
    if attributes.exists() {
        let contents = fs::read_to_string(&attributes)?;
        if !contents
            .lines()
            .any(|line| line == "events.jsonl merge=union")
        {
            bail!(
                "sync repository {} does not configure union merging for events.jsonl",
                repo.display()
            );
        }
    } else {
        fs::write(&attributes, SYNC_ATTRIBUTES).context("write sync Git attributes")?;
    }
    let log = repo.join("events.jsonl");
    if !log.exists() {
        fs::write(log, "").context("create event log")?;
    }
    Ok(())
}

fn run_git(repo: &Path, arguments: &[&str]) -> Result<()> {
    let output = ProcessCommand::new("git")
        .args(arguments)
        .current_dir(repo)
        .output()
        .with_context(|| format!("run git {}", arguments.join(" ")))?;
    if output.status.success() {
        return Ok(());
    }
    bail!(
        "git {} failed in {}: {}",
        arguments.join(" "),
        repo.display(),
        String::from_utf8_lossy(&output.stderr).trim()
    )
}

fn git_succeeds(repo: &Path, arguments: &[&str]) -> Result<bool> {
    let status = ProcessCommand::new("git")
        .args(arguments)
        .current_dir(repo)
        .output()
        .with_context(|| format!("run git {}", arguments.join(" ")))?;
    Ok(status.status.success())
}

fn read_event_log(path: &Path) -> Result<Vec<EventRecord>> {
    let contents = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let mut records = BTreeSet::new();
    let mut result = Vec::new();
    for (line_number, line) in contents.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let line = LogLine::parse(line)
            .with_context(|| format!("parse event log line {}", line_number + 1))?;
        if records.insert(line.cid.to_string()) {
            result.push(EventRecord {
                cid: line.cid,
                bytes: line.bytes,
            });
        }
    }
    Ok(result)
}

/// Write independent fidelity tiers rather than pretending one format can be
/// both human-restorable and an exact protocol archive. Source: DI-0009.
async fn backup(ledger: &Ledger, directory: &Path) -> Result<()> {
    if directory.exists() {
        bail!(
            "backup destination {} already exists; choose a new directory",
            directory.display()
        );
    }
    fs::create_dir_all(directory)
        .with_context(|| format!("create backup directory {}", directory.display()))?;

    let all_items = ledger.list_items(ItemScope::All, true).await?;
    fs::write(
        directory.join("items.json"),
        format!("{}\n", serde_json::to_string_pretty(&all_items)?),
    )
    .context("write backup items.json")?;
    fs::write(
        directory.join("events.cbor"),
        encode_event_archive(ledger.event_envelopes().await?)?,
    )
    .context("write backup events.cbor")?;

    let mut restore = String::from(
        "# Todo ledger recovery\n\nRun these commands from this backup directory. Markdown preserves titles, lifecycle states, and parent trees. It does not preserve original IDs, tags, priority, dates, dependencies, notes, citations, or event history. JSON and CBOR retain additional evidence but are not importable yet.\n\n```sh\n",
    );
    let unscoped = ledger.list_items(ItemScope::Unscoped, true).await?;
    if !unscoped.is_empty() {
        fs::write(
            directory.join("global-inbox.md"),
            markdown_for_stored(&unscoped),
        )
        .context("write backup global-inbox.md")?;
        restore.push_str("todo import markdown global-inbox.md\n");
    }

    for project in ledger
        .namespaces()
        .await?
        .into_iter()
        .filter(|entry| entry.path.as_str().starts_with("project/"))
    {
        let items = ledger
            .list_items(ItemScope::Project(&project.path), true)
            .await?;
        if items.is_empty() {
            continue;
        }
        let filename = format!("{}.md", project.path.as_str().replace('/', "--"));
        fs::write(directory.join(&filename), markdown_for_stored(&items))
            .with_context(|| format!("write backup {filename}"))?;
        restore.push_str(&format!(
            "todo --project {} import markdown {}\n",
            project.path, filename
        ));
    }
    restore.push_str("```\n");
    fs::write(directory.join("RESTORE.md"), restore).context("write backup RESTORE.md")?;
    Ok(())
}

fn encode_event_archive(envelopes: Vec<Vec<u8>>) -> Result<Vec<u8>> {
    let mut encoder = minicbor::Encoder::new(Vec::new());
    encoder.array(u64::try_from(envelopes.len()).context("too many event envelopes")?)?;
    for envelope in envelopes {
        encoder.bytes(&envelope)?;
    }
    Ok(encoder.into_writer())
}

/// Explicit `--project`, then an attached directory, then a project whose
/// `repo` field matches this checkout's `origin`, then `default_project`.
async fn resolve_project(
    explicit: Option<&str>,
    config: &AppConfig,
    ledger: &Ledger,
    cwd: &Path,
) -> Result<Option<NamespacePath>> {
    if let Some(explicit) = explicit {
        return Ok(Some(project_path(explicit)?));
    }
    if let Some(project) = config.project_for(cwd)? {
        return Ok(Some(project_path(project)?));
    }
    if let Some(remote) = git_origin(cwd) {
        let mut matches = ledger.projects_for_repo(&remote).await?;
        if matches.len() > 1 {
            let names: Vec<String> = matches.iter().map(ToString::to_string).collect();
            bail!(
                "{remote} is the repo of {}; pass --project to choose",
                names.join(", ")
            );
        }
        if let Some(project) = matches.pop() {
            return Ok(Some(project));
        }
    }
    config
        .default_project
        .as_deref()
        .map(project_path)
        .transpose()
}

/// `(file, line, "todo:<proquint>")` for every reference under `root`, with
/// files relative to `root`.
fn todo_references(root: &Path) -> Result<Vec<(String, String, String)>> {
    const C: &str = "[bdfghjklmnprstvz]";
    const V: &str = "[aiou]";
    let word = format!("{C}{V}{C}{V}{C}");
    let pattern = format!("todo:{word}(-{word})+");
    let mut arguments = vec!["grep", "-I", "-n", "-o", "-z", "-E", pattern.as_str()];
    if !git_succeeds(root, &["rev-parse", "--is-inside-work-tree"])? {
        arguments.insert(1, "--no-index");
    } else {
        arguments.insert(1, "--untracked");
    }
    arguments.extend(["--", "."]);
    let output = ProcessCommand::new("git")
        .args(&arguments)
        .current_dir(root)
        .output()
        .with_context(|| format!("run git grep in {}", root.display()))?;
    // git grep exits 1 when nothing matches.
    if !output.status.success() && output.status.code() != Some(1) {
        bail!(
            "git grep failed in {}: {}",
            root.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    let mut result = Vec::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let mut parts = line.splitn(3, '\0');
        if let (Some(file), Some(number), Some(reference)) =
            (parts.next(), parts.next(), parts.next())
        {
            result.push((file.to_owned(), number.to_owned(), reference.to_owned()));
        }
    }
    Ok(result)
}

/// The `origin` URL of the checkout containing `cwd`, if there is one.
fn git_origin(cwd: &Path) -> Option<String> {
    let output = ProcessCommand::new("git")
        .args(["remote", "get-url", "origin"])
        .current_dir(cwd)
        .output()
        .ok()?;
    let url = String::from_utf8(output.stdout).ok()?;
    (output.status.success() && !url.trim().is_empty()).then(|| url.trim().to_owned())
}

fn project_path(value: &str) -> Result<NamespacePath> {
    let value = if value.contains('/') {
        value.to_owned()
    } else {
        format!("project/{value}")
    };
    let path: NamespacePath = value.parse()?;
    if !path.as_str().starts_with("project/") {
        bail!("project scope must start with `project/`");
    }
    Ok(path)
}

fn entry_syntax(config: &AppConfig, fields: bool) -> EntrySyntax {
    EntrySyntax {
        ascii: config.entry_ascii.unwrap_or(true),
        fields,
    }
}

/// Maps a typed-entry actor name to its path: `quincy` is `actor/quincy`, and
/// `me` is the configured operator.
fn actor_path(config: &AppConfig, name: &str) -> Result<NamespacePath> {
    let name = match name {
        "me" => config
            .me
            .as_deref()
            .ok_or_else(|| anyhow!("`me` is not set; add me = \"<name>\" to config.toml"))?,
        other => other,
    };
    let name = name.strip_prefix("actor/").unwrap_or(name);
    Ok(format!("actor/{name}").parse()?)
}

enum Target {
    Item(String),
    Node(NamespacePath),
}

/// Resolves `todo set`'s target: a full namespace path, else an item, else an
/// actor name. Anything that reads as a proquint must be an item, so a mistyped
/// item ID cannot quietly create an actor.
async fn resolve_target(ledger: &Ledger, config: &AppConfig, target: &str) -> Result<Target> {
    if let Ok(path) = target.parse::<NamespacePath>()
        && !matches!(path.kind()?, NamespaceKind::Custom(_))
    {
        return Ok(Target::Node(path));
    }
    match ledger.get_item(target).await {
        Ok(item) => Ok(Target::Item(item.item.id.to_string())),
        Err(error) if todo_core::proquint_to_hex(target).is_some() => Err(error),
        Err(_) => Ok(Target::Node(actor_path(config, target)?)),
    }
}

/// Applies typed words to an existing item, echoing each change in its canonical
/// form. Each word is its own event; a failure stops at that word.
async fn apply_to_item(
    ledger: &Ledger,
    config: &AppConfig,
    item: &str,
    directives: &[Directive],
) -> Result<()> {
    for Directive { remove, kind } in directives {
        let remove = *remove;
        match kind {
            DirectiveKind::Tag(path) if remove => ledger.remove_namespace(item, path).await?,
            DirectiveKind::Tag(path) => ledger.assign_namespace(item, path).await?,
            DirectiveKind::Project(name) => {
                let target = if remove {
                    None
                } else {
                    Some(project_path(name)?)
                };
                ledger.move_project(item, target.as_ref()).await?;
            }
            DirectiveKind::Role { role, actor } => {
                let path = actor_path(config, actor)?;
                if remove {
                    ledger.remove_role(item, role, &path).await?;
                } else {
                    ledger.assign_role(item, role, &path).await?;
                }
            }
            DirectiveKind::Due(date) => {
                ledger.set_due_on(item, (!remove).then_some(*date)).await?;
            }
            DirectiveKind::Defer(date) => {
                ledger
                    .set_defer_until(item, (!remove).then_some(*date))
                    .await?;
            }
            DirectiveKind::Priority(_) if remove => {
                bail!("a priority cannot be removed; set p:normal instead")
            }
            DirectiveKind::Priority(priority) => {
                ledger.set_priority(item, *priority).await?;
            }
            DirectiveKind::EstimateMinutes(minutes) => {
                ledger
                    .set_estimate(item, (!remove).then_some(*minutes))
                    .await?;
            }
            DirectiveKind::Depends(other) if remove => {
                ledger.remove_dependency(item, other).await?
            }
            DirectiveKind::Depends(other) => ledger.add_dependency(item, other).await?,
            DirectiveKind::Relates(other) if remove => {
                ledger
                    .remove_relation(item, RelationKind::RelatesTo, other)
                    .await?;
            }
            DirectiveKind::Relates(other) => {
                ledger
                    .add_relation(item, RelationKind::RelatesTo, other)
                    .await?;
            }
            DirectiveKind::Field { .. } => {
                bail!("items have no fields; key=value applies to actors and projects")
            }
        }
        println!("{} {kind}", if remove { "-" } else { "+" });
    }
    Ok(())
}

/// Applies `key=value` words to a namespace node. `member=<actor>` and
/// `member-of=<actor>` record membership instead of a field.
async fn apply_to_node(
    ledger: &Ledger,
    config: &AppConfig,
    path: &NamespacePath,
    directives: &[Directive],
) -> Result<()> {
    if let Some(Directive { kind, .. }) = directives
        .iter()
        .find(|directive| !matches!(directive.kind, DirectiveKind::Field { .. }))
    {
        bail!("{kind} applies to items, not to {path}");
    }
    if ledger.namespace_by_path(path).await?.is_none() {
        println!("Creating {path}");
    }
    for Directive { remove, kind } in directives {
        let DirectiveKind::Field { key, value } = kind else {
            continue;
        };
        let (member, group) = match key.as_str() {
            "member" => (actor_path(config, value)?, path.clone()),
            "member-of" => (path.clone(), actor_path(config, value)?),
            _ => {
                let value = (!remove && !value.is_empty()).then_some(value.as_str());
                ledger.set_field(path, key, value).await?;
                println!("{path} {key}={}", value.unwrap_or(""));
                continue;
            }
        };
        if *remove {
            ledger.unlink_namespace(&member, MEMBER_OF, &group).await?;
            println!("{member} is no longer {MEMBER_OF} {group}");
        } else {
            ledger.link_namespace(&member, MEMBER_OF, &group).await?;
            println!("{member} {MEMBER_OF} {group}");
        }
    }
    Ok(())
}

async fn show_actor(ledger: &Ledger, path: &NamespacePath) -> Result<()> {
    if ledger.namespace_by_path(path).await?.is_none() {
        bail!("unknown actor {path}");
    }
    println!("{path}");
    for (key, value) in ledger.fields(path).await? {
        println!("{key}: {value}");
    }
    for (relation, group) in ledger.namespace_links(path).await? {
        println!("{relation}: {group}");
    }
    for (_, member) in ledger.namespace_linked_from(path).await? {
        println!("member: {member}");
    }
    for (role, item) in ledger.role_items(path).await? {
        println!("{role}: {}", describe(ledger, item).await);
    }
    Ok(())
}

fn date_argument(args: &DateArgs) -> Result<Option<NaiveDate>> {
    match (&args.date, args.clear) {
        (_, true) => Ok(None),
        (Some(date), false) => Ok(Some(
            NaiveDate::parse_from_str(date, "%Y-%m-%d")
                .with_context(|| format!("invalid date {date:?}; expected YYYY-MM-DD"))?,
        )),
        (None, false) => bail!("provide a YYYY-MM-DD date or --clear"),
    }
}

fn selected_scope<'a>(global: bool, project: Option<&'a NamespacePath>) -> ItemScope<'a> {
    match (global, project) {
        (true, _) => ItemScope::All,
        (false, Some(project)) => ItemScope::Project(project),
        (false, None) => ItemScope::Unscoped,
    }
}

/// Returns the short spoken identifier for a task.
///
/// Proquints rather than trailing hexadecimal: the same distinguishing bits, but
/// readable aloud and typable from memory.
fn short_id(id: ItemId) -> String {
    id.short()
}

fn state_mark(state: ItemState) -> &'static str {
    match state {
        ItemState::Inbox => "[ ]",
        ItemState::Active => "[>]",
        ItemState::Done => "[x]",
        ItemState::Canceled => "[-]",
        ItemState::Void => "[~]",
    }
}

fn print_item(stored: &StoredItem, depth: usize) {
    let tags = if stored.namespaces.is_empty() {
        String::new()
    } else {
        format!(
            "  {}",
            stored
                .namespaces
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(" ")
        )
    };
    let mut focus = Vec::new();
    if stored.item.priority != Priority::Normal {
        focus.push(format!("!{}", stored.item.priority));
    }
    if let Some(due) = stored.item.due_on {
        focus.push(format!("due:{due}"));
    }
    if let Some(defer) = stored.item.defer_until {
        focus.push(format!("defer:{defer}"));
    }
    if !stored.dependencies.is_empty() {
        focus.push(format!("depends:{}", stored.dependencies.len()));
    }
    if !stored.blocked_by.is_empty() {
        focus.push(format!("blocked:{}", stored.blocked_by.len()));
    }
    let focus = if focus.is_empty() {
        String::new()
    } else {
        format!("  {}", focus.join(" "))
    };
    println!(
        "{}{} {} {}{}{}",
        "  ".repeat(depth),
        state_mark(stored.item.state),
        short_id(stored.item.id),
        stored.item.title,
        tags,
        focus,
    );
}

fn print_tree(items: &[StoredItem]) {
    let ids: HashSet<_> = items.iter().map(|stored| stored.item.id).collect();
    let mut children: HashMap<ItemId, Vec<&StoredItem>> = HashMap::new();
    let mut roots = Vec::new();
    for stored in items {
        match stored.item.parent_id.filter(|parent| ids.contains(parent)) {
            Some(parent) => children.entry(parent).or_default().push(stored),
            None => roots.push(stored),
        }
    }
    fn walk(item: &StoredItem, depth: usize, children: &HashMap<ItemId, Vec<&StoredItem>>) {
        print_item(item, depth);
        if let Some(descendants) = children.get(&item.item.id) {
            for descendant in descendants {
                walk(descendant, depth + 1, children);
            }
        }
    }
    for root in roots {
        walk(root, 0, &children);
    }
}

fn print_namespace_tree<'a>(paths: impl IntoIterator<Item = &'a str>) {
    let mut prefixes = BTreeSet::new();
    for path in paths {
        let mut prefix = String::new();
        for segment in path.split('/') {
            if !prefix.is_empty() {
                prefix.push('/');
            }
            prefix.push_str(segment);
            prefixes.insert(prefix.clone());
        }
    }
    for prefix in prefixes {
        let depth = prefix.matches('/').count();
        let Some(segment) = prefix.rsplit('/').next() else {
            continue;
        };
        println!("{}{}", "  ".repeat(depth), segment);
    }
}

fn parse_markdown_tasks(contents: &str) -> Result<Vec<BatchItem>> {
    let mut items = Vec::new();
    let mut ancestors: Vec<(usize, usize)> = Vec::new();
    for (line_index, line) in contents.lines().enumerate() {
        let trimmed = line.trim_start_matches([' ', '\t']);
        let leading = &line[..line.len() - trimmed.len()];
        if !trimmed.starts_with("- [") && !trimmed.starts_with("* [") {
            continue;
        }
        if leading.contains('\t') {
            bail!(
                "line {} uses a tab for task indentation; use spaces",
                line_index + 1
            );
        }
        let bytes = trimmed.as_bytes();
        let Some((&marker, &b']')) = bytes.get(3).zip(bytes.get(4)) else {
            bail!("line {} has an invalid task marker", line_index + 1);
        };
        let state = match marker.to_ascii_lowercase() {
            b' ' => ItemState::Inbox,
            b'>' => ItemState::Active,
            b'x' => ItemState::Done,
            b'-' => ItemState::Canceled,
            marker => bail!(
                "line {} has unknown task marker {:?}",
                line_index + 1,
                char::from(marker)
            ),
        };
        let title = trimmed
            .get(5..)
            .ok_or_else(|| anyhow!("line {} has an invalid task marker", line_index + 1))?
            .trim();
        if title.is_empty() {
            bail!("line {} has an empty task title", line_index + 1);
        }
        let indent = leading.len();
        while ancestors.last().is_some_and(|(level, _)| *level >= indent) {
            ancestors.pop();
        }
        if indent > 0 && ancestors.is_empty() {
            bail!("line {} is indented without a parent task", line_index + 1);
        }
        let parent_index = ancestors.last().map(|(_, index)| *index);
        let index = items.len();
        items.push(BatchItem {
            title: title.to_owned(),
            state,
            parent_index,
        });
        ancestors.push((indent, index));
    }
    if items.is_empty() {
        bail!("Markdown contains no checkbox tasks");
    }
    Ok(items)
}

fn markdown_for_batch(items: &[BatchItem]) -> Result<String> {
    let mut output = String::new();
    for (index, item) in items.iter().enumerate() {
        let mut depth = 0;
        let mut parent = item.parent_index;
        while let Some(parent_index) = parent {
            depth += 1;
            parent = items
                .get(parent_index)
                .ok_or_else(|| anyhow!("Markdown task parent index is invalid"))?
                .parent_index;
        }
        markdown_line(&mut output, depth, item.state, &item.title);
        debug_assert!(item.parent_index.is_none_or(|parent| parent < index));
    }
    Ok(output)
}

fn markdown_for_stored(items: &[StoredItem]) -> String {
    let ids: HashSet<_> = items.iter().map(|stored| stored.item.id).collect();
    let mut children: HashMap<ItemId, Vec<&StoredItem>> = HashMap::new();
    let mut roots = Vec::new();
    for stored in items {
        match stored.item.parent_id.filter(|parent| ids.contains(parent)) {
            Some(parent) => children.entry(parent).or_default().push(stored),
            None => roots.push(stored),
        }
    }
    fn walk(
        output: &mut String,
        item: &StoredItem,
        depth: usize,
        children: &HashMap<ItemId, Vec<&StoredItem>>,
    ) {
        markdown_line(output, depth, item.item.state, &item.item.title);
        if let Some(descendants) = children.get(&item.item.id) {
            for descendant in descendants {
                walk(output, descendant, depth + 1, children);
            }
        }
    }
    let mut output = String::new();
    for root in roots {
        walk(&mut output, root, 0, &children);
    }
    output
}

fn markdown_line(output: &mut String, depth: usize, state: ItemState, title: &str) {
    let marker = match state {
        ItemState::Inbox => ' ',
        ItemState::Active => '>',
        ItemState::Done => 'x',
        ItemState::Canceled => '-',
        // Markdown has no marker for "was never a task", and a void item is left
        // out of the listings this renders anyway; '-' keeps the tree readable.
        ItemState::Void => '-',
    };
    let title = title.replace(['\r', '\n'], " ");
    output.push_str(&format!("{}- [{marker}] {title}\n", "  ".repeat(depth)));
}

/// Parses a numeric argument where "clear" and an empty value remove the setting.
fn parse_clearable<T: std::str::FromStr>(value: &str, label: &str) -> Result<Option<T>> {
    let value = value.trim();
    if value.is_empty() || value.eq_ignore_ascii_case("clear") || value == "-" {
        return Ok(None);
    }
    value
        .parse::<T>()
        .map(Some)
        .map_err(|_| anyhow::anyhow!("{label} must be a whole number, or \"clear\""))
}

/// Renders an optional number for terminal output.
fn describe_optional<T: std::fmt::Display>(value: Option<T>) -> String {
    value.map_or_else(|| "cleared".to_owned(), |value| value.to_string())
}

fn print_state(item: &StoredItem) {
    println!(
        "{} {} {}",
        state_mark(item.item.state),
        short_id(item.item.id),
        item.item.title
    );
}

/// Names another task by title and spoken id, for the references in `show`.
///
/// A bare UUID makes a reader look the task up; the title is the answer they were
/// going to look up, and the proquint is short enough to repeat back.
async fn describe(ledger: &Ledger, id: ItemId) -> String {
    match ledger.get_item(&id.to_string()).await {
        Ok(found) => format!("{} {}", id.short(), found.item.title),
        // A referenced task may be deleted or out of scope; the id still names it.
        Err(_) => id.short(),
    }
}

async fn show(ledger: &Ledger, reference: &str, include_history: bool) -> Result<()> {
    let stored = ledger.get_item(reference).await?;
    println!("{} {}", state_mark(stored.item.state), stored.item.title);
    println!("id: {} ({})", stored.item.id.short(), stored.item.id);
    println!("state: {}", stored.item.state);
    println!("priority: {}", stored.item.priority);
    if let Some(project) = stored.item.project_id {
        // ponytail: scans every namespace; add a by-id lookup if show gets slow.
        if let Some(entry) = ledger
            .namespaces()
            .await?
            .into_iter()
            .find(|entry| entry.id == project)
        {
            println!("project: {}", entry.path);
        }
    }
    if let Some(due) = stored.item.due_on {
        println!("due: {due}");
    }
    if let Some(defer) = stored.item.defer_until {
        println!("defer: {defer}");
    }
    if let Some(parent) = stored.item.parent_id {
        println!("parent: {}", describe(ledger, parent).await);
    }
    if !stored.namespaces.is_empty() {
        println!(
            "tags: {}",
            stored
                .namespaces
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    for dependency in &stored.dependencies {
        println!("depends-on: {}", describe(ledger, *dependency).await);
    }
    for blocker in &stored.blocked_by {
        println!("blocked-by: {}", describe(ledger, *blocker).await);
    }
    for relation in &stored.relations {
        println!(
            "{}: {}",
            relation.relation,
            describe(ledger, relation.target).await
        );
    }
    // A symmetric relation written from the other end belongs here too.
    for (relation, source) in ledger.related_from(reference).await? {
        if relation.is_symmetric() {
            println!("{relation}: {}", describe(ledger, source).await);
        }
    }
    for (role, actor) in ledger.roles(stored.item.id).await? {
        let name = actor
            .as_str()
            .strip_prefix("actor/")
            .unwrap_or(actor.as_str());
        println!("{role}: {name}");
    }
    if let Some(difficulty) = stored.item.difficulty {
        println!("difficulty: {difficulty}");
    }
    if let Some(estimate) = stored.item.estimate_minutes {
        println!("estimate-minutes: {estimate}");
    }
    for note in ledger.notes(stored.item.id).await? {
        println!("note [{}]: {}", note.recorded_at, note.body);
    }
    for cite in ledger.citations(stored.item.id).await? {
        println!(
            "cite [{}]: {}{}",
            cite.recorded_at,
            cite.target,
            cite.label
                .map(|label| format!(" ({label})"))
                .unwrap_or_default()
        );
    }
    if include_history {
        println!("history:");
        for event in ledger.history(stored.item.id).await? {
            println!("  {} {} {}", event.recorded_at, event.kind, event.cid);
        }
    }
    Ok(())
}
