//! Pure handlers. They return data; the framework renders it.
//!
//! Each handler uses the `#[handler]` macro, which extracts CLI args from
//! `ArgMatches` so the body can be written as a normal Rust function. The
//! original function is preserved alongside the generated `*__handler`
//! wrapper, so unit tests in `#[cfg(test)]` can call it directly.

#![allow(non_snake_case)]

use crate::store::{Todo, TodoStore};
use serde::Serialize;
use standout::cli::{CommandContext, CommandContextInput, Output};
use standout_macros::handler;

#[derive(Serialize)]
pub struct TodoListResult {
    pub todos: Vec<Todo>,
    pub total: usize,
}

#[derive(Serialize)]
pub struct TodoActionResult {
    pub message: String,
    pub todo: Todo,
}

// `#[ctx]` gives us read-only access to the CommandContext. `app_state` is
// where the TodoStore lives — registered once at App-build time.
// `#[flag]` and `#[arg]` map to clap flags/positional args; the macro takes
// care of the `m.get_flag(...) / m.get_one::<T>(...)` plumbing.

#[handler]
pub fn list(
    #[flag] all: bool,
    #[ctx] ctx: &CommandContext,
) -> Result<Output<TodoListResult>, anyhow::Error> {
    let store = ctx.app_state.get_required::<TodoStore>()?;
    let mut todos = store.list();
    if !all {
        todos.retain(|t| !t.done);
    }
    let total = todos.len();
    Ok(Output::Render(TodoListResult { todos, total }))
}

// `add` reads its title from a declarative input chain registered in
// `build_app` in `src/lib.rs`: `--title <T>` first, falling back to piped
// stdin. The chain is resolved during pre-dispatch and the value lands in
// `ctx.extensions`, reachable through `ctx.input::<String>("title")`.
#[handler]
pub fn add(#[ctx] ctx: &CommandContext) -> Result<Output<TodoActionResult>, anyhow::Error> {
    let title: &String = ctx.input("title")?;
    let store = ctx.app_state.get_required::<TodoStore>()?;
    let todo = store.add(title.clone())?;
    Ok(Output::Render(TodoActionResult {
        message: format!("Added #{}", todo.id),
        todo,
    }))
}

#[handler]
pub fn done(
    #[arg] id: u32,
    #[ctx] ctx: &CommandContext,
) -> Result<Output<TodoActionResult>, anyhow::Error> {
    let store = ctx.app_state.get_required::<TodoStore>()?;
    let todo = store.mark_done(id)?;
    Ok(Output::Render(TodoActionResult {
        message: format!("Marked #{} done", todo.id),
        todo,
    }))
}
