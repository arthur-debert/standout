//! Stylesheet registry for file-based theme loading.
//!
//! This module provides [`StylesheetRegistry`], which manages theme resolution
//! from multiple sources: inline YAML, filesystem directories, or embedded content.
//!
//! # Design
//!
//! The registry is a thin wrapper around [`FileRegistry<Theme>`](crate::file_loader::FileRegistry),
//! providing stylesheet-specific functionality while reusing the generic file loading infrastructure.
//!
//! The registry uses a two-phase approach:
//!
//! 1. **Collection**: Stylesheets are collected from various sources (inline, directories, embedded)
//! 2. **Resolution**: A unified map resolves theme names to their parsed `Theme` instances
//!
//! This separation enables:
//! - **Testability**: Resolution logic can be tested without filesystem access
//! - **Flexibility**: Same resolution rules apply regardless of stylesheet source
//! - **Hot reloading**: Files are re-read and re-parsed on each access in development mode
//!
//! # Stylesheet Resolution
//!
//! Stylesheets are resolved by name using these rules:
//!
//! 1. **Inline stylesheets** (added via [`StylesheetRegistry::add_inline`]) have highest priority
//! 2. **File stylesheets** are searched in directory registration order (first directory wins)
//! 3. Names can be specified with or without extension: both `"darcula"` and `"darcula.yaml"` resolve
//!
//! # Supported Extensions
//!
//! Stylesheet files are recognized by extension, in priority order:
//!
//! | Priority | Extension | Description |
//! |----------|-----------|-------------|
//! | 1 (highest) | `.yaml` | Standard YAML extension |
//! | 2 (lowest) | `.yml` | Short YAML extension |
//!
//! If multiple files exist with the same base name but different extensions
//! (e.g., `darcula.yaml` and `darcula.yml`), the higher-priority extension wins.
//!
//! # Collision Handling
//!
//! The registry enforces strict collision rules:
//!
//! - **Same-directory, different extensions**: Higher priority extension wins (no error)
//! - **Cross-directory collisions**: Panic with detailed message listing conflicting files
//!
//! # Example
//!
//! ```rust,ignore
//! use outstanding::stylesheet::StylesheetRegistry;
//!
//! let mut registry = StylesheetRegistry::new();
//! registry.add_dir("./themes")?;
//!
//! // Get a theme by name
//! let theme = registry.get("darcula")?;
//! ```

use std::collections::HashMap;
use std::path::Path;

use crate::file_loader::{FileRegistry, FileRegistryConfig, LoadError};
use crate::theme::Theme;

use super::error::StylesheetError;

/// Recognized stylesheet file extensions in priority order.
///
/// When multiple files exist with the same base name but different extensions,
/// the extension appearing earlier in this list takes precedence.
pub const STYLESHEET_EXTENSIONS: &[&str] = &[".yaml", ".yml"];

/// Creates the file registry configuration for stylesheets.
fn stylesheet_config() -> FileRegistryConfig<Theme> {
    FileRegistryConfig {
        extensions: STYLESHEET_EXTENSIONS,
        transform: |content| {
            Theme::from_yaml(content).map_err(|e| LoadError::Transform {
                name: String::new(), // FileRegistry fills in the actual name
                message: e.to_string(),
            })
        },
    }
}

/// Registry for stylesheet/theme resolution from multiple sources.
///
/// The registry maintains a unified view of themes from:
/// - Inline YAML strings (highest priority)
/// - Multiple filesystem directories
/// - Embedded content (for release builds)
///
/// # Resolution Order
///
/// When looking up a theme name:
///
/// 1. Check inline themes first
/// 2. Check file-based themes in registration order
/// 3. Return error if not found
///
/// # Hot Reloading
///
/// In development mode (debug builds), file-based themes are re-read and
/// re-parsed on each access, enabling rapid iteration without restarts.
///
/// # Example
///
/// ```rust,ignore
/// let mut registry = StylesheetRegistry::new();
///
/// // Add inline theme (highest priority)
/// registry.add_inline("custom", r#"
/// header:
///   fg: cyan
///   bold: true
/// "#)?;
///
/// // Add from directory
/// registry.add_dir("./themes")?;
///
/// // Get a theme
/// let theme = registry.get("darcula")?;
/// ```
pub struct StylesheetRegistry {
    /// The underlying file registry for directory-based file loading.
    inner: FileRegistry<Theme>,

