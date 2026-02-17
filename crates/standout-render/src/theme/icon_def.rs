//! Icon definitions and icon set collections.
//!
//! Icons are characters (not images) used in terminal output. Each icon
//! has a classic (Unicode) variant and an optional Nerd Font variant.
//!
//! # Example
//!
//! ```rust
//! use standout_render::{IconDefinition, IconSet, IconMode};
//!
//! let icons = IconSet::new()
//!     .add("pending", IconDefinition::new("⚪"))
//!     .add("done", IconDefinition::new("⚫").with_nerdfont("\u{f00c}"))
//!     .add("timer", IconDefinition::new("⏲").with_nerdfont("\u{f017}"));
//!
//! // Resolve for classic mode
//! let resolved = icons.resolve(IconMode::Classic);
//! assert_eq!(resolved.get("pending").unwrap(), "⚪");
//! assert_eq!(resolved.get("done").unwrap(), "⚫");
//!
//! // Resolve for Nerd Font mode
//! let resolved = icons.resolve(IconMode::NerdFont);
//! assert_eq!(resolved.get("pending").unwrap(), "⚪"); // No nerdfont variant, uses classic
//! assert_eq!(resolved.get("done").unwrap(), "\u{f00c}");
//! ```

use std::collections::HashMap;

use super::icon_mode::IconMode;

/// A single icon definition with classic and optional Nerd Font variants.
///
/// The classic variant is always required and works in all terminals.
/// The Nerd Font variant is optional and used when the terminal has a
/// Nerd Font installed.
///
/// Icons can be N characters long, though they are typically a single character.
///
/// # Example
///
/// ```rust
/// use standout_render::{IconDefinition, IconMode};
///
/// // Classic-only icon
/// let icon = IconDefinition::new("⚪");
/// assert_eq!(icon.resolve(IconMode::Classic), "⚪");
/// assert_eq!(icon.resolve(IconMode::NerdFont), "⚪"); // Falls back to classic
///
/// // Icon with Nerd Font variant
/// let icon = IconDefinition::new("⚫").with_nerdfont("\u{f00c}");
/// assert_eq!(icon.resolve(IconMode::Classic), "⚫");
/// assert_eq!(icon.resolve(IconMode::NerdFont), "\u{f00c}");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IconDefinition {
    /// Classic variant (always required). Works in all terminals.
    pub classic: String,
    /// Nerd Font variant (optional). Used when Nerd Font is available.
    pub nerdfont: Option<String>,
}

impl IconDefinition {
    /// Creates a new icon definition with a classic variant.
    pub fn new(classic: impl Into<String>) -> Self {
        Self {
            classic: classic.into(),
            nerdfont: None,
        }
    }

    /// Adds a Nerd Font variant to this icon definition.
    pub fn with_nerdfont(mut self, nerdfont: impl Into<String>) -> Self {
        self.nerdfont = Some(nerdfont.into());
        self
    }

    /// Resolves the icon string for the given mode.
    ///
    /// In `NerdFont` mode, returns the Nerd Font variant if available,
    /// otherwise falls back to the classic variant.
    ///
    /// In `Classic` or `Auto` mode, always returns the classic variant.
    pub fn resolve(&self, mode: IconMode) -> &str {
        match mode {
            IconMode::NerdFont => self.nerdfont.as_deref().unwrap_or(&self.classic),
            IconMode::Classic | IconMode::Auto => &self.classic,
        }
    }
}

/// A collection of named icon definitions.
///
/// `IconSet` stores icon definitions and resolves them for a given
/// [`IconMode`] into a flat map of name → string.
///
/// # Example
///
/// ```rust
/// use standout_render::{IconSet, IconDefinition, IconMode};
///
/// let icons = IconSet::new()
///     .add("check", IconDefinition::new("[ok]").with_nerdfont("\u{f00c}"))
///     .add("cross", IconDefinition::new("[!!]").with_nerdfont("\u{f00d}"));
///
/// let resolved = icons.resolve(IconMode::Classic);
/// assert_eq!(resolved.get("check").unwrap(), "[ok]");
/// assert_eq!(resolved.get("cross").unwrap(), "[!!]");
/// ```
#[derive(Debug, Clone, Default)]
pub struct IconSet {
    icons: HashMap<String, IconDefinition>,
}

impl IconSet {
    /// Creates an empty icon set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds an icon definition, returning `self` for chaining.
    pub fn add(mut self, name: impl Into<String>, def: IconDefinition) -> Self {
        self.icons.insert(name.into(), def);
        self
    }

    /// Inserts an icon definition by mutable reference.
    pub fn insert(&mut self, name: impl Into<String>, def: IconDefinition) {
        self.icons.insert(name.into(), def);
    }

    /// Resolves all icons for the given mode into a flat name → string map.
    pub fn resolve(&self, mode: IconMode) -> HashMap<String, String> {
        self.icons
            .iter()
            .map(|(name, def)| (name.clone(), def.resolve(mode).to_string()))
            .collect()
    }

    /// Returns true if no icons are defined.
    pub fn is_empty(&self) -> bool {
        self.icons.is_empty()
    }

