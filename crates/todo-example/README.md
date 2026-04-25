# Todo: a worked example of a Standout app

A small but real CLI built on [Standout](https://standout.magik.works/). Three commands (`add`, `list`, `done`), a JSON-backed store, file templates with adaptive CSS, an InputChain on `add`, a post-dispatch hook for audit logging, and end-to-end tests that exercise the whole pipeline in-process.

This is meant to be read top-to-bottom with the source open beside it. Each section explains *what* a file does and *why* it's shaped that way — and points out the options you'd toggle for a different app.

```text
crates/todo-example/
├── Cargo.toml
├── README.md                 (this file)
├── src/
│   ├── lib.rs                App wiring: builder, commands, hooks, InputChain
│   ├── main.rs               Thin shell: load store, build app, run
│   ├── handlers.rs           Pure functions returning data
│   ├── store.rs              JSON-backed, Mutex-guarded TodoStore
│   ├── templates/
│   │   ├── add.jinja
│   │   ├── done.jinja
│   │   └── list.jinja
│   └── styles/
│       └── todo.css
└── tests/
    └── integration.rs        Full-pipeline tests via TestHarness
```

## Cargo.toml

```toml
[dependencies]
standout = { path = "../standout" }
standout-macros = { path = "../standout-macros" }     # for #[handler]
standout-dispatch = { path = "../standout-dispatch" }  # used by macro-emitted code
standout-input = { path = "../standout-input", default-features = false, features = ["simple-prompts"] }
clap = { version = "4", features = ["derive"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
anyhow = "1"

[dev-dependencies]
standout-test = { path = "../standout-test" }
serial_test = "3"
tempfile = "3"
```

A few notes:

- `standout-input` has heavier optional backends (`editor` opens `$EDITOR`; `inquire` pulls in a TUI prompt library). `simple-prompts` is enough for `--title` + piped stdin and pulls in nothing extra.
- `standout-dispatch` only appears here because the `#[handler]` macro emits `::standout_dispatch::…` paths in user crates. (In a published-version setup this is just `standout-dispatch = "7"`.)
- `standout-test` and `serial_test` are dev-only. The harness mutates process-global state, so its tests must be `#[serial]`.

## `src/store.rs` — the domain

A 70-line JSON-backed store. Mutex inside, `&self` outside, so handlers can stay pure (`fn list(&self)`, `fn add(&self, ...)`) and `app_state` can hand out a shared reference.

In a real app this would be a SQLite handle, a service client, etc. The shape doesn't change — handlers receive a `&Store` and call methods on it.

> **Option:** you could also keep state as a plain `struct` and pass it through `App::builder()` as an `FnMut` closure capturing `&mut store`. Standout supports that natively (no `Arc<Mutex>`); see the "Mutable Handlers" section of the Standout intro guide. We use interior mutability here because it pairs more cleanly with the `#[handler]` macro and `app_state`.

## `src/handlers.rs` — pure functions returning data

This is where Standout's central design rule shows up: handlers return data; the framework renders it.

```rust
#[handler]
pub fn list(
    #[flag] all: bool,
    #[ctx] ctx: &CommandContext,
) -> Result<Output<TodoListResult>, anyhow::Error> {
    let store = ctx.app_state.get_required::<TodoStore>()?;
    let mut todos = store.list();
    if !all { todos.retain(|t| !t.done); }
    let total = todos.len();
    Ok(Output::Render(TodoListResult { todos, total }))
}
```

What's going on here:

- `#[handler]` is a proc macro that turns this into a normal Standout handler. It generates `list__handler(matches, ctx)` next to your function and handles the `m.get_flag("all")` / `m.get_one::<u32>("id")` plumbing for you. The original function is preserved, so a unit test can call `list(true, &ctx)` directly.
- `#[flag]` → `bool` (e.g. `--all`); `#[arg]` → required positional or `Option<T>` for optional, or `Vec<T>` for multiple; `#[ctx]` → `&CommandContext`. `#[matches]` is the escape hatch when you need raw `&ArgMatches`.
- The return type is `Result<Output<T>, anyhow::Error>`, which is exactly what `HandlerResult<T>` expands to. Returning `Output::Render(value)` hands `value` to the renderer; `Output::Silent` skips rendering; `Output::Binary { data, filename }` writes raw bytes.

> **Option:** the `#[handler]` macro can be skipped — you can write `pub fn list(matches: &ArgMatches, ctx: &CommandContext) -> HandlerResult<…>` directly. The `#[derive(Dispatch)]` macro on a clap `Subcommand` enum then auto-wires variants to handlers by name. We use the explicit `App::builder().command_with(…)` form here so we can attach an InputChain to one specific command — see `lib.rs`.

`add` shows the InputChain integration. The handler body has no idea where the title came from:

```rust
#[handler]
pub fn add(#[ctx] ctx: &CommandContext) -> Result<Output<TodoActionResult>, anyhow::Error> {
    let title: &String = ctx.input("title")?;
    /* ... */
}
```

The chain is registered in `lib.rs` and resolved in pre-dispatch — by the time the handler runs, the value is sitting in `ctx.extensions`.

## `src/templates/` — content, declaratively

Three templates, one per command. Standout's templating is MiniJinja with two extras:

- **BBCode-style style tags**: `[stylename]content[/stylename]` wrap text in a style defined in the stylesheet. Tags can be nested, and they're stripped automatically in non-styled output modes (text, JSON, etc.).
- **Tabular filters** like `col(width, align=…, truncate=…)` pad/truncate values to a column width. `col("fill", width=N)` fills remaining space.

`list.jinja` is the only one that uses `col`:

```jinja
[title]Your Todos[/title] [muted]({{ total }})[/muted]
{% if total == 0 %}
[muted]Nothing here yet. Add one with `tdoo add "<title>"`.[/muted]
{%- else %}
{% for todo in todos -%}
{%- set status = "done" if todo.done else "pending" -%}
[index]{{ ("#" ~ todo.id) | col(4, align="right") }}[/index]  [{{ status }}]{{ todo.title | col(40, truncate="end") }}[/{{ status }}]
{% endfor -%}
{%- endif %}
```

Two patterns worth pointing out:

- `[{{ status }}]…[/{{ status }}]` lets a value drive the style — `done` items get the `.done` style, `pending` items get `.pending`. This is the canonical way to express conditional styling without `{% if %}` chains.
- `col(4, align="right")` and `col(40, truncate="end")` give us a real columnar layout for free. For something more elaborate (full `#[derive(Tabular)]` on the row type with the framework's `list-view` template handling layout for you) see the [Tabular guide](https://standout.magik.works/topics/tabular.html). For a list of three columns this is plenty.

> **Option:** Standout also accepts inline template *strings* via `App::builder().command(name, handler, "template body here")`. Files are the canonical form because they get diff-friendly version control, IDE syntax highlighting, hot reload in debug builds, and a place for non-engineers to edit copy.

## `src/styles/todo.css` — adaptive theming

Standout reads CSS, not a custom DSL, and supports `@media (prefers-color-scheme: …)` for light/dark adaptation.

```css
.title { color: cyan; font-weight: bold; }
.done  { color: gray; text-decoration: line-through; }
.pending { font-weight: bold; }

@media (prefers-color-scheme: light) {
    .pending { color: black; }
    .title   { color: blue; }
}
@media (prefers-color-scheme: dark) {
    .pending { color: white; }
}
```

Standout picks light vs dark from `$COLORFGBG` and other terminal signals at runtime; the same binary reskins itself per terminal.

> **Option:** YAML stylesheets work too (`themes/foo.yaml`, with the same `light:` / `dark:` keys), but CSS is the recommended surface. A class can also use theme-relative colors like `cube(60%, 20%, 0%)` — see the [Styling guide](https://standout.magik.works/topics/rendering-system.html).

The filename is the theme name. `todo.css` → theme `"todo"`, activated via `.default_theme("todo")` on the App builder.

## `src/lib.rs` — App wiring

The actual Standout configuration lives here. `main.rs` is just `build_app(store).run(cli, args)` — the lib boundary lets the integration tests build the same `App` the binary builds.

```rust
let app = App::builder()
    .app_state(store)
    .templates(embed_templates!("src/templates"))
    .styles(embed_styles!("src/styles"))
    .default_theme("todo")
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
```

Reading top-down:

- **`app_state(store)`**: a process-lifetime injection slot keyed by type. Handlers reach it with `ctx.app_state.get_required::<TodoStore>()`. Multiple `app_state(...)` calls are fine, one per type.
- **`embed_templates!("src/templates")` / `embed_styles!("src/styles")`**: walk those directories at compile time and embed the contents in the binary. In debug builds the original paths are also watched so edits to `.jinja` / `.css` show up without recompiling.
- **`default_theme("todo")`**: pick a stylesheet by basename. Without this, the framework's built-in default theme is used.
- **`command_with(path, handler, configure)`**: register a leaf handler with extra config. The handler is the `*__handler` function the macro generates. The configure closure receives a `CommandConfig` that exposes `.template(...)`, `.input(...)`, `.pre_dispatch(...)`, `.post_dispatch(...)`, `.post_output(...)`, `.pipe_to(...)`, `.pipe_through(...)`, `.pipe_to_clipboard()`.
- **`.input("title", chain)`**: register a declarative input chain. The chain is resolved in pre-dispatch and the value lands in `ctx.extensions`, reachable through `ctx.input::<String>("title")`. Chain sources tried in order: `ArgSource`, `StdinSource`, `EnvSource`, `EditorSource` (with `editor` feature), `ClipboardSource`, `TextPromptSource`, etc. `.default(value)` provides a fallback; `.validate(predicate, msg)` gates the resolved value.
- **`.post_dispatch(audit_hook)`**: a function-shaped hook that runs after the handler returns and before rendering. It receives the handler's output as `serde_json::Value` and returns a (possibly modified) value. Pre-dispatch hooks can mutate `ctx.extensions`; post-output hooks can transform the rendered text. See the audit-hook implementation at the bottom of `lib.rs`.

> **Option:** for many commands without per-command config, the typing-saving form is `#[derive(Dispatch)]` on a clap `Subcommand` enum + `App::builder().commands(Commands::dispatch_config())`. That auto-maps each variant to `handlers::{snake_case}` and uses templates named after the command. Per-variant attributes (`#[dispatch(template = "…", post_dispatch = …, pipe_to_clipboard, …)]`) cover most needs. We use `command_with` here because we wanted one `.input()` chain — that's not (yet) expressible as a variant attribute.

## `tests/integration.rs` — the test harness

Standout's testability claim is that you can drive the full pipeline — clap parsing, dispatch, hooks, rendering — entirely in-process, with controlled env vars / stdin / clipboard / fixtures. `standout-test::TestHarness` is the bundled API.

```rust
#[test]
#[serial]
fn add_reads_title_from_piped_stdin_when_arg_is_absent() {
    let (app, _dir) = fresh_app();

    let added = TestHarness::new()
        .no_color()
        .piped_stdin("ship the docs\n")
        .run(&app, cli_command(), ["tdoo", "add"]);

    added.assert_success();
    added.assert_stdout_contains("ship the docs");
}
```

What's covered:

- `TestHarness::new()` — empty builder, no overrides until `.run(...)`.
- `.no_color()` — strip ANSI from rendered output for stable string comparisons.
- `.piped_stdin(content)` — simulate `echo content | tdoo add`. The InputChain's `StdinSource` picks this up.
- `.env("KEY", "value")` / `.env_remove("KEY")` — set or unset for the run; restored on drop. Used in the audit-hook test to point `TODO_AUDIT_LOG` at a tempfile.
- `.fixture(path, content)` — write a file into a tempdir and `chdir` there. Useful when the app reads config or data from cwd-relative paths.
- `.clipboard(content)` — mock clipboard read.
- `.terminal_width(n)` / `.no_color()` / `.is_tty()` — control the rendering environment.
- `.run(&app, cmd, argv)` — execute the pipeline. Returns a `TestResult` with `assert_success()`, `assert_stdout_contains(s)`, `assert_stdout_eq(s)`, and `.stdout() -> &str` for custom checks.

Every override is restored on drop, including on panic. Because they touch process-global state, **every TestHarness test must be `#[serial]`** (re-exported as `standout_test::serial`).

The test file is small (about 130 lines) but covers seven distinct scenarios — empty-state rendering, the JSON output mode, the InputChain validator firing, the audit hook side effect, the round-trip through the store, etc. None of them spawn a subprocess.

> **Option:** for tests that just need to assert on a handler's *data* (not the rendered output), call the original `list(true, &ctx)` function directly — `#[handler]` keeps the original function callable. That's the lowest-friction unit-test path. `TestHarness` is for end-to-end checks that need to verify the templating, output mode, or environment behavior.

## Trying it

```bash
# Default store path is $HOME/.todos.json — override per-run with $TODO_FILE.
TODO_FILE=/tmp/tdoo.json cargo run -p todo-example -- add --title "buy milk"
TODO_FILE=/tmp/tdoo.json cargo run -p todo-example -- list
TODO_FILE=/tmp/tdoo.json cargo run -p todo-example -- done 1
TODO_FILE=/tmp/tdoo.json cargo run -p todo-example -- list --all

# Pipe input.
echo "write tests" | TODO_FILE=/tmp/tdoo.json cargo run -p todo-example -- add

# Free output modes.
TODO_FILE=/tmp/tdoo.json cargo run -p todo-example -- list --output json
TODO_FILE=/tmp/tdoo.json cargo run -p todo-example -- list --output yaml
TODO_FILE=/tmp/tdoo.json cargo run -p todo-example -- list --output text

# Run the test suite (every test is #[serial], all in-process).
cargo test -p todo-example
```

## What this example does *not* show

A few things deliberately left out to keep this small:

- **`#[derive(Dispatch)]`** on a clap subcommand enum. That's the lower-boilerplate pattern when you have many commands without per-command config. See `crates/standout/tests/dispatch_derive.rs` for the reference.
- **`#[derive(Tabular)]` + the framework's `list-view` template**. We used the `col` Jinja filter directly because it's more legible for a three-column layout. For richer tables (variable widths, headers, fractional units), see the [Tabular guide](https://standout.magik.works/topics/tabular.html).
- **Output piping** (`pipe_to("tee /tmp/log")`, `pipe_through("jq")`, `pipe_to_clipboard()`). All three are one method call on `cfg` inside `command_with`; we just didn't have a sensible domain reason to wire one up.
- **Topics-based help** — Standout supports rich `--help` topics for long-form docs. See the [Topics guide](https://standout.magik.works/topics/topics-system.html).
- **`standout-seeker`**, the search/select layer — see the Seeker guide if you want it; it's intentionally separate.
