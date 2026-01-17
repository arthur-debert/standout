//! Integration tests for the Tabular derive macro.
//!
//! These tests verify that the `#[derive(Tabular)]` macro generates correct
//! `TabularSpec` configurations from struct field annotations.

#![cfg(feature = "macros")]

use outstanding::tabular::{Align, Anchor, Overflow, Tabular, TruncateAt, Width};
use outstanding_macros::Tabular as DeriveTabular;
use serde::Serialize;

// =============================================================================
// Basic derive tests
// =============================================================================

#[derive(Serialize, DeriveTabular)]
struct BasicTask {
    id: String,
    title: String,
    status: String,
}

#[test]
fn test_basic_derive_compiles() {
    let spec = BasicTask::tabular_spec();
    assert_eq!(spec.columns.len(), 3);
}

#[test]
fn test_basic_derive_field_names() {
    let spec = BasicTask::tabular_spec();
    assert_eq!(spec.columns[0].name.as_deref(), Some("id"));
    assert_eq!(spec.columns[1].name.as_deref(), Some("title"));
    assert_eq!(spec.columns[2].name.as_deref(), Some("status"));
}

#[test]
fn test_basic_derive_default_keys() {
    let spec = BasicTask::tabular_spec();
    // Keys default to field names
    assert_eq!(spec.columns[0].key.as_deref(), Some("id"));
    assert_eq!(spec.columns[1].key.as_deref(), Some("title"));
    assert_eq!(spec.columns[2].key.as_deref(), Some("status"));
}

#[test]
fn test_basic_derive_default_headers() {
    let spec = BasicTask::tabular_spec();
    // Headers default to field names
    assert_eq!(spec.columns[0].header.as_deref(), Some("id"));
    assert_eq!(spec.columns[1].header.as_deref(), Some("title"));
    assert_eq!(spec.columns[2].header.as_deref(), Some("status"));
}

// =============================================================================
// Width attribute tests
// =============================================================================

#[derive(Serialize, DeriveTabular)]
struct WidthTask {
    #[col(width = 8)]
    id: String,

    #[col(width = "fill")]
    title: String,

    #[col(width = "2fr")]
    description: String,

    #[col(min = 10, max = 30)]
    status: String,
}

#[test]
fn test_width_fixed() {
    let spec = WidthTask::tabular_spec();
    assert_eq!(spec.columns[0].width, Width::Fixed(8));
}

#[test]
fn test_width_fill() {
    let spec = WidthTask::tabular_spec();
    assert_eq!(spec.columns[1].width, Width::Fill);
}

#[test]
fn test_width_fraction() {
    let spec = WidthTask::tabular_spec();
    assert_eq!(spec.columns[2].width, Width::Fraction(2));
}

#[test]
fn test_width_bounded() {
    let spec = WidthTask::tabular_spec();
    assert_eq!(
        spec.columns[3].width,
        Width::Bounded {
            min: Some(10),
            max: Some(30)
        }
    );
}

// =============================================================================
// Alignment and anchor tests
// =============================================================================

#[derive(Serialize, DeriveTabular)]
struct AlignTask {
    #[col(align = "left")]
    left_aligned: String,

    #[col(align = "right")]
    right_aligned: String,

    #[col(align = "center")]
    center_aligned: String,

    #[col(anchor = "right")]
    right_anchored: String,
}

#[test]
fn test_align_left() {
    let spec = AlignTask::tabular_spec();
    assert_eq!(spec.columns[0].align, Align::Left);
}

#[test]
fn test_align_right() {
    let spec = AlignTask::tabular_spec();
    assert_eq!(spec.columns[1].align, Align::Right);
}

#[test]
fn test_align_center() {
    let spec = AlignTask::tabular_spec();
    assert_eq!(spec.columns[2].align, Align::Center);
}

#[test]
fn test_anchor_right() {
    let spec = AlignTask::tabular_spec();
    assert_eq!(spec.columns[3].anchor, Anchor::Right);
}

// =============================================================================
// Overflow tests
// =============================================================================

#[derive(Serialize, DeriveTabular)]
struct OverflowTask {
    #[col(overflow = "wrap")]
    wrapped: String,

    #[col(overflow = "clip")]
    clipped: String,

    #[col(overflow = "expand")]
    expanded: String,

    #[col(overflow = "truncate", truncate_at = "middle")]
    truncated_middle: String,

    #[col(truncate_at = "start")]
    truncated_start: String,
}

#[test]
fn test_overflow_wrap() {
    let spec = OverflowTask::tabular_spec();
    assert_eq!(spec.columns[0].overflow, Overflow::Wrap { indent: 0 });
}

#[test]
fn test_overflow_clip() {
    let spec = OverflowTask::tabular_spec();
    assert_eq!(spec.columns[1].overflow, Overflow::Clip);
}

#[test]
fn test_overflow_expand() {
    let spec = OverflowTask::tabular_spec();
    assert_eq!(spec.columns[2].overflow, Overflow::Expand);
}

#[test]
fn test_overflow_truncate_middle() {
    let spec = OverflowTask::tabular_spec();
    assert_eq!(
        spec.columns[3].overflow,
        Overflow::Truncate {
            at: TruncateAt::Middle,
            marker: "…".to_string()
        }
    );
}

#[test]
fn test_overflow_truncate_start() {
    let spec = OverflowTask::tabular_spec();
    assert_eq!(
        spec.columns[4].overflow,
        Overflow::Truncate {
            at: TruncateAt::Start,
            marker: "…".to_string()
        }
    );
}

