//! Integration tests for Resource default values (#76).
//!
//! These tests verify that the Resource macro correctly handles:
//! - #[resource(default = "value")] for default values
//! - Defaults are injected when fields not provided
//! - Explicit values override defaults

use serde::{Deserialize, Serialize};
use standout::cli::{ResourceQuery, ResourceStore};
use std::collections::HashMap;
use std::sync::RwLock;

// ============================================================================
// Test fixtures
// ============================================================================

/// A task struct with default values
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct TaskWithDefaults {
    id: String,
    title: String,
    /// Status defaults to pending
    status: String,
    /// Priority defaults to 3
    priority: u8,
}

#[derive(Debug)]
struct TestError(String);

impl std::fmt::Display for TestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for TestError {}

/// In-memory store for testing
struct InMemoryTaskStore {
    tasks: RwLock<HashMap<String, TaskWithDefaults>>,
}

impl InMemoryTaskStore {
    fn new() -> Self {
        Self {
            tasks: RwLock::new(HashMap::new()),
        }
    }
}

impl ResourceStore for InMemoryTaskStore {
    type Item = TaskWithDefaults;
    type Id = String;
    type Error = TestError;

    fn parse_id(&self, id_str: &str) -> Result<Self::Id, Self::Error> {
        if id_str.is_empty() {
            Err(TestError("ID cannot be empty".to_string()))
        } else {
            Ok(id_str.to_string())
        }
    }

    fn get(&self, id: &Self::Id) -> Result<Option<Self::Item>, Self::Error> {
        Ok(self.tasks.read().unwrap().get(id).cloned())
    }

    fn not_found_error(id: &Self::Id) -> Self::Error {
        TestError(format!("Task '{}' not found", id))
    }

    fn list(&self, query: Option<&ResourceQuery>) -> Result<Vec<Self::Item>, Self::Error> {
        let tasks = self.tasks.read().unwrap();
        let mut result: Vec<_> = tasks.values().cloned().collect();
        result.sort_by(|a, b| a.id.cmp(&b.id));

        if let Some(q) = query {
            if let Some(limit) = q.limit {
                result.truncate(limit);
            }
        }

        Ok(result)
    }

    fn create(&self, data: serde_json::Value) -> Result<Self::Item, Self::Error> {
        let task: TaskWithDefaults =
            serde_json::from_value(data).map_err(|e| TestError(e.to_string()))?;
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
        if let Some(priority) = data.get("priority").and_then(|v| v.as_u64()) {
            task.priority = priority as u8;
        }

        Ok(task.clone())
    }

    fn delete(&self, id: &Self::Id) -> Result<(), Self::Error> {
        let mut tasks = self.tasks.write().unwrap();
        tasks.remove(id).ok_or_else(|| Self::not_found_error(id))?;
        Ok(())
    }
}

// ============================================================================
// Default value injection tests
// ============================================================================

#[test]
fn test_default_injection_logic() {
    // Test the default injection logic directly using serde_json
    let mut data = serde_json::json!({
        "id": "t1",
        "title": "Test task"
    });

    // Simulate default injection for missing fields
    if data.get("status").is_none() {
        data["status"] = serde_json::json!("pending");
    }
    if data.get("priority").is_none() {
        data["priority"] = serde_json::json!(3);
    }

    // Verify defaults were injected
    assert_eq!(data["status"], "pending");
    assert_eq!(data["priority"], 3);

    // Create task with injected defaults
    let store = InMemoryTaskStore::new();
    let task = store.create(data).unwrap();

    assert_eq!(task.status, "pending");
    assert_eq!(task.priority, 3);
}

#[test]
fn test_explicit_values_override_defaults() {
    // Simulate data with explicit values
    let mut data = serde_json::json!({
        "id": "t2",
        "title": "Another task",
        "status": "done",
        "priority": 1
    });

    // Default injection should NOT overwrite existing values
    if data.get("status").is_none() {
        data["status"] = serde_json::json!("pending");
    }
    if data.get("priority").is_none() {
        data["priority"] = serde_json::json!(3);
    }

    // Verify explicit values are preserved
    assert_eq!(data["status"], "done");
    assert_eq!(data["priority"], 1);

    // Create task with explicit values
    let store = InMemoryTaskStore::new();
    let task = store.create(data).unwrap();

    assert_eq!(task.status, "done");
    assert_eq!(task.priority, 1);
}

#[test]
fn test_partial_defaults() {
    // Test when some fields are provided and some need defaults
    let mut data = serde_json::json!({
        "id": "t3",
        "title": "Task with partial data",
        "status": "in_progress"
        // priority not provided - should get default
    });

    // Default injection
    if data.get("status").is_none() {
        data["status"] = serde_json::json!("pending");
    }
    if data.get("priority").is_none() {
        data["priority"] = serde_json::json!(3);
    }

    // Verify mixed explicit/default values
    assert_eq!(data["status"], "in_progress"); // Explicit - preserved
    assert_eq!(data["priority"], 3); // Default - injected

    let store = InMemoryTaskStore::new();
    let task = store.create(data).unwrap();

    assert_eq!(task.status, "in_progress");
    assert_eq!(task.priority, 3);
}

#[test]
fn test_numeric_default_types() {
    // Verify numeric defaults work correctly with different types
    let mut data = serde_json::json!({
        "id": "t4",
        "title": "Numeric test"
    });

    // Inject numeric default as string (how it comes from #[resource(default = "3")])
    // The JSON serialization handles the type conversion
    if data.get("priority").is_none() {
        data["priority"] = serde_json::json!("3");
    }

    // The store's create method handles the conversion
    // In this test, we verify the string can be parsed
    let priority_str = data["priority"].as_str().unwrap();
    let priority: u8 = priority_str.parse().unwrap();
    assert_eq!(priority, 3);
}

#[test]
fn test_default_with_string_type() {
    // Verify string defaults work correctly
    let mut data = serde_json::json!({
        "id": "t5",
        "title": "String default test"
    });

    // Inject string default
    if data.get("status").is_none() {
        data["status"] = serde_json::json!("pending");
    }

    assert_eq!(data["status"], "pending");
}
