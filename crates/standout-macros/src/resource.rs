//! Derive macro for Resource operations.
//!
//! This module provides the [`Resource`] derive macro that generates CLI commands
//! and handlers for Create, Read, Update, Delete operations on structs.
//!
//! # Example
//!
//! ```rust,ignore
//! use standout_macros::Resource;
//!
//! #[derive(Clone, Resource)]
//! #[resource(object = "task", store = TaskStore)]
//! pub struct Task {
//!     #[resource(id)]
//!     pub id: String,
//!
//!     #[resource(arg(short, long), form(required))]
//!     pub title: String,
//!
//!     #[resource(arg(short, long), choices = ["pending", "done"])]
//!     pub status: String,
//!
//!     #[resource(readonly)]
//!     pub created_at: DateTime<Utc>,
//! }
//! ```
//!
//! This generates:
//! - `TaskCommands` enum with List, View, Create, Update, Delete variants
//! - Handler module `__task_resource_handlers` with implementations
//! - Dispatch configuration via `TaskCommands::dispatch_config()`

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
    spanned::Spanned,
    Data, DeriveInput, Error, Expr, Fields, Ident, Meta, Result, Token, Type,
};

/// Container-level attributes for #[resource(...)]
#[derive(Default)]
struct ResourceContainerAttrs {
    /// Required: singular object name (e.g., "task")
    object: Option<String>,
    /// Required: store type implementing ResourceStore
    store: Option<syn::Path>,
    /// Optional: plural name (defaults to "{object}s")
    plural: Option<String>,
    /// Optional: subset of operations to generate
    operations: Option<Vec<ResourceOperation>>,
    /// Optional: enable validify integration for validation/modification
    validify: bool,
    /// Optional: default subcommand when none specified (e.g., "list")
    default_command: Option<String>,
    /// Optional: command name aliases (e.g., view -> show, delete -> rm)
    aliases: std::collections::HashMap<String, String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ResourceOperation {
    List,
    View,
    Create,
    Update,
    Delete,
}

impl ResourceOperation {
    fn all() -> Vec<Self> {
        vec![
            Self::List,
            Self::View,
            Self::Create,
            Self::Update,
            Self::Delete,
        ]
    }

    fn from_str(s: &str) -> Option<Self> {
        match s {
            "list" => Some(Self::List),
            "view" => Some(Self::View),
            "create" => Some(Self::Create),
            "update" => Some(Self::Update),
            "delete" => Some(Self::Delete),
            _ => None,
        }
    }
}

/// Field-level attributes for #[resource(...)]
#[derive(Default)]
struct ResourceFieldAttrs {
    /// This field is the primary identifier
    id: bool,
    /// Exclude from create/update operations
    readonly: bool,
    /// Exclude from all Resource operations
    skip: bool,
    /// Default value expression for create
    default_expr: Option<String>,
    /// Constrained values for this field
    choices: Option<Vec<String>>,
    /// Field type is an enum that implements ValueEnum
    value_enum: bool,
    /// Help text extracted from doc comments
    doc: Option<String>,
    /// Short option character (e.g., 't' for -t)
    short: Option<char>,
    /// Custom long option name (overrides field name)
    long: Option<String>,
}

/// Information about a field for Resource operations
struct ResourceFieldInfo {
    ident: Ident,
    ty: Type,
    attrs: ResourceFieldAttrs,
}

/// Categorizes field types for code generation
#[derive(Clone)]
#[allow(dead_code)] // Enum variant reserved for future use
enum TypeKind {
    /// Simple scalar type (String, i32, etc.)
    Scalar(Type),
    /// Optional type (Option<T>)
    Option(Type),
    /// Collection type (Vec<T>)
    Vec(Type),
    /// Enum type that should use ValueEnum
    Enum(Type),
}

impl TypeKind {
    /// Analyzes a type and returns its kind
    fn from_type(ty: &Type) -> Self {
        if let Type::Path(type_path) = ty {
            if let Some(segment) = type_path.path.segments.last() {
                let ident_str = segment.ident.to_string();

                // Check for Vec<T>
                if ident_str == "Vec" {
                    if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                        if let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first() {
                            return TypeKind::Vec(inner_ty.clone());
                        }
                    }
                }

                // Check for Option<T>
                if ident_str == "Option" {
                    if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                        if let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first() {
                            return TypeKind::Option(inner_ty.clone());
                        }
                    }
                }
            }
        }

        // Default to scalar
        TypeKind::Scalar(ty.clone())
    }

    /// Returns the inner type (for display and extraction)
    fn inner_type(&self) -> &Type {
        match self {
            TypeKind::Scalar(ty) => ty,
            TypeKind::Option(ty) => ty,
            TypeKind::Vec(ty) => ty,
            TypeKind::Enum(ty) => ty,
        }
    }
}

impl Parse for ResourceContainerAttrs {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut attrs = ResourceContainerAttrs::default();

        let content: Punctuated<Meta, Token![,]> = Punctuated::parse_terminated(input)?;

