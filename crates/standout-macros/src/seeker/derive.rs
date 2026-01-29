//! Implementation of the `#[derive(Seekable)]` macro.
//!
//! This macro generates an implementation of the `Seekable` trait and
//! field name constants for type-safe query building.

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{spanned::Spanned, Data, DeriveInput, Error, Fields, Result};

use super::attrs::{parse_seek_attrs, SeekType};

/// Main implementation of the Seekable derive macro.
pub fn seekable_derive_impl(input: DeriveInput) -> Result<TokenStream> {
    let struct_name = &input.ident;

    // Ensure we have a struct with named fields
    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(named) => &named.named,
            _ => {
                return Err(Error::new(
                    input.span(),
                    "Seekable can only be derived for structs with named fields",
                ))
            }
        },
        _ => {
            return Err(Error::new(
                input.span(),
                "Seekable can only be derived for structs",
            ))
        }
    };

    // Collect field information
    let mut field_matches: Vec<TokenStream> = Vec::new();
    let mut field_constants: Vec<TokenStream> = Vec::new();

    for field in fields.iter() {
        let field_name = field
            .ident
            .as_ref()
            .ok_or_else(|| Error::new(field.span(), "expected named field"))?;

        // Parse field attributes
        let seek_attrs = parse_seek_attrs(&field.attrs)?;

        // Skip if marked with #[seek(skip)]
        if seek_attrs.skip {
            continue;
        }

        // If no seek type is specified, skip this field
        let seek_type = match seek_attrs.seek_type {
            Some(t) => t,
            None => continue,
        };

        // Determine the query field name
        let query_name = seek_attrs.rename.unwrap_or_else(|| field_name.to_string());

        // Generate constant name (SCREAMING_SNAKE_CASE)
        let const_name = format_ident!("{}", to_screaming_snake_case(&query_name));

        // Generate the constant
        field_constants.push(quote! {
            /// Field name constant for type-safe queries.
            pub const #const_name: &'static str = #query_name;
        });

        // Generate the match arm for seeker_field_value
        let value_expr = match seek_type {
            SeekType::String => {
                quote! { ::standout_seeker::Value::String(&self.#field_name) }
            }
            SeekType::Number => {
                quote! { ::standout_seeker::Value::Number(::standout_seeker::Number::from(self.#field_name)) }
            }
            SeekType::Timestamp => {
                quote! {
                    ::standout_seeker::Value::Timestamp(
                        ::standout_seeker::SeekerTimestamp::seeker_timestamp(&self.#field_name)
                    )
                }
            }
            SeekType::Enum => {
                quote! {
                    ::standout_seeker::Value::Enum(
                        ::standout_seeker::SeekerEnum::seeker_discriminant(&self.#field_name)
                    )
                }
            }
            SeekType::Bool => {
                quote! { ::standout_seeker::Value::Bool(self.#field_name) }
            }
        };

        field_matches.push(quote! {
            #query_name => #value_expr,
        });
    }

    // Generate the impl block
    let expanded = quote! {
        impl #struct_name {
            #(#field_constants)*
        }

        impl ::standout_seeker::Seekable for #struct_name {
            fn seeker_field_value(&self, field: &str) -> ::standout_seeker::Value<'_> {
                match field {
                    #(#field_matches)*
                    _ => ::standout_seeker::Value::None,
                }
            }
        }
    };

    Ok(expanded)
}

/// Convert a string to SCREAMING_SNAKE_CASE.
fn to_screaming_snake_case(s: &str) -> String {
    let mut result = String::with_capacity(s.len() + 4);
    let mut prev_was_lower = false;

    for c in s.chars() {
        if c.is_uppercase() {
            if prev_was_lower {
                result.push('_');
            }
            result.push(c);
            prev_was_lower = false;
        } else if c == '_' || c == '-' {
            result.push('_');
            prev_was_lower = false;
        } else {
            result.push(c.to_ascii_uppercase());
            prev_was_lower = true;
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_screaming_snake_case() {
        assert_eq!(to_screaming_snake_case("name"), "NAME");
        assert_eq!(to_screaming_snake_case("created_at"), "CREATED_AT");
        assert_eq!(to_screaming_snake_case("createdAt"), "CREATED_AT");
        assert_eq!(to_screaming_snake_case("my-field"), "MY_FIELD");
        assert_eq!(to_screaming_snake_case("XMLParser"), "XMLPARSER");
    }
}