    /// Inline themes (stored separately for highest priority).
    inline: HashMap<String, Theme>,
}

impl Default for StylesheetRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl StylesheetRegistry {
    /// Creates an empty stylesheet registry.
    pub fn new() -> Self {
        Self {
            inner: FileRegistry::new(stylesheet_config()),
            inline: HashMap::new(),
        }
    }

    /// Adds an inline theme from a YAML string.
    ///
    /// Inline themes have the highest priority and will shadow any
    /// file-based themes with the same name.
    ///
    /// # Arguments
    ///
    /// * `name` - The theme name for resolution
    /// * `yaml` - The YAML content defining the theme
    ///
    /// # Errors
    ///
    /// Returns an error if the YAML content cannot be parsed.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// registry.add_inline("custom", r#"
    /// header:
    ///   fg: cyan
    ///   bold: true
    /// muted:
    ///   dim: true
    /// "#)?;
    /// ```
    pub fn add_inline(
        &mut self,
        name: impl Into<String>,
        yaml: &str,
    ) -> Result<(), StylesheetError> {
        let theme = Theme::from_yaml(yaml)?;
        self.inline.insert(name.into(), theme);
        Ok(())
    }

    /// Adds a pre-parsed theme directly.
    ///
    /// This is useful when you have a `Theme` instance already constructed
    /// programmatically and want to register it in the registry.
    ///
    /// # Arguments
    ///
    /// * `name` - The theme name for resolution
    /// * `theme` - The pre-built theme instance
    pub fn add_theme(&mut self, name: impl Into<String>, theme: Theme) {
        self.inline.insert(name.into(), theme);
    }

    /// Adds a stylesheet directory to search for files.
    ///
    /// Themes in the directory are resolved by their filename without
    /// extension. For example, with directory `./themes`:
    ///
    /// - `"darcula"` → `./themes/darcula.yaml`
    /// - `"monokai"` → `./themes/monokai.yaml`
    ///
    /// # Errors
    ///
    /// Returns an error if the directory doesn't exist.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// registry.add_dir("./themes")?;
    /// let theme = registry.get("darcula")?;
    /// ```
    pub fn add_dir<P: AsRef<Path>>(&mut self, path: P) -> Result<(), StylesheetError> {
        self.inner.add_dir(path).map_err(|e| StylesheetError::Load {
            message: e.to_string(),
        })
    }

    /// Adds pre-embedded themes (for release builds).
    ///
    /// Embedded themes are stored directly in memory without filesystem access.
    /// This is typically used with `include_str!` to bundle themes at compile time.
    ///
    /// # Arguments
    ///
    /// * `themes` - Map of theme name to parsed Theme
    pub fn add_embedded(&mut self, themes: HashMap<String, Theme>) {
        for (name, theme) in themes {
            self.inline.insert(name, theme);
        }
    }

    /// Adds a pre-embedded theme by name.
    ///
    /// This is a convenience method for adding a single embedded theme.
    ///
    /// # Arguments
    ///
    /// * `name` - The theme name for resolution
    /// * `theme` - The pre-built theme instance
    pub fn add_embedded_theme(&mut self, name: impl Into<String>, theme: Theme) {
        self.inner.add_embedded(&name.into(), theme);
    }