        for meta in content {
            match &meta {
                Meta::NameValue(nv) if nv.path.is_ident("object") => {
                    if let Expr::Lit(expr_lit) = &nv.value {
                        if let syn::Lit::Str(lit_str) = &expr_lit.lit {
                            attrs.object = Some(lit_str.value());
                        } else {
                            return Err(Error::new(nv.value.span(), "expected string literal"));
                        }
                    } else {
                        return Err(Error::new(nv.value.span(), "expected string literal"));
                    }
                }
                Meta::NameValue(nv) if nv.path.is_ident("store") => {
                    if let Expr::Path(expr_path) = &nv.value {
                        attrs.store = Some(expr_path.path.clone());
                    } else {
                        return Err(Error::new(nv.value.span(), "expected path"));
                    }
                }
                Meta::NameValue(nv) if nv.path.is_ident("plural") => {
                    if let Expr::Lit(expr_lit) = &nv.value {
                        if let syn::Lit::Str(lit_str) = &expr_lit.lit {
                            attrs.plural = Some(lit_str.value());
                        } else {
                            return Err(Error::new(nv.value.span(), "expected string literal"));
                        }
                    } else {
                        return Err(Error::new(nv.value.span(), "expected string literal"));
                    }
                }
                Meta::List(list) if list.path.is_ident("operations") => {
                    let mut ops = Vec::new();
                    // Parse the inner tokens as comma-separated identifiers
                    let inner: Punctuated<Ident, Token![,]> =
                        list.parse_args_with(Punctuated::parse_terminated)?;
                    for ident in inner {
                        if let Some(op) = ResourceOperation::from_str(&ident.to_string()) {
                            ops.push(op);
                        } else {
                            return Err(Error::new(
                                ident.span(),
                                format!("unknown operation '{}', expected one of: list, view, create, update, delete", ident),
                            ));
                        }
                    }
                    attrs.operations = Some(ops);
                }
                Meta::Path(p) if p.is_ident("validify") => {
                    attrs.validify = true;
                }
                Meta::NameValue(nv) if nv.path.is_ident("default") => {
                    if let Expr::Lit(expr_lit) = &nv.value {
                        if let syn::Lit::Str(lit_str) = &expr_lit.lit {
                            attrs.default_command = Some(lit_str.value());
                        } else {
                            return Err(Error::new(nv.value.span(), "expected string literal"));
                        }
                    } else {
                        return Err(Error::new(nv.value.span(), "expected string literal"));
                    }
                }
                Meta::List(list) if list.path.is_ident("aliases") => {
                    // Parse aliases(view = "show", delete = "rm")
                    let inner: Punctuated<Meta, Token![,]> =
                        list.parse_args_with(Punctuated::parse_terminated)?;
                    for alias_meta in inner {
                        if let Meta::NameValue(nv) = alias_meta {
                            let cmd_name =
                                nv.path.get_ident().map(|i| i.to_string()).ok_or_else(|| {
                                    Error::new(nv.path.span(), "expected command name")
                                })?;

                            // Validate that it's a valid command name
                            if ResourceOperation::from_str(&cmd_name).is_none() {
                                return Err(Error::new(
                                    nv.path.span(),
                                    format!("unknown command '{}', expected one of: list, view, create, update, delete", cmd_name),
                                ));
                            }

                            if let Expr::Lit(expr_lit) = &nv.value {
                                if let syn::Lit::Str(lit_str) = &expr_lit.lit {
                                    attrs.aliases.insert(cmd_name, lit_str.value());
                                } else {
                                    return Err(Error::new(
                                        nv.value.span(),
                                        "expected string literal for alias",
                                    ));
                                }
                            } else {
                                return Err(Error::new(
                                    nv.value.span(),
                                    "expected string literal for alias",
                                ));
                            }
                        } else {
                            return Err(Error::new(
                                alias_meta.span(),
                                "expected command = \"alias\" format",
                            ));
                        }
                    }
                }
                _ => {
                    return Err(Error::new(
                        meta.span(),
                        "unknown attribute, expected one of: object, store, plural, operations, validify, default, aliases",
                    ));
                }
            }
        }

        Ok(attrs)
    }
}