    /// Returns the number of defined icons.
    pub fn len(&self) -> usize {
        self.icons.len()
    }

    /// Merges another icon set into this one.
    ///
    /// Icons from `other` take precedence over icons in `self`.
    pub fn merge(mut self, other: IconSet) -> Self {
        self.icons.extend(other.icons);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // IconDefinition tests
    // =========================================================================

    #[test]
    fn test_icon_definition_classic_only() {
        let icon = IconDefinition::new("⚪");
        assert_eq!(icon.classic, "⚪");
        assert_eq!(icon.nerdfont, None);
    }

    #[test]
    fn test_icon_definition_with_nerdfont() {
        let icon = IconDefinition::new("⚫").with_nerdfont("\u{f00c}");
        assert_eq!(icon.classic, "⚫");
        assert_eq!(icon.nerdfont, Some("\u{f00c}".to_string()));
    }

    #[test]
    fn test_icon_definition_resolve_classic_mode() {
        let icon = IconDefinition::new("⚫").with_nerdfont("\u{f00c}");
        assert_eq!(icon.resolve(IconMode::Classic), "⚫");
    }

    #[test]
    fn test_icon_definition_resolve_nerdfont_mode() {
        let icon = IconDefinition::new("⚫").with_nerdfont("\u{f00c}");
        assert_eq!(icon.resolve(IconMode::NerdFont), "\u{f00c}");
    }

    #[test]
    fn test_icon_definition_resolve_nerdfont_fallback() {
        // No nerdfont variant, should fall back to classic
        let icon = IconDefinition::new("⚪");
        assert_eq!(icon.resolve(IconMode::NerdFont), "⚪");
    }

    #[test]
    fn test_icon_definition_resolve_auto_mode() {
        let icon = IconDefinition::new("⚫").with_nerdfont("\u{f00c}");
        // Auto mode resolves to classic
        assert_eq!(icon.resolve(IconMode::Auto), "⚫");
    }

    #[test]
    fn test_icon_definition_multi_char() {
        let icon = IconDefinition::new("[ok]").with_nerdfont("\u{f00c}");
        assert_eq!(icon.resolve(IconMode::Classic), "[ok]");
        assert_eq!(icon.resolve(IconMode::NerdFont), "\u{f00c}");
    }

    #[test]
    fn test_icon_definition_empty_string() {
        let icon = IconDefinition::new("");
        assert_eq!(icon.resolve(IconMode::Classic), "");
    }

    #[test]
    fn test_icon_definition_equality() {
        let a = IconDefinition::new("⚪").with_nerdfont("nf");
        let b = IconDefinition::new("⚪").with_nerdfont("nf");
        assert_eq!(a, b);
    }

    // =========================================================================
    // IconSet tests
    // =========================================================================

    #[test]
    fn test_icon_set_new_is_empty() {
        let set = IconSet::new();
        assert!(set.is_empty());
        assert_eq!(set.len(), 0);
    }

    #[test]
    fn test_icon_set_add() {
        let set = IconSet::new()
            .add("pending", IconDefinition::new("⚪"))
            .add("done", IconDefinition::new("⚫"));
        assert_eq!(set.len(), 2);
        assert!(!set.is_empty());
    }

    #[test]
    fn test_icon_set_insert() {
        let mut set = IconSet::new();
        set.insert("pending", IconDefinition::new("⚪"));
        assert_eq!(set.len(), 1);
    }

    #[test]
    fn test_icon_set_resolve_classic() {
        let set = IconSet::new()
            .add("pending", IconDefinition::new("⚪"))
            .add("done", IconDefinition::new("⚫").with_nerdfont("\u{f00c}"));

        let resolved = set.resolve(IconMode::Classic);
        assert_eq!(resolved.get("pending").unwrap(), "⚪");
        assert_eq!(resolved.get("done").unwrap(), "⚫");
    }

    #[test]
    fn test_icon_set_resolve_nerdfont() {
        let set = IconSet::new()
            .add("pending", IconDefinition::new("⚪"))
            .add("done", IconDefinition::new("⚫").with_nerdfont("\u{f00c}"));

        let resolved = set.resolve(IconMode::NerdFont);
        assert_eq!(resolved.get("pending").unwrap(), "⚪"); // No nerdfont, falls back
        assert_eq!(resolved.get("done").unwrap(), "\u{f00c}");
    }

    #[test]
    fn test_icon_set_merge() {
        let base = IconSet::new()
            .add("keep", IconDefinition::new("K"))
            .add("override", IconDefinition::new("OLD"));

        let extension = IconSet::new()
            .add("override", IconDefinition::new("NEW"))
            .add("added", IconDefinition::new("A"));

        let merged = base.merge(extension);

        assert_eq!(merged.len(), 3);
        let resolved = merged.resolve(IconMode::Classic);
        assert_eq!(resolved.get("keep").unwrap(), "K");
        assert_eq!(resolved.get("override").unwrap(), "NEW");
        assert_eq!(resolved.get("added").unwrap(), "A");
    }

    #[test]
    fn test_icon_set_resolve_empty() {
        let set = IconSet::new();
        let resolved = set.resolve(IconMode::Classic);
        assert!(resolved.is_empty());
    }
}
