//! `tdoo` library surface — exposed so integration tests can build the
//! same `App` the binary builds and run it through `standout-test`.

use anyhow::Result;
use clap::{ArgMatches, CommandFactory, Parser, Subcommand};
use serde_json::Value as JsonValue;
use standout::cli::hooks::HookError;
use standout::cli::{App, CommandContext};
use standout::input::{ArgSource, InputChain, StdinSource};
use standout::{embed_styles, embed_templates};

pub mod handlers;
pub mod store;

pub use store::TodoStore;

#[derive(Parser)]
#[command(name = "tdoo", about = "A tiny todo list — the Standout sample app")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

// We use plain clap-derive here. Standout has a `#[derive(Dispatch)]` that
// auto-wires variants to handlers by name; we skip it because we want to
// register `add` with a custom input chain (see below) and showcase the
// explicit `command_with` form too.
#[derive(Subcommand)]
pub enum Commands {
    /// Add a new todo. Title comes from --title or piped stdin.
    Add {
        #[arg(short, long)]
        title: Option<String>,
    },
    /// List todos. By default only pending ones; pass --all for everything.
    List {
        #[arg(short, long)]
        all: bool,
    },
    /// Mark a todo done.
    Done { id: u32 },
}

pub fn build_app(store: TodoStore) -> Result<App> {
    let app = App::builder()
        // app_state is a process-lifetime injection slot. Handlers reach it
        // via `ctx.app_state.get_required::<TodoStore>()`.
        .app_state(store)
        // `embed_templates!` walks the directory at compile time; the
        // resulting bundle is embedded in the binary, but in debug builds
        // the framework also watches the original path for hot reload.
        .templates(embed_templates!("src/templates"))
        .styles(embed_styles!("src/styles"))
        // Style file is `todo.css`, so the theme name is `todo`. Without
        // this call the framework's built-in default theme is used instead.
        .default_theme("todo")
        // `command_with` (rather than `.command`) lets us attach config —
        // here, an InputChain for `add` and a post-dispatch audit hook for
        // mutations. The handler comes from `#[handler]` on the function:
        // the macro generates `add__handler` next to `add`.
        .command_with("add", handlers::add__handler, |cfg| {
            cfg.template("add.jinja")
                .input(
                    "title",
                    InputChain::<String>::new()
                        .try_source(ArgSource::new("title"))
                        .try_source(StdinSource::new())
                        .validate(|s| !s.trim().is_empty(), "title cannot be empty"),
                )
                .post_dispatch(audit_hook)
        })?
        .command_with("list", handlers::list__handler, |cfg| {
            cfg.template("list.jinja")
        })?
        .command_with("done", handlers::done__handler, |cfg| {
            cfg.template("done.jinja").post_dispatch(audit_hook)
        })?
        .build()?;
    Ok(app)
}

pub fn cli_command() -> clap::Command {
    Cli::command()
}

pub fn resolve_store_path() -> std::path::PathBuf {
    if let Ok(p) = std::env::var("TODO_FILE") {
        return p.into();
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    std::path::PathBuf::from(home).join(".todos.json")
}

// A post-dispatch hook is the framework's seam for cross-cutting concerns
// after the handler returns but before rendering. The hook receives the
// handler's already-serialized JSON and can transform it. Here we use it
// for an audit trail so the handler stays focused on its single concern.
fn audit_hook(
    _matches: &ArgMatches,
    ctx: &CommandContext,
    value: JsonValue,
) -> std::result::Result<JsonValue, HookError> {
    if let Ok(path) = std::env::var("TODO_AUDIT_LOG") {
        let line = format!(
            "{}\t{}\n",
            ctx.command_path.join("."),
            value
                .get("todo")
                .and_then(|t| t.get("id"))
                .unwrap_or(&JsonValue::Null)
        );
        // A hook returning Err aborts the pipeline; we swallow IO errors
        // here because audit logging shouldn't fail the user's command.
        let _ = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .and_then(|mut f| std::io::Write::write_all(&mut f, line.as_bytes()));
    }
    Ok(value)
}