/// Parse field-level #[resource(...)] attributes
fn parse_field_attrs(attrs: &[syn::Attribute]) -> Result<ResourceFieldAttrs> {
    let mut field_attrs = ResourceFieldAttrs::default();
    let mut doc_lines: Vec<String> = Vec::new();

    for attr in attrs {
        // Extract doc comments (/// becomes #[doc = "..."])
        if attr.path().is_ident("doc") {
            if let Meta::NameValue(nv) = &attr.meta {
                if let Expr::Lit(expr_lit) = &nv.value {
                    if let syn::Lit::Str(lit_str) = &expr_lit.lit {
                        doc_lines.push(lit_str.value().trim().to_string());
                    }
                }
            }
        }

        if attr.path().is_ident("resource") {
            let nested: Punctuated<Meta, Token![,]> =
                attr.parse_args_with(Punctuated::parse_terminated)?;

            for meta in nested {
                match &meta {
                    Meta::Path(p) if p.is_ident("id") => {
                        field_attrs.id = true;
                    }
                    Meta::Path(p) if p.is_ident("readonly") => {
                        field_attrs.readonly = true;
                    }
                    Meta::Path(p) if p.is_ident("skip") => {
                        field_attrs.skip = true;
                    }
                    Meta::Path(p) if p.is_ident("value_enum") => {
                        field_attrs.value_enum = true;
                    }
                    Meta::NameValue(nv) if nv.path.is_ident("default") => {
                        if let Expr::Lit(expr_lit) = &nv.value {
                            if let syn::Lit::Str(lit_str) = &expr_lit.lit {
                                field_attrs.default_expr = Some(lit_str.value());
                            }
                        }
                    }
                    Meta::List(list) if list.path.is_ident("choices") => {
                        let inner: Punctuated<syn::LitStr, Token![,]> =
                            list.parse_args_with(Punctuated::parse_terminated)?;
                        let choices: Vec<String> = inner.iter().map(|l| l.value()).collect();
                        field_attrs.choices = Some(choices);
                    }
                    Meta::List(list) if list.path.is_ident("arg") => {
                        // Parse arg(short = 'x', long = "name")
                        let inner: Punctuated<Meta, Token![,]> =
                            list.parse_args_with(Punctuated::parse_terminated)?;
                        for arg_meta in inner {
                            if let Meta::NameValue(nv) = arg_meta {
                                if nv.path.is_ident("short") {
                                    if let Expr::Lit(expr_lit) = &nv.value {
                                        if let syn::Lit::Char(lit_char) = &expr_lit.lit {
                                            field_attrs.short = Some(lit_char.value());
                                        }
                                    }
                                } else if nv.path.is_ident("long") {
                                    if let Expr::Lit(expr_lit) = &nv.value {
                                        if let syn::Lit::Str(lit_str) = &expr_lit.lit {
                                            field_attrs.long = Some(lit_str.value());
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Meta::List(list) if list.path.is_ident("form") => {}
                    Meta::List(list) if list.path.is_ident("validate") => {}
                    _ => {
                        // Ignore unrecognized attributes for forward compatibility
                    }
                }
            }
        }
    }

    // Concatenate doc lines with spaces
    if !doc_lines.is_empty() {
        field_attrs.doc = Some(doc_lines.join(" "));
    }

    Ok(field_attrs)
}

/// Parse container-level #[resource(...)] attributes
fn parse_container_attrs(input: &DeriveInput) -> Result<ResourceContainerAttrs> {
    for attr in &input.attrs {
        if attr.path().is_ident("resource") {
            return attr.parse_args::<ResourceContainerAttrs>();
        }
    }

    Err(Error::new(
        input.span(),
        "missing `#[resource(object = \"...\", store = ...)]` attribute",
    ))
}

/// Extract field information from struct
fn extract_fields(fields: &Fields) -> Result<Vec<ResourceFieldInfo>> {
    let named_fields = match fields {
        Fields::Named(named) => &named.named,
        _ => {
            return Err(Error::new(
                fields.span(),
                "Resource can only be derived for structs with named fields",
            ));
        }
    };

    let mut result = Vec::new();
    for field in named_fields {
        let ident = field
            .ident
            .clone()
            .ok_or_else(|| Error::new(field.span(), "expected named field"))?;
        let attrs = parse_field_attrs(&field.attrs)?;

        result.push(ResourceFieldInfo {
            ident,
            ty: field.ty.clone(),
            attrs,
        });
    }

    Ok(result)
}

/// Main implementation of the Resource derive macro
pub fn resource_derive_impl(input: DeriveInput) -> Result<TokenStream> {
    let container_attrs = parse_container_attrs(&input)?;

    let object_name = container_attrs
        .object
        .ok_or_else(|| Error::new(input.span(), "missing `object` in #[resource(...)]"))?;

    let store_type = container_attrs
        .store
        .ok_or_else(|| Error::new(input.span(), "missing `store` in #[resource(...)]"))?;

    let _plural_name = container_attrs
        .plural
        .unwrap_or_else(|| format!("{}s", object_name));

    let operations = container_attrs
        .operations
        .unwrap_or_else(ResourceOperation::all);

    let use_validify = container_attrs.validify;
    let default_command = container_attrs.default_command;
    let aliases = container_attrs.aliases;

    // Helper to get command name (alias or default)
    let get_cmd_name = |default: &str| -> String {
        aliases
            .get(default)
            .cloned()
            .unwrap_or_else(|| default.to_string())
    };

    let struct_name = &input.ident;

    // Extract struct fields
    let fields = match &input.data {
        Data::Struct(data) => extract_fields(&data.fields)?,
        _ => {
            return Err(Error::new(
                input.span(),
                "Resource can only be derived for structs",
            ));
        }
    };

    // Find the ID field
    let _id_field = fields.iter().find(|f| f.attrs.id).ok_or_else(|| {
        Error::new(
            input.span(),
            "no field marked with #[resource(id)] - one field must be the identifier",
        )
    })?;

    // Generate names
    let commands_enum_name = format_ident!("{}Commands", struct_name);
    let handlers_module_name = format_ident!("__{}_resource_handlers", object_name);
    let object_name_upper = {
        let mut chars = object_name.chars();
        chars
            .next()
            .map(|c| c.to_uppercase().collect::<String>())
            .unwrap_or_default()
            + chars.as_str()
    };

    // Fields for create/update (excluding id, readonly, and skip)
    let mutable_fields: Vec<&ResourceFieldInfo> = fields
        .iter()
        .filter(|f| !f.attrs.id && !f.attrs.readonly && !f.attrs.skip)
        .collect();

    // Helper function to generate clap args based on type
    #[allow(clippy::too_many_arguments)]
    fn generate_arg(
        name: &Ident,
        ty: &Type,
        long_name: &str,
        choices: &Option<Vec<String>>,
        is_value_enum: bool,
        doc: &Option<String>,
        default_expr: &Option<String>,
        short: &Option<char>,
    ) -> TokenStream {
        let type_kind = TypeKind::from_type(ty);

        // Generate help attribute if doc comment exists
        let help_attr = doc
            .as_ref()
            .map(|d| quote! { help = #d, })
            .unwrap_or_default();

        // Generate default_value attribute for display in help
        let default_attr = default_expr
            .as_ref()
            .map(|d| quote! { default_value = #d, })
            .unwrap_or_default();

        // Generate short option attribute
        let short_attr = short
            .as_ref()
            .map(|s| quote! { short = #s, })
            .unwrap_or_default();

        // Handle explicit choices (string-based)
        if let Some(choice_values) = choices {
            let choice_values: Vec<&String> = choice_values.iter().collect();
            return quote! {
                #[arg(long = #long_name, #short_attr #help_attr #default_attr value_parser = clap::builder::PossibleValuesParser::new([#(#choice_values),*]))]
                pub #name: Option<String>,
            };
        }

        // Handle value_enum types
        if is_value_enum {
            let inner = type_kind.inner_type();
            return quote! {
                #[arg(long = #long_name, #short_attr #help_attr #default_attr value_enum)]
                pub #name: Option<#inner>,
            };
        }

        match type_kind {
            TypeKind::Vec(inner_ty) => {
                // Vec<T> -> multi-value arg
                quote! {
                    #[arg(long = #long_name, #short_attr #help_attr num_args = 0..)]
                    pub #name: Vec<#inner_ty>,
                }
            }
            TypeKind::Option(inner_ty) => {
                // Option<T> -> optional arg (already optional)
                quote! {
                    #[arg(long = #long_name, #short_attr #help_attr #default_attr)]
                    pub #name: Option<#inner_ty>,
                }
            }
            TypeKind::Scalar(scalar_ty) | TypeKind::Enum(scalar_ty) => {
                // Scalar -> wrap in Option for CLI
                quote! {
                    #[arg(long = #long_name, #short_attr #help_attr #default_attr)]
                    pub #name: Option<#scalar_ty>,
                }
            }
        }
    }

    // Helper function to generate JSON extraction based on type
    fn generate_json_extraction(
        name: &Ident,
        ty: &Type,
        long_name: &str,
        choices: &Option<Vec<String>>,
        is_value_enum: bool,
        include_changed: bool,
    ) -> TokenStream {
        let name_str = name.to_string();
        let type_kind = TypeKind::from_type(ty);

        // Handle explicit choices (always String)
        if choices.is_some() {
            return if include_changed {
                quote! {
                    if let Some(val) = matches.get_one::<String>(#long_name) {
                        __data[#name_str] = ::serde_json::json!(val);
                        __changed.push(#name_str.to_string());
                    }
                }
            } else {
                quote! {
                    if let Some(val) = matches.get_one::<String>(#long_name) {
                        __data[#name_str] = ::serde_json::json!(val);
                    }
                }
            };
        }

        // Handle value_enum types
        if is_value_enum {
            let inner = type_kind.inner_type();
            return if include_changed {
                quote! {
                    if let Some(val) = matches.get_one::<#inner>(#long_name) {
                        __data[#name_str] = ::serde_json::json!(val);
                        __changed.push(#name_str.to_string());
                    }
                }
            } else {
                quote! {
                    if let Some(val) = matches.get_one::<#inner>(#long_name) {
                        __data[#name_str] = ::serde_json::json!(val);
                    }
                }
            };
        }

        match type_kind {
            TypeKind::Vec(inner_ty) => {
                // Vec<T> -> get_many
                if include_changed {
                    quote! {
                        let __vec_vals: Vec<#inner_ty> = matches.get_many::<#inner_ty>(#long_name)
                            .map(|v| v.cloned().collect())
                            .unwrap_or_default();
                        if !__vec_vals.is_empty() {
                            __data[#name_str] = ::serde_json::json!(__vec_vals);
                            __changed.push(#name_str.to_string());
                        }
                    }
                } else {
                    quote! {
                        let __vec_vals: Vec<#inner_ty> = matches.get_many::<#inner_ty>(#long_name)
                            .map(|v| v.cloned().collect())
                            .unwrap_or_default();
                        if !__vec_vals.is_empty() {
                            __data[#name_str] = ::serde_json::json!(__vec_vals);
                        }
                    }
                }
            }
            TypeKind::Option(inner_ty) => {
                // Option<T> -> get_one
                if include_changed {
                    quote! {
                        if let Some(val) = matches.get_one::<#inner_ty>(#long_name) {
                            __data[#name_str] = ::serde_json::json!(val);
                            __changed.push(#name_str.to_string());
                        }
                    }
                } else {
                    quote! {
                        if let Some(val) = matches.get_one::<#inner_ty>(#long_name) {
                            __data[#name_str] = ::serde_json::json!(val);
                        }
                    }
                }
            }
            TypeKind::Scalar(scalar_ty) | TypeKind::Enum(scalar_ty) => {
                // Scalar -> get_one
                if include_changed {
                    quote! {
                        if let Some(val) = matches.get_one::<#scalar_ty>(#long_name) {
                            __data[#name_str] = ::serde_json::json!(val);
                            __changed.push(#name_str.to_string());
                        }
                    }
                } else {
                    quote! {
                        if let Some(val) = matches.get_one::<#scalar_ty>(#long_name) {
                            __data[#name_str] = ::serde_json::json!(val);
                        }
                    }
                }
            }
        }
    }

    // Generate clap args for create command
    let create_args: Vec<TokenStream> = mutable_fields
        .iter()
        .map(|f| {
            let name = &f.ident;
            let ty = &f.ty;
            let name_str = name.to_string();
            // Use custom long name if provided, otherwise derive from field name
            let long_name = f
                .attrs
                .long
                .clone()
                .unwrap_or_else(|| name_str.replace('_', "-"));
            generate_arg(
                name,
                ty,
                &long_name,
                &f.attrs.choices,
                f.attrs.value_enum,
                &f.attrs.doc,
                &f.attrs.default_expr,
                &f.attrs.short,
            )
        })
        .collect();

    // Generate clap args for update command (all optional, no defaults)
    let update_args: Vec<TokenStream> = mutable_fields
        .iter()
        .map(|f| {
            let name = &f.ident;
            let ty = &f.ty;
            let name_str = name.to_string();
            // Use custom long name if provided, otherwise derive from field name
            let long_name = f
                .attrs
                .long
                .clone()
                .unwrap_or_else(|| name_str.replace('_', "-"));
            generate_arg(
                name,
                ty,
                &long_name,
                &f.attrs.choices,
                f.attrs.value_enum,
                &f.attrs.doc,
                &None, // No defaults for update - user is changing existing values
                &f.attrs.short,
            )
        })
        .collect();

    // Generate JSON field builders for create handler
    let create_json_fields: Vec<TokenStream> = mutable_fields
        .iter()
        .map(|f| {
            let name = &f.ident;
            let ty = &f.ty;
            let name_str = name.to_string();
            // Use custom long name if provided for CLI arg matching
            let long_name = f
                .attrs
                .long
                .clone()
                .unwrap_or_else(|| name_str.replace('_', "-"));
            generate_json_extraction(
                name,
                ty,
                &long_name,
                &f.attrs.choices,
                f.attrs.value_enum,
                false,
            )
        })
        .collect();

    // Generate JSON field builders for update handler
    let update_json_fields: Vec<TokenStream> = mutable_fields
        .iter()
        .map(|f| {
            let name = &f.ident;
            let ty = &f.ty;
            let name_str = name.to_string();
            // Use custom long name if provided for CLI arg matching
            let long_name = f
                .attrs
                .long
                .clone()
                .unwrap_or_else(|| name_str.replace('_', "-"));
            generate_json_extraction(
                name,
                ty,
                &long_name,
                &f.attrs.choices,
                f.attrs.value_enum,
                true,
            )
        })
        .collect();

    // Generate default value injections for create handler
    let create_default_injections: Vec<TokenStream> = mutable_fields
        .iter()
        .filter_map(|f| {
            f.attrs.default_expr.as_ref().map(|default_val| {
                let name_str = f.ident.to_string();
                quote! {
                    if __data.get(#name_str).is_none() {
                        __data[#name_str] = ::serde_json::json!(#default_val);
                    }
                }
            })
        })
        .collect();

    // Generate command enum variants based on selected operations
    let mut command_variants = Vec::new();
    let mut dispatch_commands = Vec::new();

    if operations.contains(&ResourceOperation::List) {
        let cmd_name = get_cmd_name("list");
        command_variants.push(quote! {
            /// List all items
            #[command(name = #cmd_name)]
            List {
                #[arg(long)]
                filter: Option<String>,
                #[arg(long)]
                sort: Option<String>,
                #[arg(long)]
                limit: Option<usize>,
            },
        });
        dispatch_commands.push(quote! {
            let __builder = __builder.command_with(
                #cmd_name,
                #handlers_module_name::list,
                |cfg| cfg.template("standout/list-view")
            );
        });
    }

    if operations.contains(&ResourceOperation::View) {
        let cmd_name = get_cmd_name("view");
        command_variants.push(quote! {
            /// View one or more items
            #[command(name = #cmd_name)]
            View {
                /// The ID(s) of the item(s) to view
                #[arg(num_args = 1..)]
                ids: Vec<String>,
            },
        });
        dispatch_commands.push(quote! {
            let __builder = __builder.command_with(
                #cmd_name,
                #handlers_module_name::view,
                |cfg| cfg.template("standout/detail-view")
            );
        });
    }

    if operations.contains(&ResourceOperation::Create) {
        let cmd_name = get_cmd_name("create");
        command_variants.push(quote! {
            /// Create a new item
            #[command(name = #cmd_name)]
            Create {
                #(#create_args)*
                #[arg(long)]
                dry_run: bool,
            },
        });
        dispatch_commands.push(quote! {
            let __builder = __builder.command_with(
                #cmd_name,
                #handlers_module_name::create,
                |cfg| cfg.template("standout/create-view")
            );
        });
    }

    if operations.contains(&ResourceOperation::Update) {
        let cmd_name = get_cmd_name("update");
        command_variants.push(quote! {
            /// Update an existing item
            #[command(name = #cmd_name)]
            Update {
                /// The ID of the item to update
                id: String,
                #(#update_args)*
                #[arg(long)]
                dry_run: bool,
            },
        });
        dispatch_commands.push(quote! {
            let __builder = __builder.command_with(
                #cmd_name,
                #handlers_module_name::update,
                |cfg| cfg.template("standout/update-view")
            );
        });
    }

    if operations.contains(&ResourceOperation::Delete) {
        let cmd_name = get_cmd_name("delete");
        command_variants.push(quote! {
            /// Delete one or more items
            #[command(name = #cmd_name)]
            Delete {
                /// The ID(s) of the item(s) to delete
                #[arg(num_args = 1..)]
                ids: Vec<String>,
                #[arg(long)]
                confirm: bool,
                #[arg(long)]
                force: bool,
            },
        });
        dispatch_commands.push(quote! {
            let __builder = __builder.command_with(
                #cmd_name,
                #handlers_module_name::delete,
                |cfg| cfg.template("standout/delete-view")
            );
        });
    }

    // Generate handler implementations
    let list_handler = if operations.contains(&ResourceOperation::List) {
        quote! {
            pub fn list(
                matches: &::clap::ArgMatches,
                ctx: &::standout::cli::CommandContext,
            ) -> ::standout::cli::HandlerResult<::standout::views::ListViewResult<#struct_name>> {
                let store = ctx.app_state.get_required::<#store_type>()?;

                // ── Stage 1: Build Query ──
                let query = {
                    let filter = matches.get_one::<String>("filter").cloned();
                    let sort = matches.get_one::<String>("sort").cloned();
                    let limit = matches.get_one::<usize>("limit").cloned();

                    if filter.is_some() || sort.is_some() || limit.is_some() {
                        let mut q = ::standout::cli::ResourceQuery::new();
                        if let Some(f) = filter { q = q.filter(f); }
                        if let Some(s) = sort { q = q.sort(s); }
                        if let Some(l) = limit { q = q.limit(l); }
                        Some(q)
                    } else {
                        None
                    }
                };

                // ── Stage 2: Validation (identity) ──
                let query = ::standout::cli::validate_identity(query)?;

                // ── Stage 3: Data Fetch ──
                let items = store.list(query.as_ref())?;
                let total = items.len();

                // ── Stage 4: App Logic (identity) ──
                let items = ::standout::cli::app_logic_identity(items)?;

                // ── Stage 5: View Building ──
                Ok(::standout::cli::Output::Render(
                    ::standout::views::list_view(items)
                        .total_count(total)
                        .tabular_spec(<#struct_name as ::standout::tabular::Tabular>::tabular_spec())
                        .build()
                ))
            }
        }
    } else {
        quote! {}
    };

    let view_handler = if operations.contains(&ResourceOperation::View) {
        quote! {
            pub fn view(
                matches: &::clap::ArgMatches,
                ctx: &::standout::cli::CommandContext,
            ) -> ::standout::cli::HandlerResult<::serde_json::Value> {
                let store = ctx.app_state.get_required::<#store_type>()?;

                // ── Stage 1: Get IDs ──
                let id_strs: Vec<String> = matches.get_many::<String>("ids")
                    .map(|v| v.cloned().collect())
                    .unwrap_or_default();

                if id_strs.len() == 1 {
                    // Single ID - use DetailViewResult for backwards compatibility
                    let id_str = &id_strs[0];
                    let id = store.parse_id(id_str)
                        .map_err(|e| ::standout::cli::IdResolutionError::parse_failed(id_str, e.to_string()))?;

                    let item = store.resolve(&id)
                        .map_err(|_| ::standout::cli::IdResolutionError::not_found(id_str))?;

                    let item = ::standout::cli::validate_identity(item)?;
                    let item = ::standout::cli::app_logic_identity(item)?;

                    let result = ::standout::views::detail_view(item)
                        .title(#object_name_upper)
                        .subtitle(id_str)
                        .action("Update", format!("{} update {}", #object_name, id_str))
                        .action("Delete", format!("{} delete {}", #object_name, id_str))
                        .build();
                    Ok(::standout::cli::Output::Render(
                        ::serde_json::to_value(result).unwrap_or_default()
                    ))
                } else {
                    // Multiple IDs - collect items and use ListViewResult
                    let mut items: Vec<#struct_name> = Vec::new();
                    let mut errors: Vec<String> = Vec::new();

                    for id_str in &id_strs {
                        match store.parse_id(id_str) {
                            Ok(id) => {
                                match store.resolve(&id) {
                                    Ok(item) => items.push(item),
                                    Err(_) => errors.push(format!("'{}' not found", id_str)),
                                }
                            }
                            Err(e) => errors.push(format!("Invalid ID '{}': {}", id_str, e)),
                        }
                    }

                    let total = items.len();
                    let mut result = ::standout::views::list_view(items)
                        .total_count(total)
                        .tabular_spec(<#struct_name as ::standout::tabular::Tabular>::tabular_spec())
                        .build();

                    // Add errors as warnings if any
                    for err in errors {
                        result = result.warning(err);
                    }

                    Ok(::standout::cli::Output::Render(
                        ::serde_json::to_value(result).unwrap_or_default()
                    ))
                }
            }
        }
    } else {
        quote! {}
    };

    // Generate validation stage based on whether validify is enabled
    let create_validation_stage = if use_validify {
        quote! {
            // ── Stage 2: Validation (validify) ──
            // Deserialize, apply modifiers, and validate
            let mut __item: #struct_name = ::serde_json::from_value(__data)
                .map_err(|e| ::standout::cli::ValidationError::general(format!("Invalid data: {}", e)))?;
            __item.validify()
                .map_err(|e| ::standout::cli::ValidationError::general(e.to_string()))?;
            let __data = ::serde_json::to_value(&__item)
                .map_err(|e| ::standout::cli::ValidationError::general(format!("Serialization failed: {}", e)))?;
        }
    } else {
        quote! {
            // ── Stage 2: Validation (identity) ──
            let __data = ::standout::cli::validate_identity(__data)?;
        }
    };

    let create_handler = if operations.contains(&ResourceOperation::Create) {
        quote! {
            pub fn create(
                matches: &::clap::ArgMatches,
                ctx: &::standout::cli::CommandContext,
            ) -> ::standout::cli::HandlerResult<::standout::views::CreateViewResult<#struct_name>> {
                let store = ctx.app_state.get_required::<#store_type>()?;
                let dry_run = matches.get_flag("dry_run");

                // ── Stage 1: Build Data ──
                let mut __data = ::serde_json::json!({});
                #(#create_json_fields)*

                // Inject defaults for missing fields
                #(#create_default_injections)*

                #create_validation_stage

                if dry_run {
                    // For dry-run, try to deserialize to show what would be created
                    match ::serde_json::from_value::<#struct_name>(__data) {
                        Ok(preview) => {
                            // ── Stage 3: App Logic (identity) ──
                            let preview = ::standout::cli::app_logic_identity(preview)?;

                            // ── Stage 4: View Building ──
                            Ok(::standout::cli::Output::Render(
                                ::standout::views::create_view(preview)
                                    .dry_run()
                                    .info("Dry run - no changes made")
                                    .build()
                            ))
                        }
                        Err(e) => {
                            Err(::standout::cli::ValidationError::general(format!("Invalid data: {}", e)).into())
                        }
                    }
                } else {
                    // ── Stage 3: Data Store ──
                    let item = store.create(__data)?;

                    // ── Stage 4: App Logic (identity) ──
                    let item = ::standout::cli::app_logic_identity(item)?;

                    // ── Stage 5: View Building ──
                    Ok(::standout::cli::Output::Render(
                        ::standout::views::create_view(item)
                            .success(format!("{} created", #object_name_upper))
                            .build()
                    ))
                }
            }
        }
    } else {
        quote! {}
    };

    // Generate update validation stage based on whether validify is enabled
    let update_validation_stage = if use_validify {
        quote! {
            // ── Stage 4: Validation (validify) ──
            // Merge update data with existing item, validate, then use partial update
            let mut __merged = ::serde_json::to_value(&before)
                .map_err(|e| ::standout::cli::ValidationError::general(format!("Serialization failed: {}", e)))?;
            if let (Some(merged_obj), Some(data_obj)) = (__merged.as_object_mut(), __data.as_object()) {
                for (k, v) in data_obj {
                    merged_obj.insert(k.clone(), v.clone());
                }
            }
            let mut __merged_item: #struct_name = ::serde_json::from_value(__merged)
                .map_err(|e| ::standout::cli::ValidationError::general(format!("Invalid data: {}", e)))?;
            __merged_item.validify()
                .map_err(|e| ::standout::cli::ValidationError::general(e.to_string()))?;
            // Keep __data as the partial update for the store
        }
    } else {
        quote! {
            // ── Stage 4: Validation (identity) ──
            let __data = ::standout::cli::validate_identity(__data)?;
        }
    };

    let update_handler = if operations.contains(&ResourceOperation::Update) {
        quote! {
            pub fn update(
                matches: &::clap::ArgMatches,
                ctx: &::standout::cli::CommandContext,
            ) -> ::standout::cli::HandlerResult<::standout::views::UpdateViewResult<#struct_name>> {
                let store = ctx.app_state.get_required::<#store_type>()?;
                let dry_run = matches.get_flag("dry_run");

                // ── Stage 1: ID Resolution ──
                let id_str = matches.get_one::<String>("id").unwrap();
                let id = store.parse_id(id_str)
                    .map_err(|e| ::standout::cli::IdResolutionError::parse_failed(id_str, e.to_string()))?;

                // ── Stage 2: Data Fetch (get current state) ──
                let before = store.resolve(&id)
                    .map_err(|_| ::standout::cli::IdResolutionError::not_found(id_str))?;

                // ── Stage 3: Build Update Data ──
                let mut __data = ::serde_json::json!({});
                let mut __changed: Vec<String> = Vec::new();
                #(#update_json_fields)*

                #update_validation_stage

                if dry_run {
                    // ── Stage 5: App Logic (identity) ──
                    let before = ::standout::cli::app_logic_identity(before)?;

                    // ── Stage 6: View Building ──
                    Ok(::standout::cli::Output::Render(
                        ::standout::views::update_view(before.clone())
                            .before(before)
                            .changed_fields(__changed)
                            .dry_run()
                            .info("Dry run - no changes made")
                            .build()
                    ))
                } else if __changed.is_empty() {
                    // ── Stage 5: App Logic (identity) ──
                    let before = ::standout::cli::app_logic_identity(before)?;

                    // ── Stage 6: View Building ──
                    Ok(::standout::cli::Output::Render(
                        ::standout::views::update_view(before)
                            .info("No changes specified")
                            .build()
                    ))
                } else {
                    // ── Stage 5: Store Update ──
                    let after = store.update(&id, __data)?;

                    // ── Stage 6: App Logic (identity) ──
                    let after = ::standout::cli::app_logic_identity(after)?;

                    // ── Stage 7: View Building ──
                    Ok(::standout::cli::Output::Render(
                        ::standout::views::update_view(after)
                            .before(before)
                            .changed_fields(__changed)
                            .success(format!("{} updated", #object_name_upper))
                            .build()
                    ))
                }
            }
        }
    } else {
        quote! {}
    };

    let delete_handler = if operations.contains(&ResourceOperation::Delete) {
        quote! {
            pub fn delete(
                matches: &::clap::ArgMatches,
                ctx: &::standout::cli::CommandContext,
            ) -> ::standout::cli::HandlerResult<::serde_json::Value> {
                let store = ctx.app_state.get_required::<#store_type>()?;
                let confirm = matches.get_flag("confirm");
                let force = matches.get_flag("force");

                // ── Stage 1: Get IDs ──
                let id_strs: Vec<String> = matches.get_many::<String>("ids")
                    .map(|v| v.cloned().collect())
                    .unwrap_or_default();

                if id_strs.len() == 1 {
                    // Single ID - backwards compatible behavior
                    let id_str = &id_strs[0];
                    let id = store.parse_id(id_str)
                        .map_err(|e| ::standout::cli::IdResolutionError::parse_failed(id_str, e.to_string()))?;

                    let item = store.resolve(&id)
                        .map_err(|_| ::standout::cli::IdResolutionError::not_found(id_str))?;

                    let item = ::standout::cli::validate_identity(item)?;

                    if !confirm && !force {
                        let item = ::standout::cli::app_logic_identity(item)?;
                        let result = ::standout::views::delete_view(item)
                            .warning(format!("Use --confirm to delete this {}", #object_name))
                            .build();
                        Ok(::standout::cli::Output::Render(
                            ::serde_json::to_value(result).unwrap_or_default()
                        ))
                    } else {
                        store.delete(&id)?;
                        let item = ::standout::cli::app_logic_identity(item)?;
                        let result = ::standout::views::delete_view(item)
                            .confirmed()
                            .success(format!("{} deleted", #object_name_upper))
                            .build();
                        Ok(::standout::cli::Output::Render(
                            ::serde_json::to_value(result).unwrap_or_default()
                        ))
                    }
                } else {
                    // Multiple IDs - batch operation
                    if !confirm && !force {
                        // Show what would be deleted
                        let mut items: Vec<#struct_name> = Vec::new();
                        let mut errors: Vec<String> = Vec::new();

                        for id_str in &id_strs {
                            match store.parse_id(id_str) {
                                Ok(id) => {
                                    match store.resolve(&id) {
                                        Ok(item) => items.push(item),
                                        Err(_) => errors.push(format!("'{}' not found", id_str)),
                                    }
                                }
                                Err(e) => errors.push(format!("Invalid ID '{}': {}", id_str, e)),
                            }
                        }

                        let count = items.len();
                        let mut result = ::standout::views::list_view(items)
                            .total_count(count)
                            .tabular_spec(<#struct_name as ::standout::tabular::Tabular>::tabular_spec())
                            .build();

                        result = result.warning(format!("Use --confirm to delete {} {}(s)", count, #object_name));
                        for err in errors {
                            result = result.warning(err);
                        }

                        Ok(::standout::cli::Output::Render(
                            ::serde_json::to_value(result).unwrap_or_default()
                        ))
                    } else {
                        // Actually delete
                        let mut deleted: Vec<#struct_name> = Vec::new();
                        let mut errors: Vec<String> = Vec::new();

                        for id_str in &id_strs {
                            match store.parse_id(id_str) {
                                Ok(id) => {
                                    match store.resolve(&id) {
                                        Ok(item) => {
                                            match store.delete(&id) {
                                                Ok(()) => deleted.push(item),
                                                Err(e) => errors.push(format!("Failed to delete '{}': {}", id_str, e)),
                                            }
                                        }
                                        Err(_) => errors.push(format!("'{}' not found", id_str)),
                                    }
                                }
                                Err(e) => errors.push(format!("Invalid ID '{}': {}", id_str, e)),
                            }
                        }

                        let deleted_count = deleted.len();
                        let error_count = errors.len();

                        let mut result = ::standout::views::list_view(deleted)
                            .total_count(deleted_count)
                            .tabular_spec(<#struct_name as ::standout::tabular::Tabular>::tabular_spec())
                            .build();

                        if error_count == 0 {
                            result = result.success(format!("{} {}(s) deleted", deleted_count, #object_name));
                        } else {
                            result = result.info(format!("{} deleted, {} failed", deleted_count, error_count));
                        }
                        for err in errors {
                            result = result.warning(err);
                        }

                        Ok(::standout::cli::Output::Render(
                            ::serde_json::to_value(result).unwrap_or_default()
                        ))
                    }
                }
            }
        }
    } else {
        quote! {}
    };

    // Generate command attribute based on whether default is set
    let command_attr = if default_command.is_some() {
        quote! { #[command(subcommand_required = false)] }
    } else {
        quote! {}
    };

    // Generate default_command method
    let default_command_method = if let Some(ref cmd) = default_command {
        quote! {
            /// Returns the default subcommand name, if configured.
            pub fn default_command() -> Option<&'static str> {
                Some(#cmd)
            }
        }
    } else {
        quote! {
            /// Returns the default subcommand name, if configured.
            pub fn default_command() -> Option<&'static str> {
                None
            }
        }
    };

    // Generate the full output
    let expanded = quote! {
        /// Commands enum for Resource operations on #struct_name
        #[derive(::clap::Subcommand, Clone, Debug)]
        #command_attr
        pub enum #commands_enum_name {
            #(#command_variants)*
        }

        impl #commands_enum_name {
            /// Returns the dispatch configuration for these Resource commands
            pub fn dispatch_config() -> impl FnOnce(::standout::cli::GroupBuilder) -> ::standout::cli::GroupBuilder {
                |__builder| {
                    #(#dispatch_commands)*
                    __builder
                }
            }

            #default_command_method
        }

        #[doc(hidden)]
        #[allow(non_snake_case)]
        mod #handlers_module_name {
            use super::*;

            #list_handler
            #view_handler
            #create_handler
            #update_handler
            #delete_handler
        }
    };

    Ok(expanded)
}
