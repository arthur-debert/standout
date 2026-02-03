//! Integration tests for the #[command] proc macro.

#![allow(non_snake_case)] // Generated handler names use __handler suffix

use serde::Serialize;
use serde_json::json;
use standout::cli::handler::{CommandContext, Output};
use standout::command;
use standout_dispatch::verify::verify_handler_args;

// =============================================================================
// Basic command with flag
// =============================================================================

#[derive(Serialize)]
struct ListOutput {
    all: bool,
}

#[command(name = "list", about = "List all items")]
fn list_cmd(
    #[flag(short = 'a', help = "Show all items")] all: bool,
) -> Result<Output<ListOutput>, anyhow::Error> {
    Ok(Output::Render(ListOutput { all }))
}

#[test]
fn test_command_generates_handler() {
    let cmd = list_cmd__command();
    let matches = cmd.try_get_matches_from(["list", "-a"]).unwrap();

    let ctx = CommandContext::default();
    let result = list_cmd__handler(&matches, &ctx);
    assert!(result.is_ok());
}

#[test]
fn test_command_generates_clap_command() {
    let cmd = list_cmd__command();
    assert_eq!(cmd.get_name(), "list");

    // Check that flag is defined
    let arg = cmd.get_arguments().find(|a| a.get_id() == "all");
    assert!(arg.is_some());

    let arg = arg.unwrap();
    assert_eq!(arg.get_short(), Some('a'));
}

#[test]
fn test_command_generates_template() {
    // Template defaults to command name
    assert_eq!(list_cmd__template(), "list");
}

#[test]
fn test_command_generates_expected_args() {
    let expected = list_cmd__expected_args();
    assert_eq!(expected.len(), 1);
    assert_eq!(expected[0].cli_name, "all");
}

#[test]
fn test_command_handler_struct() {
    use standout_dispatch::Handler;

    let mut handler = list_cmd_Handler;
    let cmd = list_cmd__command();
    let matches = cmd.try_get_matches_from(["list"]).unwrap();
    let ctx = CommandContext::default();

    let result = handler.handle(&matches, &ctx);
    assert!(result.is_ok());

    // Handler also exposes expected_args
    let expected = handler.expected_args();
    assert_eq!(expected.len(), 1);
}

#[test]
fn test_command_verification_passes() {
    let cmd = list_cmd__command();
    let expected = list_cmd__expected_args();
    assert!(verify_handler_args(&cmd, "list_cmd", &expected).is_ok());
}

// =============================================================================
// Command with custom template
// =============================================================================

#[command(name = "add", about = "Add an item", template = "add_item_view")]
fn add_cmd(#[arg(help = "Item name")] name: String) -> Result<Output<String>, anyhow::Error> {
    Ok(Output::Render(name))
}

#[test]
fn test_custom_template() {
    assert_eq!(add_cmd__template(), "add_item_view");
}

#[test]
fn test_required_arg() {
    let cmd = add_cmd__command();
    let arg = cmd.get_arguments().find(|a| a.get_id() == "name").unwrap();
    assert!(arg.is_required_set());
}

// =============================================================================
// Command with optional arg
// =============================================================================

#[command(name = "search")]
fn search_cmd(
    #[arg(short = 'q', long = "query", help = "Search query")] query: Option<String>,
) -> Result<Output<Option<String>>, anyhow::Error> {
    Ok(Output::Render(query))
}

#[test]
fn test_optional_arg_present() {
    let cmd = search_cmd__command();
    let matches = cmd.try_get_matches_from(["search", "-q", "test"]).unwrap();

    let ctx = CommandContext::default();
    let result = search_cmd__handler(&matches, &ctx);
    assert!(result.is_ok());
}

#[test]
fn test_optional_arg_missing() {
    let cmd = search_cmd__command();
    let matches = cmd.try_get_matches_from(["search"]).unwrap();

    let ctx = CommandContext::default();
    let result = search_cmd__handler(&matches, &ctx);
    assert!(result.is_ok());
}

// =============================================================================
// Command with multiple parameters
// =============================================================================

// Note: For non-String types like usize, you would need to add value_parser support
// to the #[command] macro. For now, use String and parse manually.
#[command(name = "process", about = "Process items")]
fn process_cmd(
    #[flag(short = 'v', long = "verbose")] verbose: bool,
    #[flag(short = 'd', long = "dry-run", help = "Dry run mode")] dry_run: bool,
    #[arg(short = 'l', long = "limit")] limit: Option<String>,
) -> Result<Output<serde_json::Value>, anyhow::Error> {
    Ok(Output::Render(json!({
        "verbose": verbose,
        "dry_run": dry_run,
        "limit": limit,
    })))
}

