//! Proc macros for compile-time resource embedding in Outstanding.
//!
//! This crate provides macros that walk directories at compile time and embed
//! all matching files into the binary. This enables single-binary distribution
//! without external file dependencies.
//!
//! # Available Macros
//!
//! - [`embed_templates!`] - Embed template files (`.jinja`, `.jinja2`, `.j2`, `.txt`)
//! - [`embed_styles!`] - Embed stylesheet files (`.yaml`, `.yml`)
//!
//! # Design Philosophy
//!
//! These macros are intentionally minimal ("dumb"). They only:
//!
//! 1. Walk the directory at compile time
//! 2. Filter files by extension
//! 3. Read file contents
//! 4. Pass raw `(name_with_ext, content)` pairs to the outstanding crate
//!
//! All "smart" logic (extension priority, name normalization, collision detection)
//! lives in the `outstanding` crate's registries. This design:
//!
//! - **Avoids duplication**: Logic exists in one place
//! - **Ensures consistency**: Same behavior for runtime and compile-time loading
//! - **Simplifies debugging**: Macros are easier to troubleshoot when simple
//!
//! # Relationship to file_loader
//!
//! These macros are the compile-time counterpart to runtime file loading provided
//! by [`outstanding::file_loader`]. Both approaches use the same registry APIs
//! and produce identical behavior.
//!
//! | Mode | File Source | Hot Reload | Use Case |
//! |------|-------------|------------|----------|
//! | Runtime (`add_dir`) | Filesystem | Yes | Development |
//! | Compile-time (`embed_*!`) | Embedded | No | Release |
//!
//! See [`outstanding::file_loader`] for the complete file loading documentation.
//!
//! # Example
//!
//! ```rust,ignore
//! use outstanding::{embed_templates, embed_styles};
//!
//! // Development: runtime loading with hot reload
//! // let mut templates = TemplateRegistry::new();
//! // templates.add_template_dir("./templates")?;
//!
//! // Release: compile-time embedding
//! let templates = embed_templates!("./templates");
//! let styles = embed_styles!("./styles");
//!
//! // Same API for both approaches
//! let content = templates.get_content("report/summary")?;
//! let theme = styles.get("dark")?;
//! ```

mod embed;

use proc_macro::TokenStream;
use syn::{parse_macro_input, LitStr};

/// Embeds all template files from a directory at compile time.
///
/// This macro walks the specified directory, reads all files with recognized
/// template extensions, and generates a [`TemplateRegistry`] with all templates
/// pre-loaded.
///
/// # Supported Extensions
///
/// Files are recognized by extension (in priority order):
/// - `.jinja` (highest priority)
/// - `.jinja2`
/// - `.j2`
/// - `.txt` (lowest priority)
///
/// When multiple files share the same base name with different extensions
/// (e.g., `config.jinja` and `config.txt`), the higher-priority extension wins
/// for extensionless lookups.
///
/// # Name Resolution
///
/// Files are named by their relative path from the root directory, without extension:
///
/// | File Path | Resolution Name |
/// |-----------|-----------------|
/// | `list.jinja` | `"list"` |
/// | `report/summary.jinja` | `"report/summary"` |
/// | `report/summary.jinja` | `"report/summary.jinja"` (explicit) |
///
/// # Example
///
/// ```rust,ignore
/// use outstanding::embed_templates;
///
/// let templates = embed_templates!("./templates");
///
/// // Access by base name (extension stripped)
/// let content = templates.get_content("report/summary")?;
///
/// // Or explicitly with extension
/// let content = templates.get_content("report/summary.jinja")?;
/// ```
///
/// # Compile-Time Errors
///
/// The macro will fail to compile if:
/// - The directory doesn't exist
/// - The directory is not readable
/// - Any file content is not valid UTF-8
///
/// [`TemplateRegistry`]: outstanding::TemplateRegistry
#[proc_macro]
pub fn embed_templates(input: TokenStream) -> TokenStream {
    let path_lit = parse_macro_input!(input as LitStr);
    embed::embed_templates_impl(path_lit).into()
}

/// Embeds all stylesheet files from a directory at compile time.
///
/// This macro walks the specified directory, reads all files with recognized
/// stylesheet extensions, parses them as YAML themes, and generates a
/// [`StylesheetRegistry`] with all themes pre-loaded.
///
/// # Supported Extensions
///
/// Files are recognized by extension (in priority order):
/// - `.yaml` (highest priority)
/// - `.yml` (lowest priority)
///
/// When multiple files share the same base name with different extensions
/// (e.g., `dark.yaml` and `dark.yml`), the higher-priority extension wins.
///
/// # Name Resolution
///
/// Files are named by their relative path from the root directory, without extension:
///
/// | File Path | Resolution Name |
/// |-----------|-----------------|
/// | `default.yaml` | `"default"` |
/// | `themes/dark.yaml` | `"themes/dark"` |
///
/// # Example
///
/// ```rust,ignore
/// use outstanding::embed_styles;
///
/// let mut styles = embed_styles!("./styles");
///
/// // Access by base name (extension stripped)
/// let theme = styles.get("themes/dark")?;
/// ```
///
/// # Compile-Time Errors
///
/// The macro will fail to compile if:
/// - The directory doesn't exist
/// - The directory is not readable
/// - Any file content is not valid UTF-8
///
/// # Runtime Errors
///
/// The generated code will panic at runtime if any YAML file fails to parse.
/// This should be caught during development since the same files would fail
/// with runtime loading.
///
/// [`StylesheetRegistry`]: outstanding::stylesheet::StylesheetRegistry
#[proc_macro]
pub fn embed_styles(input: TokenStream) -> TokenStream {
    let path_lit = parse_macro_input!(input as LitStr);
    embed::embed_styles_impl(path_lit).into()
}
