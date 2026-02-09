use clap::{Command, Subcommand};
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
#[resource(
    object = "task",
    store = InMemoryTaskStore,
    shortcut(name = "complete", sets(status = "done")),
    shortcut(name = "reopen", sets(status = "pending"))
)]
struct Task {
    #[resource(id)]
    #[tabular(name = "ID")]
    pub id: String,

    #[resource(arg(long))]
    #[tabular(name = "TITLE")]
    pub title: String,

    #[resource(arg(long), default = "pending")]
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

    fn seed(&self, tasks: Vec<Task>) {
        let mut store = self.tasks.write().unwrap();
        for task in tasks {
            store.insert(task.id.clone(), task);
        }
    }
}

impl ResourceStore for InMemoryTaskStore {
    type Item = Task;
    type Id = String;
    type Error = TestError;

    fn parse_id(&self, id_str: &str) -> Result<Self::Id, Self::Error> {
        if id_str.is_empty() {
            return Err(TestError("ID cannot be empty".to_string()));
        }
        Ok(id_str.to_string())
    }

    fn get(&self, id: &Self::Id) -> Result<Option<Self::Item>, Self::Error> {
        Ok(self.tasks.read().unwrap().get(id).cloned())
    }

    fn not_found_error(id: &Self::Id) -> Self::Error {
        TestError(format!("Task '{}' not found", id))
    }

