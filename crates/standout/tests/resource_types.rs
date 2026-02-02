//! Integration tests for Resource with Vec<T> and enum type support.
//!
//! These tests verify that the Resource macro correctly handles:
//! - Vec<String> for multi-value arguments
//! - Option<T> for optional arguments
//! - Enum types with #[resource(value_enum)]

use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use standout::cli::{ResourceQuery, ResourceStore};
use std::collections::HashMap;
use std::sync::RwLock;

// ============================================================================
// Test fixtures
// ============================================================================

/// A task status enum that implements ValueEnum for clap
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum TaskStatus {
    Pending,
    InProgress,
    Done,
}

/// A priority enum
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Priority {
    Low,
    Medium,
    High,
}

/// A task struct with various field types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct TypedTask {
    id: String,
    title: String,
    tags: Vec<String>,
    status: TaskStatus,
    priority: Option<Priority>,
    assignees: Vec<String>,
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
    tasks: RwLock<HashMap<String, TypedTask>>,
}

impl InMemoryTaskStore {
    fn new() -> Self {
        Self {
            tasks: RwLock::new(HashMap::new()),
        }
    }

    fn with_tasks(tasks: Vec<TypedTask>) -> Self {
        let store = Self::new();
        for task in tasks {
            store.tasks.write().unwrap().insert(task.id.clone(), task);
        }
        store
    }
}