    /// Gets a theme by name.
    ///
    /// Looks up the theme in order: inline first, then file-based.
    /// In development mode, file-based themes are re-read on each access.
    ///
    /// # Arguments
    ///
    /// * `name` - The theme name (with or without extension)
    ///
    /// # Errors
    ///
    /// Returns an error if the theme is not found or cannot be parsed.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let theme = registry.get("darcula")?;
    /// ```
    pub fn get(&mut self, name: &str) -> Result<Theme, StylesheetError> {
        // Check inline first
        if let Some(theme) = self.inline.get(name) {
            return Ok(theme.clone());
        }

        // Try file-based
        self.inner.get(name).map_err(|e| StylesheetError::Load {
            message: e.to_string(),
        })
    }

    /// Checks if a theme exists in the registry.
    ///
    /// # Arguments
    ///
    /// * `name` - The theme name to check
    pub fn contains(&self, name: &str) -> bool {
        self.inline.contains_key(name) || self.inner.get_entry(name).is_some()
    }

    /// Returns an iterator over all registered theme names.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.inline
            .keys()
            .map(|s| s.as_str())
            .chain(self.inner.names())
    }

    /// Returns the number of registered themes.
    pub fn len(&self) -> usize {
        self.inline.len() + self.inner.len()
    }

    /// Returns true if no themes are registered.
    pub fn is_empty(&self) -> bool {
        self.inline.is_empty() && self.inner.is_empty()
    }

    /// Clears all registered themes.
    pub fn clear(&mut self) {
        self.inline.clear();
        self.inner.clear();
    }

    /// Refreshes file-based themes from disk.
    ///
    /// This re-walks all registered directories and updates the internal
    /// cache. Useful in long-running applications that need to pick up
    /// theme changes without restarting.
    ///
    /// # Errors
    ///
    /// Returns an error if any directory cannot be read.
    pub fn refresh(&mut self) -> Result<(), StylesheetError> {
        self.inner.refresh().map_err(|e| StylesheetError::Load {
            message: e.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_registry_new_is_empty() {
        let registry = StylesheetRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn test_registry_add_inline() {
        let mut registry = StylesheetRegistry::new();
        registry
            .add_inline(
                "test",
                r#"
                header:
                    fg: cyan
                    bold: true
                "#,
            )
            .unwrap();

        assert!(!registry.is_empty());
        assert_eq!(registry.len(), 1);
        assert!(registry.contains("test"));
    }

    #[test]
    fn test_registry_add_theme() {
        let mut registry = StylesheetRegistry::new();
        let theme = Theme::new().add("header", console::Style::new().cyan().bold());
        registry.add_theme("custom", theme);

        assert!(registry.contains("custom"));
        let retrieved = registry.get("custom").unwrap();
        assert!(retrieved.resolve_styles(None).has("header"));
    }

    #[test]
    fn test_registry_get_inline() {
        let mut registry = StylesheetRegistry::new();
        registry
            .add_inline(
                "darcula",
                r#"
                header:
                    fg: cyan
                muted:
                    dim: true
                "#,
            )
            .unwrap();

        let theme = registry.get("darcula").unwrap();
        let styles = theme.resolve_styles(None);
        assert!(styles.has("header"));
        assert!(styles.has("muted"));
    }

    #[test]
    fn test_registry_add_dir() {
        let temp_dir = TempDir::new().unwrap();
        let theme_path = temp_dir.path().join("monokai.yaml");
        fs::write(
            &theme_path,
            r#"
            keyword:
                fg: magenta
                bold: true
            string:
                fg: green
            "#,
        )
        .unwrap();

        let mut registry = StylesheetRegistry::new();
        registry.add_dir(temp_dir.path()).unwrap();

        let theme = registry.get("monokai").unwrap();
        let styles = theme.resolve_styles(None);
        assert!(styles.has("keyword"));
        assert!(styles.has("string"));
    }

    #[test]
    fn test_registry_inline_shadows_file() {
        let temp_dir = TempDir::new().unwrap();
        let theme_path = temp_dir.path().join("test.yaml");
        fs::write(
            &theme_path,
            r#"
            from_file:
                fg: red
            header:
                fg: red
            "#,
        )
        .unwrap();

        let mut registry = StylesheetRegistry::new();
        registry.add_dir(temp_dir.path()).unwrap();
        registry
            .add_inline(
                "test",
                r#"
            from_inline:
                fg: blue
            header:
                fg: blue
            "#,
            )
            .unwrap();

        // Inline should win
        let theme = registry.get("test").unwrap();
        let styles = theme.resolve_styles(None);
        assert!(styles.has("from_inline"));
        assert!(!styles.has("from_file"));
    }

    #[test]
    fn test_registry_extension_priority() {
        let temp_dir = TempDir::new().unwrap();

        // Create both .yaml and .yml with different content
        fs::write(
            temp_dir.path().join("theme.yaml"),
            r#"
            from_yaml:
                fg: cyan
            source:
                fg: cyan
            "#,
        )
        .unwrap();

        fs::write(
            temp_dir.path().join("theme.yml"),
            r#"
            from_yml:
                fg: red
            source:
                fg: red
            "#,
        )
        .unwrap();

        let mut registry = StylesheetRegistry::new();
        registry.add_dir(temp_dir.path()).unwrap();

        // .yaml should win over .yml
        let theme = registry.get("theme").unwrap();
        let styles = theme.resolve_styles(None);
        assert!(styles.has("from_yaml"));
        assert!(!styles.has("from_yml"));
    }

    #[test]
    fn test_registry_names() {
        let mut registry = StylesheetRegistry::new();
        registry.add_inline("alpha", "header: bold").unwrap();
        registry.add_inline("beta", "header: dim").unwrap();

        let names: Vec<&str> = registry.names().collect();
        assert!(names.contains(&"alpha"));
        assert!(names.contains(&"beta"));
    }

    #[test]
    fn test_registry_clear() {
        let mut registry = StylesheetRegistry::new();
        registry.add_inline("test", "header: bold").unwrap();
        assert!(!registry.is_empty());

        registry.clear();
        assert!(registry.is_empty());
    }

    #[test]
    fn test_registry_not_found() {
        let mut registry = StylesheetRegistry::new();
        let result = registry.get("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_registry_invalid_yaml() {
        let mut registry = StylesheetRegistry::new();
        let result = registry.add_inline("bad", "not: [valid: yaml");
        assert!(result.is_err());
    }

    #[test]
    fn test_registry_hot_reload() {
        let temp_dir = TempDir::new().unwrap();
        let theme_path = temp_dir.path().join("dynamic.yaml");
        fs::write(
            &theme_path,
            r#"
            version_v1:
                fg: red
            header:
                fg: red
            "#,
        )
        .unwrap();

        let mut registry = StylesheetRegistry::new();
        registry.add_dir(temp_dir.path()).unwrap();

        // First read
        let theme1 = registry.get("dynamic").unwrap();
        let styles1 = theme1.resolve_styles(None);
        assert!(styles1.has("version_v1"));

        // Update the file
        fs::write(
            &theme_path,
            r#"
            version_v2:
                fg: green
            updated_style:
                fg: blue
            header:
                fg: blue
            "#,
        )
        .unwrap();

        // Refresh and read again
        registry.refresh().unwrap();
        let theme2 = registry.get("dynamic").unwrap();
        let styles2 = theme2.resolve_styles(None);
        assert!(styles2.has("updated_style"));
    }

    #[test]
    fn test_registry_adaptive_theme() {
        let mut registry = StylesheetRegistry::new();
        registry
            .add_inline(
                "adaptive",
                r#"
            panel:
                fg: gray
                light:
                    fg: black
                dark:
                    fg: white
            "#,
            )
            .unwrap();

        let theme = registry.get("adaptive").unwrap();

        // Check light mode
        let light_styles = theme.resolve_styles(Some(crate::ColorMode::Light));
        assert!(light_styles.has("panel"));

        // Check dark mode
        let dark_styles = theme.resolve_styles(Some(crate::ColorMode::Dark));
        assert!(dark_styles.has("panel"));
    }
}
