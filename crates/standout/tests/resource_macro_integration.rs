use clap::{Command, Parser, Subcommand};
use serde::{Deserialize, Serialize};
use standout::cli::handler::Extensions;
use standout::cli::{CommandContext, ResourceQuery, ResourceStore};
use standout_macros::{Resource, Tabular};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

// ============================================================================
// Test Fixtures
// ============================================================================

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Resource, Tabular)]
#[resource(object = "task", store = InMemoryTaskStore)]
struct Task {
    #[resource(id)]
    #[tabular(name = "ID")]
    pub id: String,

    #[resource(arg(short, long))]
    #[tabular(name = "TITLE")]
    pub title: String,

    #[resource(arg(short, long), default = "pending")]
    #[tabular(name = "STATUS")]
    pub status: String,
}

#[derive(Debug)]
struct TestError(String);

impl std::fmt::Display for TestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for TestError {}

struct InMemoryTaskStore {
    tasks: RwLock<HashMap<String, Task>>,
}

impl InMemoryTaskStore {
    fn new() -> Self {
        Self {
            tasks: RwLock::new(HashMap::new()),
        }
    }
}

impl ResourceStore for InMemoryTaskStore {
    type Item = Task;
    type Id = String;
    type Error = TestError;

    fn parse_id(&self, id_str: &str) -> Result<Self::Id, Self::Error> {
        Ok(id_str.to_string())
    }

    fn get(&self, id: &Self::Id) -> Result<Option<Self::Item>, Self::Error> {
        Ok(self.tasks.read().unwrap().get(id).cloned())
    }

    fn not_found_error(id: &Self::Id) -> Self::Error {
        TestError(format!("Task '{}' not found", id))
    }

    fn list(&self, _query: Option<&ResourceQuery>) -> Result<Vec<Self::Item>, Self::Error> {
        let tasks = self.tasks.read().unwrap();
        Ok(tasks.values().cloned().collect())
    }

    fn create(&self, mut data: serde_json::Value) -> Result<Self::Item, Self::Error> {
        if data.get("id").is_none() {
            if let Some(obj) = data.as_object_mut() {
                obj.insert("id".to_string(), serde_json::json!("gen-1"));
            }
        }
        let task: Task = serde_json::from_value(data).map_err(|e| TestError(e.to_string()))?;
        self.tasks
            .write()
            .unwrap()
            .insert(task.id.clone(), task.clone());
        Ok(task)
    }

    fn update(&self, id: &Self::Id, data: serde_json::Value) -> Result<Self::Item, Self::Error> {
        let mut tasks = self.tasks.write().unwrap();
        let task = tasks.get_mut(id).ok_or_else(|| Self::not_found_error(id))?;

        if let Some(title) = data.get("title").and_then(|v| v.as_str()) {
            task.title = title.to_string();
        }
        if let Some(status) = data.get("status").and_then(|v| v.as_str()) {
            task.status = status.to_string();
        }

        Ok(task.clone())
    }

    fn delete(&self, id: &Self::Id) -> Result<(), Self::Error> {
        let mut tasks = self.tasks.write().unwrap();
        tasks.remove(id).ok_or_else(|| Self::not_found_error(id))?;
        Ok(())
    }
}

// Helper to drive the generated CLI
#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: TaskCommands,
}

#[test]
fn test_macro_create_flow() {
    let store = InMemoryTaskStore::new();
    let mut app_state = Extensions::new();
    app_state.insert(store);

    let ctx = CommandContext::new(vec!["app".to_string()], Arc::new(app_state));
    // Needed to access the store later
    let store = ctx.app_state.get_required::<InMemoryTaskStore>().unwrap();

    // Test Create
    let cmd = TaskCommands::augment_subcommands(Command::new("app"));
    // Removed --id, handled by store
    let matches = cmd
        .try_get_matches_from(vec!["app", "create", "--title", "My Task"])
        .unwrap();
    let subcommand_matches = matches.subcommand_matches("create").unwrap();

    let result = __task_resource_handlers::create(subcommand_matches, &ctx).unwrap();

    // Verify output type
    if let standout::cli::Output::Render(val) = result {
        let json_str = serde_json::to_string(&val).unwrap();
        assert!(json_str.contains("My Task"));
    } else {
        panic!("Expected Render output");
    }

    // Verify store state
    let task = store.get(&"gen-1".to_string()).unwrap().unwrap();
    assert_eq!(task.title, "My Task");
    assert_eq!(task.status, "pending"); // Default value
}

