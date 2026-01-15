//! CSS stylesheet parsing.
//!
//! # Motivation
//!
//! While YAML is excellent for structured data, it can be verbose for defining style rules.
//! CSS is the industry standard for styling, offering a syntax that is both familiar
//! to developers and concise for defining visual attributes.
//!
//! By supporting CSS, `outstanding` allows developers to leverage their existing knowledge
//! and potentially use standard tooling (like syntax highlighters) to define their terminal
//! themes.
//!
//! # Design
//!
//! This module implements a subset of CSS level 3, tailored for terminal styling.
//! It maps CSS selectors to `outstanding` style types and CSS properties to
//! terminal attributes (ANSI codes).
//!
//! The parser is built on top of `cssparser` (the same tokenizer used by Firefox),
//! ensuring robust handling of syntax, comments, and escapes.
//!
//! ## Mapping
//!
//! - **Selectors**: CSS class selectors (`.my-style`) map directly to style names in the theme.
//!   Currently, simple class selectors are supported.
//!   - `.error` -> defines style "error"
//!   - `.title, .header` -> defines styles "title" and "header"
//!
//! - **Properties**: Standard CSS properties are mapped to terminal equivalents.
//!   - `color` -> Foreground color
//!   - `background-color` -> Background color
//!   - `font-weight: bold` -> Bold text
//!   - `text-decoration: underline` -> Underlined text
//!   - `visibility: hidden` -> Hidden text
//!
//! - **Adaptive Styles**: Media queries are used to define light/dark mode overrides.
//!   - `@media (prefers-color-scheme: dark) { ... }`
//!
//! # Supported Attributes
//!
//! The following properties are supported:
//!
//! | CSS Property | Value | Effect |
//! |--------------|-------|--------|
//! | `color`, `fg` | Color (Hex, Named, Integer) | Sets the text color |
//! | `background-color`, `bg` | Color (Hex, Named, Integer) | Sets the background color |
//! | `font-weight` | `bold` | Makes text **bold** |
//! | `font-style` | `italic` | Makes text *italic* |
//! | `text-decoration` | `underline`, `line-through` | Underlines or strikes through text |
//! | `visibility` | `hidden` | Hides the text |
//! | `bold`, `italic`, `dim`, `blink`, `reverse`, `hidden` | `true`, `false` | Direct control over ANSI flags |
//!
//! # Example
//!
//! ```css
//! /* Base styles applied to all themes */
//! .title {
//!     font-weight: bold;
//!     color: #ff00ff; /* Magenta */
//! }
//!
//! .error {
//!     color: red;
//!     font-weight: bold;
//! }
//!
//! /* Semantic alias */
//! .critical {
//!     color: red;
//!     text-decoration: underline;
//!     animation: blink; /* parsing 'blink' property directly is also supported */
//! }
//!
//! /* Adaptive Overrides */
//! @media (prefers-color-scheme: dark) {
//!     .title {
//!         color: #ffcccc; /* Lighter magenta for dark backgrounds */
//!     }
//! }
//!
//! @media (prefers-color-scheme: light) {
//!     .title {
//!         color: #880088; /* Darker magenta for light backgrounds */
//!     }
//! }
//! ```
//!
use std::collections::HashMap;

use cssparser::{
    AtRuleParser, CowRcStr, DeclarationParser, ParseError, Parser, ParserInput, ParserState,
    QualifiedRuleParser, RuleBodyItemParser, RuleBodyParser, Token,
};

use super::attributes::StyleAttributes;
use super::color::ColorDef;
use super::definition::StyleDefinition;
use super::error::StylesheetError;
use super::parser::{build_variants, ThemeVariants};

/// Parses a CSS stylesheet and builds theme variants.
pub fn parse_css(css: &str) -> Result<ThemeVariants, StylesheetError> {
    let mut input = ParserInput::new(css);
    let mut parser = Parser::new(&mut input);

    let mut css_parser = StyleSheetParser {
        definitions: HashMap::new(),
        current_mode: None,
    };

    let rule_list_parser = cssparser::StyleSheetParser::new(&mut parser, &mut css_parser);

    for result in rule_list_parser {
        if let Err(e) = result {
            // For now, simpler error conversion.
            return Err(StylesheetError::Parse {
                path: None,
                message: format!("CSS Parse Error: {:?}", e),
            });
        }
    }

    build_variants(&css_parser.definitions)
}

struct StyleSheetParser {
    definitions: HashMap<String, StyleDefinition>,
    current_mode: Option<Mode>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Light,
    Dark,
}

impl<'i> QualifiedRuleParser<'i> for StyleSheetParser {
    type Prelude = Vec<String>;
    type QualifiedRule = ();
    type Error = ();

    fn parse_prelude<'t>(
        &mut self,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self::Prelude, ParseError<'i, Self::Error>> {
        let mut names = Vec::new();

