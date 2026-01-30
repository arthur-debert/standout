//! Integration tests for ListView macro dispatch.

use clap::{ArgMatches, Subcommand};
use serde::Serialize;
use standout::cli::{CommandContext, Dispatch, GroupBuilder, HandlerResult, Output};
use standout::views::list_view;
use standout::{Tabular, TabularRow};

// Define a Tabular item
#[derive(Serialize, Tabular, TabularRow, Clone)]
struct Task {
    #[col(width = 5)]
    id: u32,
    #[col(width = 20)]
    name: String,
}

mod handlers {
    use super::*;

    // Handler returns ListViewResult
    // Note: We don't set tabular_spec manually!
    pub fn list(
        _matches: &ArgMatches,
        _ctx: &CommandContext,
    ) -> HandlerResult<standout::views::ListViewResult<Task>> {
        let tasks = vec![Task {
            id: 1,
            name: "Task 1".to_string(),
        }];
        Ok(Output::Render(list_view(tasks).build()))
    }
}

#[derive(Subcommand, Dispatch)]
#[dispatch(handlers = handlers)]
enum Commands {
    #[dispatch(list_view, item_type = "Task")]
    List,
}

#[test]
fn test_list_view_macro_injection() {
    // 1. Get the dispatch config
    let config = Commands::dispatch_config();
    let builder = config(GroupBuilder::new());

    // 2. Verify command is registered
    assert!(builder.contains("list"));

    // 3. Get the build result (mocking dispatch flow)
    // We need to access the stored handler closure to run it.
    // GroupBuilder stores GroupEntry::Command { handler: Box<dyn ErasedCommandConfig> }
    // We can call register() on it to get DispatchFn.

    // This part is internal API usage, might be tricky from integration test.
    // But we can rebuild App?
    // standout::cli::AppBuilder employs GroupBuilder logic.

    use standout::cli::App;
    let app = App::<standout::cli::ThreadSafe>::builder()
        .commands(Commands::dispatch_config())
        .expect("Failed to set commands")
        .build()
        .expect("Failed to build app");

    // 4. Dispatch 'list' command with JSON output mode
    // We need to construct clap matches.
    let cmd = clap::Command::new("test").subcommand(clap::Command::new("list"));
    let matches = cmd.try_get_matches_from(vec!["test", "list"]).unwrap();

    // We manually dispatch to inspect result
    // App::dispatch returns DispatchResult which has output() method.
    let result = app.dispatch(matches, standout::OutputMode::Json);

    assert!(result.is_handled());
    let output = result.output().expect("Expected output");

    // 5. Verify JSON contains tabular_spec
    // Since we added tabular_spec to ListViewResult and the macro should inject it
    println!("Output: {}", output);
    assert!(
        output.contains("\"tabular_spec\""),
        "Output should contain tabular_spec when list_view macro is used"
    );
    assert!(output.contains("\"width\": 5")); // Check content of spec
}
