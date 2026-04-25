use anyhow::Result;
use todo_example::{build_app, cli_command, resolve_store_path, TodoStore};

fn main() -> Result<()> {
    let store = TodoStore::load(resolve_store_path())?;
    let app = build_app(store)?;
    app.run(cli_command(), std::env::args());
    Ok(())
}