        while let Ok(token) = input.next() {
            match token {
                Token::Delim('.') => {
                    let name = input.expect_ident()?;
                    names.push(name.as_ref().to_string());
                }
                Token::Comma | Token::WhiteSpace(_) => continue,
                _ => {
                    // Ignore other tokens
                }
            }
        }

        if names.is_empty() {
            return Err(input.new_custom_error::<(), ()>(()));
        }
        Ok(names)
    }

    fn parse_block<'t>(
        &mut self,
        prelude: Self::Prelude,
        _start: &ParserState,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self::QualifiedRule, ParseError<'i, Self::Error>> {
        let mut decl_parser = StyleDeclarationParser;
        let rule_parser = RuleBodyParser::new(input, &mut decl_parser);

        let mut attributes = StyleAttributes::new();

        for (_prop, val) in rule_parser.flatten() {
            if let Some(c) = val.fg {
                attributes.fg = Some(c);
            }
            if let Some(c) = val.bg {
                attributes.bg = Some(c);
            }
            if let Some(b) = val.bold {
                attributes.bold = Some(b);
            }
            if let Some(v) = val.dim {
                attributes.dim = Some(v);
            }
            if let Some(v) = val.italic {
                attributes.italic = Some(v);
            }
            if let Some(v) = val.underline {
                attributes.underline = Some(v);
            }
            if let Some(v) = val.blink {
                attributes.blink = Some(v);
            }
            if let Some(v) = val.reverse {
                attributes.reverse = Some(v);
            }
            if let Some(v) = val.hidden {
                attributes.hidden = Some(v);
            }
            if let Some(v) = val.strikethrough {
                attributes.strikethrough = Some(v);
            }
        }

        for name in prelude {
            let def = self
                .definitions
                .entry(name)
                .or_insert(StyleDefinition::Attributes {
                    base: StyleAttributes::new(),
                    light: None,
                    dark: None,
                });

            if let StyleDefinition::Attributes {
                ref mut base,
                ref mut light,
                ref mut dark,
            } = def
            {
                match self.current_mode {
                    None => *base = base.merge(&attributes),
                    Some(Mode::Light) => {
                        let l = light.get_or_insert(StyleAttributes::new());
                        *l = l.merge(&attributes);
                    }
                    Some(Mode::Dark) => {
                        let d = dark.get_or_insert(StyleAttributes::new());
                        *d = d.merge(&attributes);
                    }
                }
            }
        }
        Ok(())
    }
}

impl<'i> AtRuleParser<'i> for StyleSheetParser {
    type Prelude = Mode;
    type AtRule = ();
    type Error = ();

    fn parse_prelude<'t>(
        &mut self,
        name: CowRcStr<'i>,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self::Prelude, ParseError<'i, Self::Error>> {
        if name.as_ref() == "media" {
            // Peek and parse blocks
            let mut found_mode: Option<Mode> = None;

            loop {
                match input.next() {
                    Ok(Token::ParenthesisBlock) => {
                        // We consumed ParenthesisBlock. Now we can call parse_nested_block.
                        let nested_res = input.parse_nested_block(|input| {
                            input.expect_ident_matching("prefers-color-scheme")?;
                            input.expect_colon()?;
                            let val = input.expect_ident()?;
                            match val.as_ref() {
                                "dark" => Ok(Mode::Dark),
                                "light" => Ok(Mode::Light),
                                _ => Err(input.new_custom_error::<(), ()>(())),
                            }
                        });
                        if let Ok(m) = nested_res {
                            found_mode = Some(m);
                        }
                    }
                    Ok(Token::WhiteSpace(_)) | Ok(Token::Comment(_)) => continue,
                    Err(_) => break, // End of input
                    Ok(_) => {
                        // Ignore other tokens
                    }
                }
            }

            if let Some(m) = found_mode {
                return Ok(m);
            }

            Err(input.new_custom_error::<(), ()>(()))
        } else {
            Err(input.new_custom_error::<(), ()>(()))
        }
    }

    fn parse_block<'t>(
        &mut self,
        mode: Self::Prelude,
        _start: &ParserState,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self::AtRule, ParseError<'i, Self::Error>> {
        let old_mode = self.current_mode;
        self.current_mode = Some(mode);

        let list_parser = cssparser::StyleSheetParser::new(input, self);
        for _ in list_parser {}

        self.current_mode = old_mode;
        Ok(())
    }
}

struct StyleDeclarationParser;

impl<'i> DeclarationParser<'i> for StyleDeclarationParser {
    type Declaration = (String, StyleAttributes);
    type Error = ();

