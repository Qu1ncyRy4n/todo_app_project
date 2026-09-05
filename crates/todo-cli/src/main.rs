use anyhow::{Context, Result, bail};
use chrono::{Local, NaiveDate};
use clap::{Args, Parser, Subcommand, ValueEnum};
use std::{
    collections::{BTreeSet, HashMap, HashSet},
    env, fs,
    path::{Path, PathBuf},
};
use todo_core::{ItemId, ItemState, NamespacePath, Priority};
use todo_store::{AppConfig, AppPaths, BatchItem, ItemScope, Ledger, StoredItem};

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
    /// Declare that an item is blocked by another item.
    Block {
        item: String,
        #[arg(long = "on")]
        blocker: String,
    },
    /// Remove a blocking relationship.
    Unblock {
        item: String,
        #[arg(long = "on")]
        blocker: String,
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
    Cbor,
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
        let _ledger = Ledger::open(&paths.database()).await?;
        let config = AppConfig::load(&paths.config())?;
        config.save(&paths.config())?;
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
    let ledger = Ledger::open(&paths.database()).await?;
    let cwd = env::current_dir().context("determine current directory")?;

    match cli.command {
        Command::Init => unreachable!(),
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
            ledger.add_dependency(&item, &blocker).await?;
            println!("{item} is blocked by {blocker}");
        }
        Command::Unblock { item, blocker } => {
            ledger.remove_dependency(&item, &blocker).await?;
            println!("Removed blocker {blocker} from {item}");
        }
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
        Command::Import(args) => {
            let contents = fs::read_to_string(&args.file)
                .with_context(|| format!("read {}", args.file.display()))?;
            let items = parse_markdown_tasks(&contents)?;
            if args.dry_run {
                print!("{}", markdown_for_batch(&items));
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
            ExportFormat::Markdown | ExportFormat::Json => {
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
                    ExportFormat::Cbor => unreachable!(),
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
    }
    Ok(())
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
    encoder.array(envelopes.len() as u64)?;
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
    if !stored.blockers.is_empty() {
        focus.push(format!("depends:{}", stored.blockers.len()));
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
        let segment = prefix.rsplit('/').next().expect("prefix has a segment");
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
        if bytes.len() < 6 || bytes[4] != b']' {
            bail!("line {} has an invalid task marker", line_index + 1);
        }
        let state = match bytes[3].to_ascii_lowercase() {
            b' ' => ItemState::Inbox,
            b'>' => ItemState::Active,
            b'x' => ItemState::Done,
            b'-' => ItemState::Canceled,
            marker => bail!(
                "line {} has unknown task marker {:?}",
                line_index + 1,
                marker as char
            ),
        };
        let title = trimmed[5..].trim();
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

fn markdown_for_batch(items: &[BatchItem]) -> String {
    let mut output = String::new();
    for (index, item) in items.iter().enumerate() {
        let mut depth = 0;
        let mut parent = item.parent_index;
        while let Some(parent_index) = parent {
            depth += 1;
            parent = items[parent_index].parent_index;
        }
        markdown_line(&mut output, depth, item.state, &item.title);
        debug_assert!(item.parent_index.is_none_or(|parent| parent < index));
    }
    output
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
    for blocker in &stored.blockers {
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