impl ResourceStore for InMemoryTaskStore {
    type Item = TypedTask;
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
        let task: TypedTask = serde_json::from_value(data).map_err(|e| TestError(e.to_string()))?;
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
        if let Some(tags) = data.get("tags").and_then(|v| v.as_array()) {
            task.tags = tags
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect();
        }
        if let Some(status) = data.get("status").and_then(|v| v.as_str()) {
            task.status = match status {
                "pending" => TaskStatus::Pending,
                "in_progress" => TaskStatus::InProgress,
                "done" => TaskStatus::Done,
                _ => task.status,
            };
        }
        if let Some(priority) = data.get("priority").and_then(|v| v.as_str()) {
            task.priority = match priority {
                "low" => Some(Priority::Low),
                "medium" => Some(Priority::Medium),
                "high" => Some(Priority::High),
                _ => task.priority,
            };
        }
        if let Some(assignees) = data.get("assignees").and_then(|v| v.as_array()) {
            task.assignees = assignees
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect();
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
// Type parsing tests (TypeKind)
// ============================================================================

#[test]
fn test_vec_type_serialization() {
    let task = TypedTask {
        id: "1".to_string(),
        title: "Test".to_string(),
        tags: vec!["rust".to_string(), "cli".to_string()],
        status: TaskStatus::Pending,
        priority: Some(Priority::High),
        assignees: vec!["alice".to_string(), "bob".to_string()],
    };

    let json = serde_json::to_value(&task).unwrap();

    // Check that Vec fields serialize correctly
    assert!(json["tags"].is_array());
    assert_eq!(json["tags"].as_array().unwrap().len(), 2);
    assert_eq!(json["tags"][0], "rust");
    assert_eq!(json["tags"][1], "cli");

    assert!(json["assignees"].is_array());
    assert_eq!(json["assignees"].as_array().unwrap().len(), 2);
}

#[test]
fn test_enum_type_serialization() {
    let task = TypedTask {
        id: "1".to_string(),
        title: "Test".to_string(),
        tags: vec![],
        status: TaskStatus::InProgress,
        priority: None,
        assignees: vec![],
    };

    let json = serde_json::to_value(&task).unwrap();

    // Check that enum serializes to lowercase string
    assert_eq!(json["status"], "inprogress");
    assert!(json["priority"].is_null());
}

#[test]
fn test_option_type_serialization() {
    // With Some value
    let task1 = TypedTask {
        id: "1".to_string(),
        title: "Test".to_string(),
        tags: vec![],
        status: TaskStatus::Pending,
        priority: Some(Priority::Medium),
        assignees: vec![],
    };

    let json1 = serde_json::to_value(&task1).unwrap();
    assert_eq!(json1["priority"], "medium");

    // With None value
    let task2 = TypedTask {
        id: "2".to_string(),
        title: "Test".to_string(),
        tags: vec![],
        status: TaskStatus::Pending,
        priority: None,
        assignees: vec![],
    };

    let json2 = serde_json::to_value(&task2).unwrap();
    assert!(json2["priority"].is_null());
}

// ============================================================================
// Store integration tests
// ============================================================================

#[test]
fn test_store_create_with_vec() {
    let store = InMemoryTaskStore::new();

    let data = serde_json::json!({
        "id": "t1",
        "title": "Task with tags",
        "tags": ["rust", "cli", "testing"],
        "status": "pending",
        "priority": null,
        "assignees": []
    });

    let task = store.create(data).unwrap();
    assert_eq!(task.tags.len(), 3);
    assert!(task.tags.contains(&"rust".to_string()));
    assert!(task.tags.contains(&"cli".to_string()));
    assert!(task.tags.contains(&"testing".to_string()));
}

#[test]
fn test_store_update_vec() {
    let store = InMemoryTaskStore::with_tasks(vec![TypedTask {
        id: "t1".to_string(),
        title: "Original".to_string(),
        tags: vec!["old".to_string()],
        status: TaskStatus::Pending,
        priority: None,
        assignees: vec![],
    }]);

    let update_data = serde_json::json!({
        "tags": ["new1", "new2"]
    });

    let updated = store.update(&"t1".to_string(), update_data).unwrap();
    assert_eq!(updated.tags.len(), 2);
    assert!(updated.tags.contains(&"new1".to_string()));
    assert!(updated.tags.contains(&"new2".to_string()));
}

#[test]
fn test_store_update_enum() {
    let store = InMemoryTaskStore::with_tasks(vec![TypedTask {
        id: "t1".to_string(),
        title: "Task".to_string(),
        tags: vec![],
        status: TaskStatus::Pending,
        priority: None,
        assignees: vec![],
    }]);

    let update_data = serde_json::json!({
        "status": "done",
        "priority": "high"
    });

    let updated = store.update(&"t1".to_string(), update_data).unwrap();
    assert_eq!(updated.status, TaskStatus::Done);
    assert_eq!(updated.priority, Some(Priority::High));
}

#[test]
fn test_store_list_with_typed_tasks() {
    let store = InMemoryTaskStore::with_tasks(vec![
        TypedTask {
            id: "t1".to_string(),
            title: "First".to_string(),
            tags: vec!["a".to_string()],
            status: TaskStatus::Pending,
            priority: Some(Priority::Low),
            assignees: vec!["alice".to_string()],
        },
        TypedTask {
            id: "t2".to_string(),
            title: "Second".to_string(),
            tags: vec!["b".to_string(), "c".to_string()],
            status: TaskStatus::Done,
            priority: Some(Priority::High),
            assignees: vec!["bob".to_string(), "charlie".to_string()],
        },
    ]);

    let tasks = store.list(None).unwrap();
    assert_eq!(tasks.len(), 2);

    // Verify types are preserved
    let first = &tasks[0];
    assert_eq!(first.tags.len(), 1);
    assert_eq!(first.status, TaskStatus::Pending);
    assert_eq!(first.priority, Some(Priority::Low));

    let second = &tasks[1];
    assert_eq!(second.tags.len(), 2);
    assert_eq!(second.status, TaskStatus::Done);
    assert_eq!(second.priority, Some(Priority::High));
}

// ============================================================================
// ValueEnum tests
// ============================================================================

#[test]
fn test_value_enum_variants() {
    // Test that ValueEnum derive works correctly
    assert_eq!(
        TaskStatus::value_variants(),
        &[
            TaskStatus::Pending,
            TaskStatus::InProgress,
            TaskStatus::Done
        ]
    );

    assert_eq!(
        Priority::value_variants(),
        &[Priority::Low, Priority::Medium, Priority::High]
    );
}

#[test]
fn test_value_enum_to_possible_value() {
    // Test that variants convert to clap's PossibleValue
    let status = TaskStatus::Pending;
    let possible = status.to_possible_value().unwrap();
    assert_eq!(possible.get_name(), "pending");

    let priority = Priority::High;
    let possible = priority.to_possible_value().unwrap();
    assert_eq!(possible.get_name(), "high");
}