    fn parse_value<'t>(
        &mut self,
        name: CowRcStr<'i>,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self::Declaration, ParseError<'i, Self::Error>> {
        let mut attrs = StyleAttributes::new();
        match name.as_ref() {
            "fg" | "color" => {
                attrs.fg = Some(parse_color(input)?);
            }
            "bg" | "background" | "background-color" => {
                attrs.bg = Some(parse_color(input)?);
            }
            "bold" => {
                if parse_bool_or_flag(input)? {
                    attrs.bold = Some(true);
                }
            }
            "dim" => {
                if parse_bool_or_flag(input)? {
                    attrs.dim = Some(true);
                }
            }
            "italic" => {
                if parse_bool_or_flag(input)? {
                    attrs.italic = Some(true);
                }
            }
            "underline" => {
                if parse_bool_or_flag(input)? {
                    attrs.underline = Some(true);
                }
            }
            "blink" => {
                if parse_bool_or_flag(input)? {
                    attrs.blink = Some(true);
                }
            }
            "reverse" => {
                if parse_bool_or_flag(input)? {
                    attrs.reverse = Some(true);
                }
            }
            "hidden" => {
                if parse_bool_or_flag(input)? {
                    attrs.hidden = Some(true);
                }
            }
            "strikethrough" => {
                if parse_bool_or_flag(input)? {
                    attrs.strikethrough = Some(true);
                }
            }

            "font-weight" => {
                let val = input.expect_ident()?;
                if val.as_ref() == "bold" {
                    attrs.bold = Some(true);
                }
            }
            "font-style" => {
                let val = input.expect_ident()?;
                if val.as_ref() == "italic" {
                    attrs.italic = Some(true);
                }
            }
            "text-decoration" => {
                let val = input.expect_ident()?;
                match val.as_ref() {
                    "underline" => attrs.underline = Some(true),
                    "line-through" => attrs.strikethrough = Some(true),
                    _ => {}
                }
            }
            "visibility" => {
                let val = input.expect_ident()?;
                if val.as_ref() == "hidden" {
                    attrs.hidden = Some(true);
                }
            }

            _ => return Err(input.new_custom_error::<(), ()>(())),
        }
        Ok((name.as_ref().to_string(), attrs))
    }
}

impl<'i> AtRuleParser<'i> for StyleDeclarationParser {
    type Prelude = ();
    type AtRule = (String, StyleAttributes);
    type Error = ();
}

impl<'i> QualifiedRuleParser<'i> for StyleDeclarationParser {
    type Prelude = ();
    type QualifiedRule = (String, StyleAttributes);
    type Error = ();
}

impl<'i> RuleBodyItemParser<'i, (String, StyleAttributes), ()> for StyleDeclarationParser {
    fn parse_declarations(&self) -> bool {
        true
    }
    fn parse_qualified(&self) -> bool {
        false
    }
}

fn parse_color<'i, 't>(input: &mut Parser<'i, 't>) -> Result<ColorDef, ParseError<'i, ()>> {
    let token = match input.next() {
        Ok(t) => t,
        Err(_) => return Err(input.new_custom_error::<(), ()>(())),
    };

    match token {
        Token::Ident(name) => {
            ColorDef::parse_string(name.as_ref()).map_err(|_| input.new_custom_error::<(), ()>(()))
        }
        Token::Hash(val) | Token::IDHash(val) => ColorDef::parse_string(&format!("#{}", val))
            .map_err(|_| input.new_custom_error::<(), ()>(())),
        _ => Err(input.new_custom_error::<(), ()>(())),
    }
}

fn parse_bool_or_flag<'i, 't>(input: &mut Parser<'i, 't>) -> Result<bool, ParseError<'i, ()>> {
    match input.expect_ident() {
        Ok(val) => Ok(val.as_ref() == "true"),
        Err(_) => Err(input.new_custom_error::<(), ()>(())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ColorMode, StyleValue};

    #[test]
    fn test_parse_simple() {
        let css = ".error { color: red; font-weight: bold; }";
        let variants = parse_css(css).unwrap();
        let base = variants.base();

        // Ensure "error" style exists
        assert!(base.contains_key("error"));

        let style = base.get("error").unwrap();
        let styled = style.apply_to("text").to_string();
        // Check for red (31) and bold (1).
        assert!(styled.contains("\x1b[31m"));
        assert!(styled.contains("\x1b[1m"));
    }

    #[test]
    fn test_parse_adaptive() {
        let css =
            ".text { color: red; } @media (prefers-color-scheme: dark) { .text { color: white; } }";
        let variants = parse_css(css).unwrap();

        let light = variants.resolve(Some(ColorMode::Light));
        let dark = variants.resolve(Some(ColorMode::Dark));

        // Light (base) -> Red
        if let StyleValue::Concrete(s) = light.get("text").unwrap() {
            let out = s.apply_to("x").to_string();
            assert!(out.contains("\x1b[31m")); // Red
        } else {
            panic!("Expected Concrete style for light mode");
        }

        // Dark -> White
        if let StyleValue::Concrete(s) = dark.get("text").unwrap() {
            let out = s.apply_to("x").to_string();
            assert!(out.contains("\x1b[37m")); // White
        } else {
            panic!("Expected Concrete style for dark mode");
        }
    }

    #[test]
    fn test_multiple_selectors() {
        let css = ".a, .b { color: blue; }";
        let variants = parse_css(css).unwrap();
        let base = variants.base();
        assert!(base.contains_key("a"));
        assert!(base.contains_key("b"));
    }
}