    fn list(&self, _query: Option<&ResourceQuery>) -> Result<Vec<Self::Item>, Self::Error> {
        Ok(self.tasks.read().unwrap().values().cloned().collect())
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

fn make_ctx(store: InMemoryTaskStore) -> CommandContext {
    let mut app_state = Extensions::new();
    app_state.insert(store);
    CommandContext::new(vec!["app".to_string()], Arc::new(app_state))
}

// ============================================================================
// Shortcut Subcommand Registration
// ============================================================================

#[test]
fn test_shortcut_subcommands_registered() {
    let names = TaskCommands::subcommand_names();
    assert!(
        names.contains(&"complete"),
        "Should have 'complete' shortcut"
    );
    assert!(names.contains(&"reopen"), "Should have 'reopen' shortcut");
    // Still has standard CRUD
    assert!(names.contains(&"list"));
    assert!(names.contains(&"view"));
    assert!(names.contains(&"create"));
    assert!(names.contains(&"update"));
    assert!(names.contains(&"delete"));
    assert_eq!(names.len(), 7, "5 CRUD + 2 shortcuts");
}

#[test]
fn test_shortcut_clap_command_exists() {
    let cmd = TaskCommands::augment_subcommands(Command::new("app"));
    assert!(
        cmd.find_subcommand("complete").is_some(),
        "complete subcommand should exist"
    );
    assert!(
        cmd.find_subcommand("reopen").is_some(),
        "reopen subcommand should exist"
    );
}

// ============================================================================
// Single-ID Shortcut Dispatch
// ============================================================================

#[test]
fn test_shortcut_complete_single_id() {
    let store = InMemoryTaskStore::new();
    store.seed(vec![Task {
        id: "t1".to_string(),
        title: "My Task".to_string(),
        status: "pending".to_string(),
    }]);

    let ctx = make_ctx(store);
    let store = ctx.app_state.get_required::<InMemoryTaskStore>().unwrap();

    let cmd = TaskCommands::augment_subcommands(Command::new("app"));
    let matches = cmd
        .try_get_matches_from(vec!["app", "complete", "t1"])
        .unwrap();
    let sub = matches.subcommand_matches("complete").unwrap();

    let result = __task_resource_handlers::shortcut_complete(sub, &ctx);
    assert!(
        result.is_ok(),
        "Shortcut should succeed: {:?}",
        result.err()
    );

    // Verify store was updated
    let task = store.get(&"t1".to_string()).unwrap().unwrap();
    assert_eq!(
        task.status, "done",
        "Status should be set to 'done' by shortcut"
    );
    assert_eq!(task.title, "My Task", "Title should be unchanged");
}

#[test]
fn test_shortcut_reopen_single_id() {
    let store = InMemoryTaskStore::new();
    store.seed(vec![Task {
        id: "t1".to_string(),
        title: "My Task".to_string(),
        status: "done".to_string(),
    }]);

    let ctx = make_ctx(store);
    let store = ctx.app_state.get_required::<InMemoryTaskStore>().unwrap();

    let cmd = TaskCommands::augment_subcommands(Command::new("app"));
    let matches = cmd
        .try_get_matches_from(vec!["app", "reopen", "t1"])
        .unwrap();
    let sub = matches.subcommand_matches("reopen").unwrap();

    let result = __task_resource_handlers::shortcut_reopen(sub, &ctx);
    assert!(result.is_ok());

    let task = store.get(&"t1".to_string()).unwrap().unwrap();
    assert_eq!(
        task.status, "pending",
        "Status should be set back to 'pending'"
    );
}

#[test]
fn test_shortcut_returns_update_view_output() {
    let store = InMemoryTaskStore::new();
    store.seed(vec![Task {
        id: "t1".to_string(),
        title: "My Task".to_string(),
        status: "pending".to_string(),
    }]);

    let ctx = make_ctx(store);

    let cmd = TaskCommands::augment_subcommands(Command::new("app"));
    let matches = cmd
        .try_get_matches_from(vec!["app", "complete", "t1"])
        .unwrap();
    let sub = matches.subcommand_matches("complete").unwrap();

    let result = __task_resource_handlers::shortcut_complete(sub, &ctx).unwrap();
    if let standout::cli::Output::Render(val) = result {
        // Should contain update view structure with changed_fields
        let json_str = serde_json::to_string(&val).unwrap();
        assert!(
            json_str.contains("done"),
            "Output should contain new status"
        );
        assert!(
            json_str.contains("changed_fields"),
            "Output should track changed fields"
        );
        assert!(
            json_str.contains("status"),
            "Changed fields should include 'status'"
        );
    } else {
        panic!("Expected Render output from shortcut");
    }
}

// ============================================================================
// Shortcut Error Handling
// ============================================================================

#[test]
fn test_shortcut_not_found() {
    let store = InMemoryTaskStore::new();
    let ctx = make_ctx(store);

    let cmd = TaskCommands::augment_subcommands(Command::new("app"));
    let matches = cmd
        .try_get_matches_from(vec!["app", "complete", "nonexistent"])
        .unwrap();
    let sub = matches.subcommand_matches("complete").unwrap();

    let result = __task_resource_handlers::shortcut_complete(sub, &ctx);
    assert!(result.is_err(), "Should fail for nonexistent ID");
}

// ============================================================================
// Batch ID Shortcuts
// ============================================================================

#[test]
fn test_shortcut_batch_multiple_ids() {
    let store = InMemoryTaskStore::new();
    store.seed(vec![
        Task {
            id: "t1".to_string(),
            title: "Task 1".to_string(),
            status: "pending".to_string(),
        },
        Task {
            id: "t2".to_string(),
            title: "Task 2".to_string(),
            status: "pending".to_string(),
        },
        Task {
            id: "t3".to_string(),
            title: "Task 3".to_string(),
            status: "pending".to_string(),
        },
    ]);

    let ctx = make_ctx(store);
    let store = ctx.app_state.get_required::<InMemoryTaskStore>().unwrap();

    let cmd = TaskCommands::augment_subcommands(Command::new("app"));
    let matches = cmd
        .try_get_matches_from(vec!["app", "complete", "t1", "t2", "t3"])
        .unwrap();
    let sub = matches.subcommand_matches("complete").unwrap();

    let result = __task_resource_handlers::shortcut_complete(sub, &ctx);
    assert!(result.is_ok(), "Batch shortcut should succeed");

    // All three should be updated
    for id in &["t1", "t2", "t3"] {
        let task = store.get(&id.to_string()).unwrap().unwrap();
        assert_eq!(task.status, "done", "Task {} should be completed", id);
    }
}

#[test]
fn test_shortcut_batch_partial_failure() {
    let store = InMemoryTaskStore::new();
    store.seed(vec![Task {
        id: "t1".to_string(),
        title: "Task 1".to_string(),
        status: "pending".to_string(),
    }]);

    let ctx = make_ctx(store);
    let store = ctx.app_state.get_required::<InMemoryTaskStore>().unwrap();

    let cmd = TaskCommands::augment_subcommands(Command::new("app"));
    let matches = cmd
        .try_get_matches_from(vec!["app", "complete", "t1", "missing"])
        .unwrap();
    let sub = matches.subcommand_matches("complete").unwrap();

    // Batch operations with partial failures still succeed (reporting errors)
    let result = __task_resource_handlers::shortcut_complete(sub, &ctx);
    assert!(
        result.is_ok(),
        "Batch should succeed even with partial failures"
    );

    // t1 should still be updated
    let task = store.get(&"t1".to_string()).unwrap().unwrap();
    assert_eq!(task.status, "done");
}

// ============================================================================
// Shortcut with Aliases
// ============================================================================

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Resource, Tabular)]
#[resource(
    object = "item",
    store = InMemoryItemStore,
    aliases(delete = "rm"),
    shortcut(name = "archive", sets(status = "archived"))
)]
struct Item {
    #[resource(id)]
    #[tabular(name = "ID")]
    pub id: String,

    #[resource(arg(long))]
    #[tabular(name = "NAME")]
    pub name: String,

