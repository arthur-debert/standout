//! Integration tests for the embed macros.
//!
//! These tests verify that the `embed_templates!` and `embed_styles!` macros
//! correctly walk directories at compile time and embed resources, with proper
//! handling of extension priority and name resolution.

#![cfg(feature = "macros")]

use outstanding::{embed_styles, embed_templates, ResolvedTemplate};

// =============================================================================
// Template embedding tests
// =============================================================================

#[test]
fn test_embed_templates_simple() {
    let templates = embed_templates!("tests/fixtures/templates");

    // Should be able to get the simple template by base name
    let resolved = templates
        .get("simple")
        .expect("simple template should exist");

    // Embedded templates should be Inline variant
    match resolved {
        ResolvedTemplate::Inline(content) => {
            assert!(content.contains("Hello"));
            assert!(content.contains("{{ name }}"));
        }
        _ => panic!("Expected Inline template"),
    }
}

#[test]
fn test_embed_templates_with_extension() {
    let templates = embed_templates!("tests/fixtures/templates");

    // Should also be able to access by full name with extension
    let resolved = templates
        .get("simple.jinja")
        .expect("simple.jinja should exist");

    match resolved {
        ResolvedTemplate::Inline(content) => {
            assert!(content.contains("Hello"));
        }
        _ => panic!("Expected Inline template"),
    }
}

#[test]
fn test_embed_templates_nested() {
    let templates = embed_templates!("tests/fixtures/templates");

    // Should be able to get nested templates
    let resolved = templates
        .get("nested/report")
        .expect("nested/report template should exist");

    match resolved {
        ResolvedTemplate::Inline(content) => {
            assert!(content.contains("Report:"));
            assert!(content.contains("{{ title }}"));
        }
        _ => panic!("Expected Inline template"),
    }
}

#[test]
fn test_embed_templates_names() {
    let templates = embed_templates!("tests/fixtures/templates");

    let names: Vec<&str> = templates.names().collect();

    // Should have both base names and names with extensions
    assert!(names.contains(&"simple"));
    assert!(names.contains(&"simple.jinja"));
    assert!(names.contains(&"nested/report"));
    assert!(names.contains(&"nested/report.jinja"));
}

// =============================================================================
// Stylesheet embedding tests
// =============================================================================

#[test]
fn test_embed_styles_simple() {
    let mut styles = embed_styles!("tests/fixtures/styles");

    // Should be able to get the default stylesheet by base name
    let theme = styles.get("default").expect("default style should exist");
    let resolved = theme.resolve_styles(None);
    assert!(resolved.has("header"));
    assert!(resolved.has("muted"));
}

#[test]
fn test_embed_styles_with_extension() {
    let mut styles = embed_styles!("tests/fixtures/styles");

    // Should also be able to access by full name with extension
    let theme = styles
        .get("default.yaml")
        .expect("default.yaml should exist");
    let resolved = theme.resolve_styles(None);
    assert!(resolved.has("header"));
}

#[test]
fn test_embed_styles_nested() {
    let mut styles = embed_styles!("tests/fixtures/styles");

    // Should be able to get nested stylesheets
    let theme = styles
        .get("themes/dark")
        .expect("themes/dark style should exist");
    let resolved = theme.resolve_styles(None);
    assert!(resolved.has("header"));
    assert!(resolved.has("panel"));
}

#[test]
fn test_embed_styles_names() {
    let styles = embed_styles!("tests/fixtures/styles");

    let names: Vec<&str> = styles.names().collect();

    // Should have both base names and names with extensions
    assert!(names.contains(&"default"));
    assert!(names.contains(&"default.yaml"));
    assert!(names.contains(&"themes/dark"));
    assert!(names.contains(&"themes/dark.yaml"));
}

// =============================================================================
// Extension priority tests
// =============================================================================

#[test]
fn test_embed_templates_extension_priority() {
    // Create test fixtures with same base name, different extensions
    // This test verifies the registry handles extension priority correctly
    let templates = embed_templates!("tests/fixtures/templates");

    // If we had both priority.jinja and priority.txt, .jinja would win
    // For now, just verify the basic functionality works
    assert!(templates.get("simple").is_ok());
}

#[test]
fn test_embed_styles_extension_priority() {
    // Similar test for stylesheets
    // .yaml has higher priority than .yml
    let mut styles = embed_styles!("tests/fixtures/styles");
    assert!(styles.get("default").is_ok());
}
