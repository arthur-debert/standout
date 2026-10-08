# Handlers and state

Use the `#[handler]` macro to keep clap extraction outside the typed function:

```rust
use serde::Serialize;
use standout::cli::{CommandContext, Output};
use standout::handler;
use todo_core::{Todo, TodoFilter, TodoStore};

#[derive(Serialize)]
pub struct TodoView {
    pub id: u32,
    pub title: String,
    pub done: bool,
}

impl From<Todo> for TodoView {
    fn from(todo: Todo) -> Self {
        Self { id: todo.id, title: todo.title, done: todo.done }
    }
}

#[derive(Serialize)]
pub struct TodoListView {
    pub todos: Vec<TodoView>,
    pub total: usize,
}

#[handler]
pub fn list(
    #[flag] all: bool,
    #[ctx] ctx: &CommandContext,
) -> Result<Output<TodoListView>, anyhow::Error> {
    let store = ctx.app_state.get_required::<TodoStore>()?;
    let filter = if all { TodoFilter::All } else { TodoFilter::Pending };
    let todos: Vec<_> = store.list(filter).into_iter().map(TodoView::from).collect();
    let total = todos.len();
    Ok(Output::Render(TodoListView { todos, total }))
}
```

Supported parameter annotations are `#[flag]` for booleans, `#[arg]` for typed required/optional/vector values, `#[ctx]`, and `#[matches]`. `name = "cli-name"` overrides the inferred flag or argument name. A `&mut Results<E>` parameter, unannotated, makes the command incremental ([rendering-and-output.md](rendering-and-output.md)).

The macro keeps `list(all, ctx)` and generates the wrapper `list__handler(matches, ctx)`, `list__expected_args()`, and `list_Handler`, a unit struct implementing `Handler`. Register per [app-wiring.md](app-wiring.md); unit-test the typed function:

```rust
let Output::Render(result) = list(false, &ctx).unwrap() else {
    panic!("expected rendered data");
};
assert_eq!(result.total, result.todos.len());
```

## Output contract

`Output<T>` is non-exhaustive; its shapes:

- `Output::Render(data)` renders a template or serializes `data` in a structured mode.
- `Output::Silent` completes without output.
- `Output::Binary { data, filename }` returns bytes and a suggested filename.
- `Output::Artifact(artifact)` returns bytes the framework writes plus a report rendered after the write.
- `Output::WithStatus`, built by `with_exit_status`, is a `Render` or `Silent` with a declared exit status.

View strings render literally, tags and ANSI included; put deliberate styling in a `FormattedText` field or the template ([rendering-and-output.md](rendering-and-output.md)).

Do not branch presentation in a handler. `CommandContext` has `command_path`, `app_state` and per-dispatch `extensions`; `representation()` and `color_policy()` are read-only, for hooks.

## State boundaries

Construct long-lived dependencies in the CLI before app assembly, register them once with `.app_state(value)`, and read them with `ctx.app_state.get_required::<T>()`. Shared mutable state needs interior mutability. File-backed settings go through `.config(builder)` and `ctx.config::<C>()?`, as in `tdoo`.

Inject request-only values in a pre-dispatch hook with `ctx.extensions.insert(value)` and retrieve them with `ctx.extensions.get_required::<T>()`. Read inputs, questionnaire answers and config through `CommandContextInput` (`input`, `questionnaire`, `config`), not from extensions directly.

Signatures: `crates/standout-dispatch/src/handler/`, `crates/standout-macros/src/handler.rs`, `crates/standout/tests/handler_macro.rs`.