    #[resource(arg(long), default = "active")]
    #[tabular(name = "STATUS")]
    pub status: String,
}

struct InMemoryItemStore {
    items: RwLock<HashMap<String, Item>>,
}

impl InMemoryItemStore {
    fn new() -> Self {
        Self {
            items: RwLock::new(HashMap::new()),
        }
    }

    fn seed(&self, items: Vec<Item>) {
        let mut store = self.items.write().unwrap();
        for item in items {
            store.insert(item.id.clone(), item);
        }
    }
}

impl ResourceStore for InMemoryItemStore {
    type Item = Item;
    type Id = String;
    type Error = TestError;

    fn parse_id(&self, id_str: &str) -> Result<Self::Id, Self::Error> {
        Ok(id_str.to_string())
    }

    fn get(&self, id: &Self::Id) -> Result<Option<Self::Item>, Self::Error> {
        Ok(self.items.read().unwrap().get(id).cloned())
    }

    fn not_found_error(id: &Self::Id) -> Self::Error {
        TestError(format!("Item '{}' not found", id))
    }

    fn list(&self, _query: Option<&ResourceQuery>) -> Result<Vec<Self::Item>, Self::Error> {
        Ok(self.items.read().unwrap().values().cloned().collect())
    }

    fn create(&self, data: serde_json::Value) -> Result<Self::Item, Self::Error> {
        let item: Item = serde_json::from_value(data).map_err(|e| TestError(e.to_string()))?;
        self.items
            .write()
            .unwrap()
            .insert(item.id.clone(), item.clone());
        Ok(item)
    }

    fn update(&self, id: &Self::Id, data: serde_json::Value) -> Result<Self::Item, Self::Error> {
        let mut items = self.items.write().unwrap();
        let item = items.get_mut(id).ok_or_else(|| Self::not_found_error(id))?;

        if let Some(name) = data.get("name").and_then(|v| v.as_str()) {
            item.name = name.to_string();
        }
        if let Some(status) = data.get("status").and_then(|v| v.as_str()) {
            item.status = status.to_string();
        }

        Ok(item.clone())
    }

    fn delete(&self, id: &Self::Id) -> Result<(), Self::Error> {
        let mut items = self.items.write().unwrap();
        items.remove(id).ok_or_else(|| Self::not_found_error(id))?;
        Ok(())
    }
}

#[test]
fn test_shortcut_coexists_with_aliases() {
    let names = ItemCommands::subcommand_names();
    // CRUD with alias: delete -> rm
    assert!(names.contains(&"rm"), "Should have 'rm' alias for delete");
    // Shortcut
    assert!(names.contains(&"archive"), "Should have 'archive' shortcut");
    // Standard
    assert!(names.contains(&"list"));
    assert!(names.contains(&"view"));
    assert!(names.contains(&"create"));
    assert!(names.contains(&"update"));
}

#[test]
fn test_shortcut_archive_with_aliased_resource() {
    let store = InMemoryItemStore::new();
    store.seed(vec![Item {
        id: "i1".to_string(),
        name: "My Item".to_string(),
        status: "active".to_string(),
    }]);

    let mut app_state = Extensions::new();
    app_state.insert(store);
    let ctx = CommandContext::new(vec!["app".to_string()], Arc::new(app_state));
    let store = ctx.app_state.get_required::<InMemoryItemStore>().unwrap();

    let cmd = ItemCommands::augment_subcommands(Command::new("app"));
    let matches = cmd
        .try_get_matches_from(vec!["app", "archive", "i1"])
        .unwrap();
    let sub = matches.subcommand_matches("archive").unwrap();

    let result = __item_resource_handlers::shortcut_archive(sub, &ctx);
    assert!(result.is_ok());

    let item = store.get(&"i1".to_string()).unwrap().unwrap();
    assert_eq!(item.status, "archived");
}

// ============================================================================
// Multiple-field Shortcut
// ============================================================================

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Resource, Tabular)]
#[resource(
    object = "ticket",
    store = InMemoryTicketStore,
    shortcut(name = "close", sets(status = "closed", resolution = "fixed"))
)]
struct Ticket {
    #[resource(id)]
    #[tabular(name = "ID")]
    pub id: String,

    #[resource(arg(long))]
    #[tabular(name = "TITLE")]
    pub title: String,

    #[resource(arg(long), default = "open")]
    #[tabular(name = "STATUS")]
    pub status: String,

    #[resource(arg(long), default = "none")]
    #[tabular(name = "RESOLUTION")]
    pub resolution: String,
}

struct InMemoryTicketStore {
    tickets: RwLock<HashMap<String, Ticket>>,
}

impl InMemoryTicketStore {
    fn new() -> Self {
        Self {
            tickets: RwLock::new(HashMap::new()),
        }
    }