#[test]
fn test_macro_update_flow() {
    let store = InMemoryTaskStore::new();
    store
        .create(serde_json::json!({
            "id": "t1",
            "title": "Old Title",
            "status": "pending"
        }))
        .unwrap();

    let mut app_state = Extensions::new();
    app_state.insert(store);

    let ctx = CommandContext::new(vec!["app".to_string()], Arc::new(app_state));
    let store = ctx.app_state.get_required::<InMemoryTaskStore>().unwrap();

    let cmd = TaskCommands::augment_subcommands(Command::new("app"));
    let matches = cmd
        .try_get_matches_from(vec!["app", "update", "t1", "--title", "New Title"])
        .unwrap();
    let subcommand_matches = matches.subcommand_matches("update").unwrap();

    let result = __task_resource_handlers::update(subcommand_matches, &ctx).unwrap();

    if let standout::cli::Output::Render(val) = result {
        let json_str = serde_json::to_string(&val).unwrap();
        assert!(json_str.contains("New Title"));
    } else {
        panic!("Expected Render output");
    }

    let task = store.get(&"t1".to_string()).unwrap().unwrap();
    assert_eq!(task.title, "New Title");
}

// ============================================================================
// Default Subcommand Help Text Test
// ============================================================================

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Resource, Tabular)]
#[resource(object = "item", store = InMemoryItemStore, default = "list")]
struct Item {
    #[resource(id)]
    #[tabular(name = "ID")]
    pub id: String,

    #[tabular(name = "NAME")]
    pub name: String,
}

struct InMemoryItemStore;

impl ResourceStore for InMemoryItemStore {
    type Item = Item;
    type Id = String;
    type Error = TestError;

    fn parse_id(&self, id_str: &str) -> Result<Self::Id, Self::Error> {
        Ok(id_str.to_string())
    }

    fn get(&self, _id: &Self::Id) -> Result<Option<Self::Item>, Self::Error> {
        Ok(None)
    }

    fn not_found_error(id: &Self::Id) -> Self::Error {
        TestError(format!("Item '{}' not found", id))
    }

    fn list(&self, _query: Option<&ResourceQuery>) -> Result<Vec<Self::Item>, Self::Error> {
        Ok(vec![])
    }

    fn create(&self, data: serde_json::Value) -> Result<Self::Item, Self::Error> {
        serde_json::from_value(data).map_err(|e| TestError(e.to_string()))
    }

    fn update(&self, id: &Self::Id, _data: serde_json::Value) -> Result<Self::Item, Self::Error> {
        Err(Self::not_found_error(id))
    }

    fn delete(&self, id: &Self::Id) -> Result<(), Self::Error> {
        Err(Self::not_found_error(id))
    }
}

#[derive(Parser)]
struct ItemCli {
    #[command(subcommand)]
    command: ItemCommands,
}

#[test]
fn test_default_subcommand_help_text() {
    // Build the command and render help
    let mut cmd = ItemCommands::augment_subcommands(Command::new("item"));
    let help = cmd.render_help().to_string();

    // Verify the help text mentions the default subcommand
    assert!(
        help.contains("'list' is used by default"),
        "Help text should indicate 'list' is the default subcommand. Got:\n{}",
        help
    );
}

#[test]
fn test_default_command_method() {
    // Verify the default_command() method returns the configured default
    assert_eq!(ItemCommands::default_command(), Some("list"));

    // Task doesn't have a default configured
    assert_eq!(TaskCommands::default_command(), None);
}

// ============================================================================
// Short Option Tests
// ============================================================================

/// This struct tests that unique short options work correctly.
/// Note: Duplicate short options (e.g., two fields with `short = 't'`)
/// will produce a compile-time error: "duplicate short option '-t': already used by field 'x'"
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Resource, Tabular)]
#[resource(object = "widget", store = InMemoryWidgetStore)]
struct Widget {
    #[resource(id)]
    #[tabular(name = "ID")]
    pub id: String,

    #[resource(arg(short = 'n'))]
    #[tabular(name = "NAME")]
    pub name: String,

    #[resource(arg(short = 'd'))]
    #[tabular(name = "DESC")]
    pub description: String,

    #[resource(arg(short = 'c'))]
    #[tabular(name = "COUNT")]
    pub count: Option<u32>,
}

struct InMemoryWidgetStore;

impl ResourceStore for InMemoryWidgetStore {
    type Item = Widget;
    type Id = String;
    type Error = TestError;

    fn parse_id(&self, id_str: &str) -> Result<Self::Id, Self::Error> {
        Ok(id_str.to_string())
    }

