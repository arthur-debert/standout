//! Traits for derive macro integration.
//!
//! These traits are implemented by the `#[derive(Tabular)]` and `#[derive(TabularRow)]`
//! macros to enable type-safe tabular formatting.

use super::TabularSpec;

/// Trait for types that can generate a `TabularSpec`.
///
/// This trait is automatically implemented by `#[derive(Tabular)]`.
///
/// # Example
///
/// ```ignore
/// use outstanding::tabular::{Tabular, TabularSpec};
/// use serde::Serialize;
///
/// #[derive(Serialize, Tabular)]
/// struct Task {
///     #[col(width = 8, style = "muted")]
///     id: String,
///
///     #[col(width = "fill")]
///     title: String,
///
///     #[col(width = 12, align = "right")]
///     status: String,
/// }
///
/// // Use the generated spec
/// let spec = Task::tabular_spec();
/// ```
pub trait Tabular {
    /// Returns the `TabularSpec` for this type.
    fn tabular_spec() -> TabularSpec;
}

/// Trait for types that can be converted to a row of strings.
///
/// This trait is automatically implemented by `#[derive(TabularRow)]`.
/// It provides optimized row extraction without JSON serialization.
///
/// # Example
///
/// ```ignore
/// use outstanding::tabular::TabularRow;
///
/// #[derive(TabularRow)]
/// struct Task {
///     id: String,
///     title: String,
///     status: String,
/// }
///
/// let task = Task {
///     id: "TSK-001".to_string(),
///     title: "Implement feature".to_string(),
///     status: "pending".to_string(),
/// };
///
/// let row: Vec<String> = task.to_row();
/// assert_eq!(row, vec!["TSK-001", "Implement feature", "pending"]);
/// ```
pub trait TabularRow {
    /// Converts this instance to a row of string values.
    fn to_row(&self) -> Vec<String>;
}
