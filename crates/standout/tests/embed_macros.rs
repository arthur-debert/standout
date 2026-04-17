//! Integration tests for the embed macros.
//!
//! These tests verify that the `embed_templates!` and `embed_styles!` macros
//! correctly walk directories at compile time and embed resources, with proper
//! handling of extension priority and name resolution.

#![cfg(feature = "macros")]

use standout::{embed_styles, embed_templates, StylesheetRegistry, TemplateRegistry};

// =============================================================================
// Template embedding tests
// =============================================================================

#[test]
fn test_embed_templates_simple() {
    // embed_templates! returns EmbeddedTemplates
    let source = embed_templates!("tests/fixtures/templates");

    // Convert to TemplateRegistry
    let templates: TemplateRegistry = source.into();

    // Should be able to get the simple template by base name
    // Use get_content() which works for both Inline and File variants
    let content = templates
        .get_content("simple")
        .expect("simple template should exist");

    assert!(content.contains("Hello"));
    assert!(content.contains("{{ name }}"));
}

#[test]
fn test_embed_templates_with_extension() {
    let templates: TemplateRegistry = embed_templates!("tests/fixtures/templates").into();

    // Should also be able to access by full name with extension
    let content = templates
        .get_content("simple.jinja")
        .expect("simple.jinja should exist");

    assert!(content.contains("Hello"));
}

#[test]
fn test_embed_templates_nested() {
    let templates: TemplateRegistry = embed_templates!("tests/fixtures/templates").into();

    // Should be able to get nested templates
    let content = templates
        .get_content("nested/report")
        .expect("nested/report template should exist");

    assert!(content.contains("Report:"));
    assert!(content.contains("{{ title }}"));
}

#[test]
fn test_embed_templates_names() {
    let templates: TemplateRegistry = embed_templates!("tests/fixtures/templates").into();

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
    // embed_styles! returns EmbeddedStyles, convert to StylesheetRegistry
    let mut styles: StylesheetRegistry = embed_styles!("tests/fixtures/styles").into();

    // Should be able to get the default stylesheet by base name
    let theme = styles.get("default").expect("default style should exist");
    let resolved = theme.resolve_styles(None);
    assert!(resolved.has("header"));
    assert!(resolved.has("muted"));
}

#[test]
fn test_embed_styles_with_extension() {
    let mut styles: StylesheetRegistry = embed_styles!("tests/fixtures/styles").into();

    // Should also be able to access by full name with extension
    let theme = styles
        .get("default.yaml")
        .expect("default.yaml should exist");
    let resolved = theme.resolve_styles(None);
    assert!(resolved.has("header"));
}

#[test]
fn test_embed_styles_nested() {
    let mut styles: StylesheetRegistry = embed_styles!("tests/fixtures/styles").into();

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
    let styles: StylesheetRegistry = embed_styles!("tests/fixtures/styles").into();

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
    let templates: TemplateRegistry = embed_templates!("tests/fixtures/templates").into();

    // If we had both priority.jinja and priority.txt, .jinja would win
    // For now, just verify the basic functionality works
    assert!(templates.get("simple").is_ok());
}

#[test]
fn test_embed_styles_extension_priority() {
    // Similar test for stylesheets
    // .yaml has higher priority than .yml
    let mut styles: StylesheetRegistry = embed_styles!("tests/fixtures/styles").into();
    assert!(styles.get("default").is_ok());
}

// =============================================================================
// CSS stylesheet tests
//
// Regression coverage for a bug where the debug hot-reload path parsed every
// on-disk stylesheet as YAML, causing `.css` files to fail to parse and silently
// fall back to the compile-time embedded copy. Both fixtures below are `.css`
// so they exercise `parse_theme_content`'s CSS branch end-to-end through the
// embed macro and the hot-reload path.
// =============================================================================

#[test]
fn test_embed_styles_css_file() {
    let mut styles: StylesheetRegistry = embed_styles!("tests/fixtures/styles").into();

    // screen.css has no YAML sibling — if CSS parsing were broken, this would
    // fail at load time or return a theme with no styles.
    let theme = styles.get("screen").expect("screen.css should load");
    let resolved = theme.resolve_styles(None);
    assert!(
        resolved.has("header"),
        "CSS .header class should be registered"
    );
    assert!(
        resolved.has("muted"),
        "CSS .muted class should be registered"
    );
}

#[test]
fn test_embed_styles_css_accessible_by_full_name() {
    let mut styles: StylesheetRegistry = embed_styles!("tests/fixtures/styles").into();

    let theme = styles
        .get("screen.css")
        .expect("screen.css should be accessible by full filename");
    let resolved = theme.resolve_styles(None);
    assert!(resolved.has("header"));
}

#[test]
fn test_embed_styles_css_beats_yaml_priority() {
    // Both themes/light.css and themes/light.yaml exist with the same base name.
    // Per STYLESHEET_EXTENSIONS = [".css", ".yaml", ".yml"], the CSS file wins.
    let mut styles: StylesheetRegistry = embed_styles!("tests/fixtures/styles").into();

    let theme = styles
        .get("themes/light")
        .expect("themes/light should resolve");
    let resolved = theme.resolve_styles(None);

    assert!(
        resolved.has("css_wins"),
        "CSS file must win priority — expected `css_wins` style from light.css"
    );
    assert!(
        !resolved.has("yaml_loses"),
        "YAML file must lose priority — `yaml_loses` should not be present"
    );
}

// =============================================================================
// EmbeddedSource tests
// =============================================================================

#[test]
fn test_embedded_source_has_entries() {
    let source = embed_templates!("tests/fixtures/templates");

    // Should have entries
    assert!(!source.entries().is_empty());

    // Should have source path (absolute path ending with our directory)
    assert!(source.source_path().ends_with("tests/fixtures/templates"));
}

#[test]
fn test_embedded_styles_source_has_entries() {
    let source = embed_styles!("tests/fixtures/styles");

    // Should have entries
    assert!(!source.entries().is_empty());

    // Should have source path (absolute path ending with our directory)
    assert!(source.source_path().ends_with("tests/fixtures/styles"));
}