    fn get(&self, _id: &Self::Id) -> Result<Option<Self::Item>, Self::Error> {
        Ok(None)
    }

    fn not_found_error(id: &Self::Id) -> Self::Error {
        TestError(format!("Widget '{}' not found", id))
    }

    fn list(&self, _query: Option<&ResourceQuery>) -> Result<Vec<Self::Item>, Self::Error> {
        Ok(vec![])
    }

    fn create(&self, data: serde_json::Value) -> Result<Self::Item, Self::Error> {
        serde_json::from_value(data).map_err(|e| TestError(e.to_string()))
    }

    fn update(&self, id: &Self::Id, _data: serde_json::Value) -> Result<Self::Item, Self::Error> {
        Err(Self::not_found_error(id))
    }

    fn delete(&self, id: &Self::Id) -> Result<(), Self::Error> {
        Err(Self::not_found_error(id))
    }
}

#[test]
fn test_unique_short_options_compile() {
    // This test verifies that the Widget struct with unique short options compiles.
    // If there were duplicate short options, this would fail at compile time.
    let cmd = WidgetCommands::augment_subcommands(Command::new("widget"));

    // Get the create subcommand's help (where short options are defined)
    let create_cmd = cmd
        .find_subcommand("create")
        .expect("create subcommand exists");
    let help = create_cmd.clone().render_help().to_string();

    // Verify short options appear in help
    assert!(
        help.contains("-n"),
        "Help should show -n short option. Got:\n{}",
        help
    );
    assert!(
        help.contains("-d"),
        "Help should show -d short option. Got:\n{}",
        help
    );
    assert!(
        help.contains("-c"),
        "Help should show -c short option. Got:\n{}",
        help
    );
}

// ============================================================================
// Keep Aliases Tests
// ============================================================================

/// This struct tests that keep_aliases preserves original command names as hidden aliases.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Resource, Tabular)]
#[resource(
    object = "project",
    store = InMemoryProjectStore,
    aliases(view = "show", delete = "rm"),
    keep_aliases
)]
struct Project {
    #[resource(id)]
    #[tabular(name = "ID")]
    pub id: String,

    #[tabular(name = "NAME")]
    pub name: String,
}

struct InMemoryProjectStore;

impl ResourceStore for InMemoryProjectStore {
    type Item = Project;
    type Id = String;
    type Error = TestError;

    fn parse_id(&self, id_str: &str) -> Result<Self::Id, Self::Error> {
        Ok(id_str.to_string())
    }

    fn get(&self, _id: &Self::Id) -> Result<Option<Self::Item>, Self::Error> {
        Ok(None)
    }

    fn not_found_error(id: &Self::Id) -> Self::Error {
        TestError(format!("Project '{}' not found", id))
    }

    fn list(&self, _query: Option<&ResourceQuery>) -> Result<Vec<Self::Item>, Self::Error> {
        Ok(vec![])
    }

    fn create(&self, data: serde_json::Value) -> Result<Self::Item, Self::Error> {
        serde_json::from_value(data).map_err(|e| TestError(e.to_string()))
    }

    fn update(&self, id: &Self::Id, _data: serde_json::Value) -> Result<Self::Item, Self::Error> {
        Err(Self::not_found_error(id))
    }

    fn delete(&self, id: &Self::Id) -> Result<(), Self::Error> {
        Err(Self::not_found_error(id))
    }
}

#[test]
fn test_keep_aliases_preserves_original_names() {
    let cmd = ProjectCommands::augment_subcommands(Command::new("project"));

    // Verify aliased commands exist with new names
    assert!(
        cmd.find_subcommand("show").is_some(),
        "Should have 'show' command (aliased from 'view')"
    );
    assert!(
        cmd.find_subcommand("rm").is_some(),
        "Should have 'rm' command (aliased from 'delete')"
    );

    // Verify original names work as aliases (they're hidden but still functional)
    // We can test this by trying to get matches with the original name
    let test_cmd = ProjectCommands::augment_subcommands(Command::new("project"));
    let result = test_cmd.try_get_matches_from(vec!["project", "view", "123"]);
    assert!(
        result.is_ok(),
        "Original 'view' name should work as hidden alias"
    );

    let test_cmd = ProjectCommands::augment_subcommands(Command::new("project"));
    let result = test_cmd.try_get_matches_from(vec!["project", "delete", "123"]);
    assert!(
        result.is_ok(),
        "Original 'delete' name should work as hidden alias"
    );
}
