//! Integration tests for Resource with Validify support.
//!
//! These tests verify that the validify feature correctly integrates
//! validation and modification with the Resource pipeline.

#![cfg(feature = "validify")]

use serde::{Deserialize, Serialize};
use standout::cli::{ResourceQuery, ResourceStore, ValidationError};
use standout::Validify;
use std::collections::HashMap;
use std::sync::RwLock;

// ============================================================================
// Test fixtures
// ============================================================================

/// A task struct with validify modifiers and validators
#[derive(Debug, Clone, Serialize, Deserialize, Validify, PartialEq)]
struct ValidatedTask {
    id: String,
    #[modify(trim)]
    #[validate(length(min = 1, max = 100))]
    title: String,
    #[modify(trim, lowercase)]
    status: String,
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
    tasks: RwLock<HashMap<String, ValidatedTask>>,
}

impl InMemoryTaskStore {
    fn new() -> Self {
        Self {
            tasks: RwLock::new(HashMap::new()),
        }
    }
}

impl ResourceStore for InMemoryTaskStore {
    type Item = ValidatedTask;
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
        let task: ValidatedTask =
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

        Ok(task.clone())
    }

    fn delete(&self, id: &Self::Id) -> Result<(), Self::Error> {
        let mut tasks = self.tasks.write().unwrap();
        tasks.remove(id).ok_or_else(|| Self::not_found_error(id))?;
        Ok(())
    }
}

// ============================================================================
// Validify trait tests
// ============================================================================

#[test]
fn test_validify_modifiers_trim() {
    let mut task = ValidatedTask {
        id: "1".to_string(),
        title: "  hello world  ".to_string(),
        status: "  PENDING  ".to_string(),
    };

    task.validify().unwrap();

    // Title should be trimmed
    assert_eq!(task.title, "hello world");
    // Status should be trimmed AND lowercased
    assert_eq!(task.status, "pending");
}

#[test]
fn test_validify_validation_length() {
    let mut task = ValidatedTask {
        id: "1".to_string(),
        title: "".to_string(), // Empty title should fail length(min = 1)
        status: "pending".to_string(),
    };

    let result = task.validify();
    assert!(result.is_err());
}

#[test]
fn test_validify_validation_max_length() {
    let long_title = "x".repeat(101); // Exceeds max = 100
    let mut task = ValidatedTask {
        id: "1".to_string(),
        title: long_title,
        status: "pending".to_string(),
    };

    let result = task.validify();
    assert!(result.is_err());
}

#[test]
fn test_validify_modifiers_before_validation() {
    // This tests that modifiers run BEFORE validation
    // A title that is just spaces should be trimmed to empty, then fail validation
    let mut task = ValidatedTask {
        id: "1".to_string(),
        title: "   ".to_string(), // Just spaces
        status: "pending".to_string(),
    };

    let result = task.validify();
    // After trim, title is "", which fails length(min = 1)
    assert!(result.is_err());
}

// ============================================================================
// Pipeline error type tests
// ============================================================================

#[test]
fn test_validation_error_field() {
    let err = ValidationError::field("title", "must not be empty");
    assert_eq!(err.field, Some("title".to_string()));
    assert_eq!(err.message, "must not be empty");
}

#[test]
fn test_validation_error_general() {
    let err = ValidationError::general("validation failed");
    assert_eq!(err.field, None);
    assert_eq!(err.message, "validation failed");
}

// ============================================================================
// Integration tests with ResourceStore
// ============================================================================

#[test]
fn test_store_with_validified_data() {
    let store = InMemoryTaskStore::new();

    // Create a task - data comes in "raw" (like from CLI)
    let mut task = ValidatedTask {
        id: "t1".to_string(),
        title: "  My Task  ".to_string(),
        status: "  PENDING  ".to_string(),
    };

    // Apply validify (as the macro would do)
    task.validify().unwrap();

    // Now serialize to JSON for the store
    let data = serde_json::to_value(&task).unwrap();
    let created = store.create(data).unwrap();

    // Verify modifiers were applied
    assert_eq!(created.title, "My Task");
    assert_eq!(created.status, "pending");
}

#[test]
fn test_store_update_with_validation() {
    let store = InMemoryTaskStore::new();

    // Create initial task
    let initial = serde_json::json!({
        "id": "t1",
        "title": "Original",
        "status": "pending"
    });
    store.create(initial).unwrap();

    // Update with new data
    let update_data = serde_json::json!({
        "title": "Updated Title"
    });
    let updated = store.update(&"t1".to_string(), update_data).unwrap();

    assert_eq!(updated.title, "Updated Title");
    assert_eq!(updated.status, "pending"); // Unchanged
}
