# Complete Working Example

A self-contained project you can copy, build, and run. This creates a simple todo list CLI with styled terminal output.

## File Structure

```text
my-todo/
├── Cargo.toml
├── src/
│   ├── main.rs
│   ├── templates/
│   │   └── list.jinja
│   └── styles/
│       └── default.css
```

## Cargo.toml

```toml
[package]
name = "my-todo"
version = "0.1.0"
edition = "2021"

[dependencies]
standout = "7"
clap = { version = "4", features = ["derive"] }
serde = { version = "1", features = ["derive"] }
```

## src/main.rs

```rust
use clap::{ArgMatches, Parser, Subcommand};
use serde::Serialize;
use standout::cli::{App, CommandContext, Dispatch, HandlerResult, Output};
use standout::{embed_styles, embed_templates};

#[derive(Parser)]
#[command(name = "my-todo")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Dispatch)]
#[dispatch(handlers = handlers)]
enum Commands {
    /// List all todos
    List,
}

#[derive(Serialize)]
struct TodoResult {
    todos: Vec<Todo>,
}

#[derive(Serialize)]
struct Todo {
    title: String,
    status: String,
}

mod handlers {
    use super::*;

    pub fn list(_m: &ArgMatches, _ctx: &CommandContext) -> HandlerResult<TodoResult> {
        // Your real logic goes here — database queries, API calls, etc.
        let todos = vec![
            Todo { title: "Write documentation".into(), status: "done".into() },
            Todo { title: "Ship v1.0".into(), status: "pending".into() },
            Todo { title: "Add tests".into(), status: "pending".into() },
        ];
        Ok(Output::Render(TodoResult { todos }))
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let app = App::builder()
        .templates(embed_templates!("src/templates"))
        .styles(embed_styles!("src/styles"))
        .default_theme("default")
        .commands(Commands::dispatch_config())
        .build()?;

    app.run(Cli::command(), std::env::args());
    Ok(())
}
```

## src/templates/list.jinja

```jinja
[title]My Todos[/title]
{% for todo in todos %}
[index]{{ loop.index }}.[/index] [{{ todo.status }}]{{ todo.title }}[/{{ todo.status }}]
{% endfor %}
```

## src/styles/default.css

```css
.title {
    color: cyan;
    font-weight: bold;
}

.index {
    color: yellow;
}

.done {
    text-decoration: line-through;
    color: gray;
}

.pending {
    font-weight: bold;
    color: white;
}

/* Adaptive: adjust for light terminals */
@media (prefers-color-scheme: light) {
    .pending { color: black; }
}
```

## Run It

```bash
cargo run -- list              # Rich terminal output with colors
cargo run -- list --output json    # JSON for scripting
cargo run -- list --output text    # Plain text, no ANSI codes
```

## What You Get

- **Testable logic**: `handlers::list` is a pure function — test it by asserting on the returned `TodoResult`
- **Free output modes**: JSON, YAML, CSV, and plain text output from the same handler
- **Hot reload**: Edit `list.jinja` or `default.css` during development — changes apply without recompiling (debug builds)
- **Adaptive styles**: The `@media` query adjusts colors for light/dark terminals automatically

## Next Steps

- [Introduction to Standout](intro-to-standout.md) — Full walkthrough with incremental steps
- [Styling System](../crates/render/topics/styling-system.md) — All CSS properties and adaptive styles
- [Tabular Layout](../crates/render/guides/intro-to-tabular.md) — Column alignment for table output