// =============================================================================
// Style tests
// =============================================================================

#[derive(Serialize, DeriveTabular)]
struct StyleTask {
    #[col(style = "muted")]
    styled: String,

    #[col(style_from_value)]
    dynamic_style: String,
}

#[test]
fn test_style() {
    let spec = StyleTask::tabular_spec();
    assert_eq!(spec.columns[0].style.as_deref(), Some("muted"));
    assert!(!spec.columns[0].style_from_value);
}

#[test]
fn test_style_from_value() {
    let spec = StyleTask::tabular_spec();
    assert!(spec.columns[1].style_from_value);
}

// =============================================================================
// Header and null_repr tests
// =============================================================================

#[derive(Serialize, DeriveTabular)]
struct HeaderTask {
    #[col(header = "Task ID")]
    id: String,

    #[col(null_repr = "N/A")]
    optional_field: String,

    #[col(key = "nested.value")]
    custom_key: String,
}

#[test]
fn test_custom_header() {
    let spec = HeaderTask::tabular_spec();
    assert_eq!(spec.columns[0].header.as_deref(), Some("Task ID"));
}

#[test]
fn test_null_repr() {
    let spec = HeaderTask::tabular_spec();
    assert_eq!(spec.columns[1].null_repr, "N/A");
}

#[test]
fn test_custom_key() {
    let spec = HeaderTask::tabular_spec();
    assert_eq!(spec.columns[2].key.as_deref(), Some("nested.value"));
}

// =============================================================================
// Skip attribute tests
// =============================================================================

#[derive(Serialize, DeriveTabular)]
struct SkipTask {
    id: String,

    #[col(skip)]
    internal_state: u32,

    title: String,
}

#[test]
fn test_skip_field() {
    let spec = SkipTask::tabular_spec();
    // Should only have 2 columns (id and title), not 3
    assert_eq!(spec.columns.len(), 2);
    assert_eq!(spec.columns[0].name.as_deref(), Some("id"));
    assert_eq!(spec.columns[1].name.as_deref(), Some("title"));
}

// =============================================================================
// Container attribute tests
// =============================================================================

#[derive(Serialize, DeriveTabular)]
#[tabular(separator = " │ ")]
struct SeparatorTask {
    id: String,
    title: String,
}

#[test]
fn test_custom_separator() {
    let spec = SeparatorTask::tabular_spec();
    assert_eq!(spec.decorations.column_sep, " │ ");
}

#[derive(Serialize, DeriveTabular)]
#[tabular(prefix = "│ ", suffix = " │")]
struct PrefixSuffixTask {
    id: String,
}

#[test]
fn test_prefix_suffix() {
    let spec = PrefixSuffixTask::tabular_spec();
    assert_eq!(spec.decorations.row_prefix, "│ ");
    assert_eq!(spec.decorations.row_suffix, " │");
}

// =============================================================================
// Combined attributes test
// =============================================================================

#[derive(Serialize, DeriveTabular)]
#[tabular(separator = " │ ")]
struct CompleteTask {
    #[col(width = 8, style = "muted", header = "ID")]
    id: String,

    #[col(width = "fill", overflow = "wrap")]
    title: String,

    #[col(width = 12, align = "right", style_from_value)]
    status: String,

    #[col(skip)]
    internal: String,

    #[col(width = 10, anchor = "right", truncate_at = "middle")]
    due: String,
}

#[test]
fn test_complete_task_columns() {
    let spec = CompleteTask::tabular_spec();
    // Should have 4 columns (internal is skipped)
    assert_eq!(spec.columns.len(), 4);
}

#[test]
fn test_complete_task_id_column() {
    let spec = CompleteTask::tabular_spec();
    let col = &spec.columns[0];
    assert_eq!(col.name.as_deref(), Some("id"));
    assert_eq!(col.width, Width::Fixed(8));
    assert_eq!(col.style.as_deref(), Some("muted"));
    assert_eq!(col.header.as_deref(), Some("ID"));
}

#[test]
fn test_complete_task_title_column() {
    let spec = CompleteTask::tabular_spec();
    let col = &spec.columns[1];
    assert_eq!(col.name.as_deref(), Some("title"));
    assert_eq!(col.width, Width::Fill);
    assert_eq!(col.overflow, Overflow::Wrap { indent: 0 });
}

#[test]
fn test_complete_task_status_column() {
    let spec = CompleteTask::tabular_spec();
    let col = &spec.columns[2];
    assert_eq!(col.name.as_deref(), Some("status"));
    assert_eq!(col.width, Width::Fixed(12));
    assert_eq!(col.align, Align::Right);
    assert!(col.style_from_value);
}

#[test]
fn test_complete_task_due_column() {
    let spec = CompleteTask::tabular_spec();
    let col = &spec.columns[3];
    assert_eq!(col.name.as_deref(), Some("due"));
    assert_eq!(col.width, Width::Fixed(10));
    assert_eq!(col.anchor, Anchor::Right);
    assert_eq!(
        col.overflow,
        Overflow::Truncate {
            at: TruncateAt::Middle,
            marker: "…".to_string()
        }
    );
}

#[test]
fn test_complete_task_decorations() {
    let spec = CompleteTask::tabular_spec();
    assert_eq!(spec.decorations.column_sep, " │ ");
}
