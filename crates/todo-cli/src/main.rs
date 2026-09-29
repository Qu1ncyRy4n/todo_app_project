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
use todo_core::{ItemId, ItemState, LogLine, NamespaceKind, NamespacePath, Priority};
use todo_store::{AppConfig, AppPaths, BatchItem, EventRecord, ItemScope, Ledger, StoredItem};

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
    title: String,
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
            let project = resolve_project(cli.project.as_deref(), &config, &cwd)?;
            let tags = args
                .tags
                .iter()
                .map(|tag| tag.parse())
                .collect::<Result<Vec<NamespacePath>, _>>()?;
            let item = ledger
                .add_item_with(&args.title, project.as_ref(), &tags, args.parent.as_deref())
                .await?;
            println!(
                "Added {} [inbox] {}",
                short_id(item.item.id),
                item.item.title
            );
        }
        Command::Ls(args) => {
            let project = if args.global {
                None
            } else {
                resolve_project(cli.project.as_deref(), &config, &cwd)?
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
                resolve_project(cli.project.as_deref(), &config, &cwd)?
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
                resolve_project(cli.project.as_deref(), &config, &cwd)?
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
                    let project = resolve_project(cli.project.as_deref(), &config, &cwd)?;
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
                    resolve_project(cli.project.as_deref(), &config, &cwd)?
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

fn resolve_project(
    explicit: Option<&str>,
    config: &AppConfig,
    cwd: &Path,
) -> Result<Option<NamespacePath>> {
    if let Some(explicit) = explicit {
        return Ok(Some(project_path(explicit)?));
    }
    config.project_for(cwd)?.map(project_path).transpose()
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

fn short_id(id: ItemId) -> String {
    id.to_string()
        .chars()
        .rev()
        .take(8)
        .collect::<String>()
        .chars()
        .rev()
        .collect()
}

fn state_mark(state: ItemState) -> &'static str {
    match state {
        ItemState::Inbox => "[ ]",
        ItemState::Active => "[>]",
        ItemState::Done => "[x]",
        ItemState::Canceled => "[-]",
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
    };
    let title = title.replace(['\r', '\n'], " ");
    output.push_str(&format!("{}- [{marker}] {title}\n", "  ".repeat(depth)));
}

fn print_state(item: &StoredItem) {
    println!(
        "{} {} {}",
        state_mark(item.item.state),
        short_id(item.item.id),
        item.item.title
    );
}

async fn show(ledger: &Ledger, reference: &str, include_history: bool) -> Result<()> {
    let stored = ledger.get_item(reference).await?;
    println!("{} {}", state_mark(stored.item.state), stored.item.title);
    println!("id: {}", stored.item.id);
    println!("state: {}", stored.item.state);
    println!("priority: {}", stored.item.priority);
    if let Some(due) = stored.item.due_on {
        println!("due: {due}");
    }
    if let Some(defer) = stored.item.defer_until {
        println!("defer: {defer}");
    }
    if let Some(parent) = stored.item.parent_id {
        println!("parent: {parent}");
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
    for blocker in &stored.dependencies {
        println!("depends-on: {blocker}");
    }
    for blocker in &stored.blocked_by {
        println!("blocked-by: {blocker}");
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
