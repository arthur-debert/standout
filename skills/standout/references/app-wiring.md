# App and command wiring

Build the `App` in the CLI package, never in a reusable library: dependencies,
embedded assets, theme, commands and hooks, all before dispatch.

```rust
use standout::cli::App;
use standout::{embed_styles, embed_templates};

let app = App::builder()
    .app_state(store)
    .templates(embed_templates!("src/templates"))
    .styles(embed_styles!("src/styles"))
    .default_theme("todo")
    .command_with("list", handlers::list_Handler, |cfg| cfg.template_name("list"))?
    .build()?;
```

`AppBuilder::command_with` takes any `Handler`: a generated `name_Handler` or `FnHandler::new(closure)`. `cfg` adds template, hooks, inputs and pipes. Dot-separated paths register nested commands.

`build()` fails when a command's template is missing. Without a human template, declare `structured_only()`, `silent()` or `binary()`.

## Derive wiring

Prefer `#[derive(Dispatch)]` when variants map to handlers by convention:

```rust
#[derive(clap::Subcommand, standout::cli::Dispatch)]
#[dispatch(handlers = handlers)]
enum Commands {
    #[dispatch(pure)]
    List,
    #[dispatch(pure, template_name = "add")]
    Add,
}

let app = App::builder()
    .commands(Commands::dispatch_config())?
    .build()?;
```

`List` registers `list` with `handlers::list`, a `fn(&ArgMatches, &CommandContext) -> HandlerResult<T>`. `#[dispatch(pure)]` selects the `#[handler]` wrapper `handlers::list__handler`, which compiles only when the function returns `Result<Output<T>, anyhow::Error>` or `Result<(), anyhow::Error>`. Other keys cover hooks, inputs, nesting, defaults, paging and pipes.

## Running and partial adoption

`App::run(command, args)` returns `true` when Standout handled the command, `false` when nothing matched. A nonzero exit status ends the process; `run_emitted` returns a `ProcessOutcome` instead.

```rust
if !app.run(Cli::command(), std::env::args()) {
    run_legacy_path();
}
```

`run_with(cmd, args, TargetProperties::detect(), InputSources::from_process())` writes neither stream; match `into_outcome()` on the `CompletedRun` as `DispatchResult::{Handled, Binary, Artifact, Silent, Error, NoMatch}` with a wildcard arm; `NoMatch` carries the `ArgMatches`.

Template names resolve with or without extension; the convention name is the command path with `.` as `/`.

Sources: `crates/todo-example/tdoo/src/app.rs`, `crates/standout/src/cli/builder/`, `crates/standout-macros/src/dispatch.rs`.