    fn seed(&self, tickets: Vec<Ticket>) {
        let mut store = self.tickets.write().unwrap();
        for ticket in tickets {
            store.insert(ticket.id.clone(), ticket);
        }
    }
}

impl ResourceStore for InMemoryTicketStore {
    type Item = Ticket;
    type Id = String;
    type Error = TestError;

    fn parse_id(&self, id_str: &str) -> Result<Self::Id, Self::Error> {
        Ok(id_str.to_string())
    }

    fn get(&self, id: &Self::Id) -> Result<Option<Self::Item>, Self::Error> {
        Ok(self.tickets.read().unwrap().get(id).cloned())
    }

    fn not_found_error(id: &Self::Id) -> Self::Error {
        TestError(format!("Ticket '{}' not found", id))
    }

    fn list(&self, _query: Option<&ResourceQuery>) -> Result<Vec<Self::Item>, Self::Error> {
        Ok(self.tickets.read().unwrap().values().cloned().collect())
    }

    fn create(&self, data: serde_json::Value) -> Result<Self::Item, Self::Error> {
        let ticket: Ticket = serde_json::from_value(data).map_err(|e| TestError(e.to_string()))?;
        self.tickets
            .write()
            .unwrap()
            .insert(ticket.id.clone(), ticket.clone());
        Ok(ticket)
    }

    fn update(&self, id: &Self::Id, data: serde_json::Value) -> Result<Self::Item, Self::Error> {
        let mut tickets = self.tickets.write().unwrap();
        let ticket = tickets
            .get_mut(id)
            .ok_or_else(|| Self::not_found_error(id))?;

        if let Some(title) = data.get("title").and_then(|v| v.as_str()) {
            ticket.title = title.to_string();
        }
        if let Some(status) = data.get("status").and_then(|v| v.as_str()) {
            ticket.status = status.to_string();
        }
        if let Some(resolution) = data.get("resolution").and_then(|v| v.as_str()) {
            ticket.resolution = resolution.to_string();
        }

        Ok(ticket.clone())
    }

    fn delete(&self, id: &Self::Id) -> Result<(), Self::Error> {
        let mut tickets = self.tickets.write().unwrap();
        tickets
            .remove(id)
            .ok_or_else(|| Self::not_found_error(id))?;
        Ok(())
    }
}

#[test]
fn test_shortcut_sets_multiple_fields() {
    let store = InMemoryTicketStore::new();
    store.seed(vec![Ticket {
        id: "tk1".to_string(),
        title: "Bug Report".to_string(),
        status: "open".to_string(),
        resolution: "none".to_string(),
    }]);

    let mut app_state = Extensions::new();
    app_state.insert(store);
    let ctx = CommandContext::new(vec!["app".to_string()], Arc::new(app_state));
    let store = ctx.app_state.get_required::<InMemoryTicketStore>().unwrap();

    let cmd = TicketCommands::augment_subcommands(Command::new("app"));
    let matches = cmd
        .try_get_matches_from(vec!["app", "close", "tk1"])
        .unwrap();
    let sub = matches.subcommand_matches("close").unwrap();

    let result = __ticket_resource_handlers::shortcut_close(sub, &ctx);
    assert!(result.is_ok());

    let ticket = store.get(&"tk1".to_string()).unwrap().unwrap();
    assert_eq!(ticket.status, "closed", "Status should be 'closed'");
    assert_eq!(ticket.resolution, "fixed", "Resolution should be 'fixed'");
    assert_eq!(ticket.title, "Bug Report", "Title should be unchanged");
}

#[test]
fn test_shortcut_multi_field_output_tracks_all_changes() {
    let store = InMemoryTicketStore::new();
    store.seed(vec![Ticket {
        id: "tk1".to_string(),
        title: "Bug Report".to_string(),
        status: "open".to_string(),
        resolution: "none".to_string(),
    }]);

    let mut app_state = Extensions::new();
    app_state.insert(store);
    let ctx = CommandContext::new(vec!["app".to_string()], Arc::new(app_state));

    let cmd = TicketCommands::augment_subcommands(Command::new("app"));
    let matches = cmd
        .try_get_matches_from(vec!["app", "close", "tk1"])
        .unwrap();
    let sub = matches.subcommand_matches("close").unwrap();

    let result = __ticket_resource_handlers::shortcut_close(sub, &ctx).unwrap();
    if let standout::cli::Output::Render(val) = result {
        let json_str = serde_json::to_string(&val).unwrap();
        // Both changed fields should be tracked
        assert!(json_str.contains("status"), "Should track status change");
        assert!(
            json_str.contains("resolution"),
            "Should track resolution change"
        );
    } else {
        panic!("Expected Render output");
    }
}