#[test]
fn test_multiple_params() {
    let cmd = process_cmd__command();

    // Check all args exist
    assert!(cmd.get_arguments().any(|a| a.get_id() == "verbose"));
    assert!(cmd.get_arguments().any(|a| a.get_id() == "dry-run"));
    assert!(cmd.get_arguments().any(|a| a.get_id() == "limit"));

    let matches = cmd
        .try_get_matches_from(["process", "-v", "--dry-run", "-l", "10"])
        .unwrap();

    let ctx = CommandContext::default();
    let result = process_cmd__handler(&matches, &ctx);
    assert!(result.is_ok());
}

#[test]
fn test_expected_args_for_multiple_params() {
    let expected = process_cmd__expected_args();
    assert_eq!(expected.len(), 3);

    // Find by cli_name
    assert!(expected.iter().any(|e| e.cli_name == "verbose"));
    assert!(expected.iter().any(|e| e.cli_name == "dry-run"));
    assert!(expected.iter().any(|e| e.cli_name == "limit"));
}

// =============================================================================
// Command with positional argument
// =============================================================================

#[command(name = "open")]
fn open_cmd(
    #[arg(positional, help = "File path")] path: String,
) -> Result<Output<String>, anyhow::Error> {
    Ok(Output::Render(path))
}

#[test]
fn test_positional_arg() {
    let cmd = open_cmd__command();
    let matches = cmd.try_get_matches_from(["open", "/path/to/file"]).unwrap();

    let ctx = CommandContext::default();
    let result = open_cmd__handler(&matches, &ctx);
    assert!(result.is_ok());
}

// =============================================================================
// Command with context access
// =============================================================================

#[command(name = "info")]
fn info_cmd(#[ctx] ctx: &CommandContext) -> Result<Output<usize>, anyhow::Error> {
    Ok(Output::Render(ctx.command_path.len()))
}

#[test]
fn test_ctx_access() {
    let cmd = info_cmd__command();
    let matches = cmd.try_get_matches_from(["info"]).unwrap();

    let mut ctx = CommandContext::default();
    ctx.command_path = vec!["app".to_string(), "info".to_string()];

    let result = info_cmd__handler(&matches, &ctx);
    assert!(result.is_ok());
}

#[test]
fn test_ctx_not_in_expected_args() {
    // #[ctx] should not appear in expected_args (it's pass-through, not CLI arg)
    let expected = info_cmd__expected_args();
    assert!(expected.is_empty());
}

// =============================================================================
// Command with Vec argument
// =============================================================================

#[command(name = "tag")]
fn tag_cmd(
    #[arg(short = 't', long = "tag", help = "Tags to apply")] tags: Vec<String>,
) -> Result<Output<Vec<String>>, anyhow::Error> {
    Ok(Output::Render(tags))
}

#[test]
fn test_vec_arg() {
    let cmd = tag_cmd__command();
    let matches = cmd
        .try_get_matches_from(["tag", "-t", "foo", "-t", "bar"])
        .unwrap();

    let ctx = CommandContext::default();
    let result = tag_cmd__handler(&matches, &ctx);
    assert!(result.is_ok());
}

// =============================================================================
// Original function preserved for testing
// =============================================================================

#[test]
fn test_original_function_preserved() {
    // Can call the original function directly
    let result = list_cmd(true);
    assert!(result.is_ok());

    let result = add_cmd("test".to_string());
    assert!(result.is_ok());
}

// =============================================================================
// Command with default value
// =============================================================================

#[command(name = "paginate")]
fn paginate_cmd(
    #[arg(long = "page-size", default = "20", help = "Items per page")] page_size: String,
) -> Result<Output<String>, anyhow::Error> {
    Ok(Output::Render(page_size))
}

#[test]
fn test_default_value() {
    let cmd = paginate_cmd__command();

    // Without providing the arg, should use default
    let matches = cmd.clone().try_get_matches_from(["paginate"]).unwrap();
    let ctx = CommandContext::default();
    let result = paginate_cmd__handler(&matches, &ctx);
    assert!(result.is_ok());

    // With explicit value
    let matches = cmd
        .try_get_matches_from(["paginate", "--page-size", "50"])
        .unwrap();
    let result = paginate_cmd__handler(&matches, &ctx);
    assert!(result.is_ok());
}
