//! Procedural macros for clap-mcp.
//!
//! Provides `#[derive(ClapMcp)]` for attribute-based execution safety configuration
//! and `ClapMcpToolExecutor` implementation.

use proc_macro::TokenStream;
use quote::{ToTokens, quote};
use syn::{
    DeriveInput, Expr, GenericArgument, Lit, Meta, MetaNameValue, Path, PathArguments, Type,
    parse_macro_input,
};

/// Parsed `#[clap_mcp(...)]` config.
type ClapMcpAttrs = (
    Option<bool>,
    Option<bool>,
    Option<bool>,
    Option<bool>,
    Option<bool>,
    Option<bool>,
    Option<bool>,
    Option<String>,
    Option<String>,
    Option<String>,
);

fn meta_string_value(meta: &syn::meta::ParseNestedMeta) -> syn::Result<String> {
    let value: Expr = meta.value()?.parse()?;
    match value {
        Expr::Lit(lit) => match lit.lit {
            Lit::Str(s) => Ok(s.value()),
            other => Err(meta.error(format!("expected string literal, got `{other:?}`"))),
        },
        other => Err(meta.error(format!("expected string literal, got `{other:?}`"))),
    }
}

/// Parses `#[clap_mcp(...)]` attributes.
fn parse_clap_mcp_attrs(attrs: &[syn::Attribute]) -> ClapMcpAttrs {
    let mut parallel_safe = None;
    let mut reinvocation_safe = None;
    let mut share_runtime = None;
    let mut catch_in_process_panics = None;
    let mut allow_mcp_without_subcommand = None;
    let mut task_augmented_tools = None;
    let mut stateful = None;
    let mut mcp_flag = None;
    let mut mcp_http_flag = None;
    let mut export_skills_flag = None;

    for attr in attrs {
        if !attr.path().is_ident("clap_mcp") {
            continue;
        }

        let _ = attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("parallel_safe") {
                if meta.input.peek(syn::token::Eq) {
                    let value: Expr = meta.value()?.parse()?;
                    parallel_safe = Some(expr_to_bool(&value));
                } else {
                    parallel_safe = Some(true); // shorthand: parallel_safe means true
                }
            } else if meta.path.is_ident("reinvocation_safe") {
                if meta.input.peek(syn::token::Eq) {
                    let value: Expr = meta.value()?.parse()?;
                    reinvocation_safe = Some(expr_to_bool(&value));
                } else {
                    reinvocation_safe = Some(true); // shorthand
                }
            } else if meta.path.is_ident("share_runtime") {
                if meta.input.peek(syn::token::Eq) {
                    let value: Expr = meta.value()?.parse()?;
                    share_runtime = Some(expr_to_bool(&value));
                } else {
                    share_runtime = Some(true); // shorthand
                }
            } else if meta.path.is_ident("catch_in_process_panics") {
                if meta.input.peek(syn::token::Eq) {
                    let value: Expr = meta.value()?.parse()?;
                    catch_in_process_panics = Some(expr_to_bool(&value));
                } else {
                    catch_in_process_panics = Some(true); // shorthand
                }
            } else if meta.path.is_ident("allow_mcp_without_subcommand") {
                if meta.input.peek(syn::token::Eq) {
                    let value: Expr = meta.value()?.parse()?;
                    allow_mcp_without_subcommand = Some(expr_to_bool(&value));
                } else {
                    allow_mcp_without_subcommand = Some(true); // shorthand
                }
            } else if meta.path.is_ident("task_augmented_tools") {
                if meta.input.peek(syn::token::Eq) {
                    let value: Expr = meta.value()?.parse()?;
                    task_augmented_tools = Some(expr_to_bool(&value));
                } else {
                    task_augmented_tools = Some(true); // shorthand
                }
            } else if meta.path.is_ident("stateful") {
                if meta.input.peek(syn::token::Eq) {
                    let value: Expr = meta.value()?.parse()?;
                    stateful = Some(expr_to_bool(&value));
                } else {
                    stateful = Some(true);
                }
            } else if meta.path.is_ident("mcp_flag") {
                mcp_flag = Some(meta_string_value(&meta)?);
            } else if meta.path.is_ident("mcp_http_flag") {
                mcp_http_flag = Some(meta_string_value(&meta)?);
            } else if meta.path.is_ident("export_skills_flag") {
                export_skills_flag = Some(meta_string_value(&meta)?);
            }
            Ok(())
        });
    }

    (
        parallel_safe,
        reinvocation_safe,
        share_runtime,
        catch_in_process_panics,
        allow_mcp_without_subcommand,
        task_augmented_tools,
        stateful,
        mcp_flag,
        mcp_http_flag,
        export_skills_flag,
    )
}

fn expr_to_bool(expr: &Expr) -> bool {
    match expr {
        Expr::Lit(lit) => match &lit.lit {
            Lit::Bool(b) => b.value,
            _ => false,
        },
        _ => false,
    }
}

/// Returns true if the field has `#[command(subcommand)]`.
fn field_has_command_subcommand(attrs: &[syn::Attribute]) -> bool {
    for attr in attrs {
        if !attr.path().is_ident("command") {
            continue;
        }
        let mut has_subcommand = false;
        let _ = attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("subcommand") {
                has_subcommand = true;
            }
            Ok(())
        });
        if has_subcommand {
            return true;
        }
    }
    false
}

/// Parses #[clap_mcp(task)] on enum variants — marks the tool as eligible for MCP task-augmented
/// `tools/call` when [`ClapMcpConfig::task_augmented_tools`] is enabled. If no variant has this
/// attribute, all tools are eligible when tasks are enabled.
fn has_clap_mcp_task(attrs: &[syn::Attribute]) -> bool {
    for attr in attrs {
        if !attr.path().is_ident("clap_mcp") {
            continue;
        }
        let mut found = false;
        let _ = attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("task") {
                found = true;
            }
            Ok(())
        });
        if found {
            return true;
        }
    }
    false
}

/// Parsed `#[clap_mcp(serialized)]` scope on a variant.
enum ClapMcpSerialized {
    Tool,
    Args(Vec<String>),
}

/// Parses `#[clap_mcp(serialized)]` or `#[clap_mcp(serialized = "arg1, arg2")]` on enum variants.
fn get_clap_mcp_serialized(attrs: &[syn::Attribute]) -> Option<ClapMcpSerialized> {
    for attr in attrs {
        if !attr.path().is_ident("clap_mcp") {
            continue;
        }
        let mut result = None;
        let parse_result = attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("serialized") {
                if meta.input.peek(syn::token::Eq) {
                    let value: Expr = meta.value()?.parse()?;
                    if let Expr::Lit(lit) = value
                        && let Lit::Str(s) = &lit.lit
                    {
                        let args: Vec<String> = s
                            .value()
                            .split(',')
                            .map(|p| p.trim().to_string())
                            .filter(|p| !p.is_empty())
                            .collect();
                        if args.is_empty() {
                            return Err(
                                meta.error("serialized = \"...\" requires at least one arg id")
                            );
                        }
                        result = Some(ClapMcpSerialized::Args(args));
                    }
                } else {
                    result = Some(ClapMcpSerialized::Tool);
                }
            }
            Ok(())
        });
        if parse_result.is_err() {
            continue;
        }
        if result.is_some() {
            return result;
        }
    }
    None
}

fn variant_field_ids(fields: &syn::Fields) -> Vec<String> {
    fields
        .iter()
        .enumerate()
        .map(|(i, f)| {
            f.ident
                .as_ref()
                .map(|ident| ident.to_string())
                .unwrap_or_else(|| format!("__f{i}"))
        })
        .collect()
}

/// Parses `#[clap_mcp(serialize_topic)]` on a field used with arg-scoped `serialized`.
fn has_clap_mcp_serialize_topic(attrs: &[syn::Attribute]) -> bool {
    for attr in attrs {
        if !attr.path().is_ident("clap_mcp") {
            continue;
        }
        let mut found = false;
        let _ = attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("serialize_topic") {
                found = true;
            }
            Ok(())
        });
        if found {
            return true;
        }
    }
    false
}

fn serialized_scope_arg_ids(serialized: &ClapMcpSerialized) -> Option<&[String]> {
    match serialized {
        ClapMcpSerialized::Tool => None,
        ClapMcpSerialized::Args(ids) => Some(ids.as_slice()),
    }
}

/// Parses #[clap_mcp(skip)] from attributes.
fn has_clap_mcp_schema_only(attrs: &[syn::Attribute]) -> bool {
    for attr in attrs {
        if !attr.path().is_ident("clap_mcp") {
            continue;
        }
        let mut found = false;
        let _ = attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("schema_only") {
                found = true;
            }
            Ok(())
        });
        if found {
            return true;
        }
    }
    false
}

fn has_clap_mcp_skip(attrs: &[syn::Attribute]) -> bool {
    for attr in attrs {
        if !attr.path().is_ident("clap_mcp") {
            continue;
        }
        let mut has_skip = false;
        let _ = attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("skip") {
                has_skip = true;
            }
            Ok(())
        });
        if has_skip {
            return true;
        }
    }
    false
}

/// Parses #[clap_mcp(skip_root_when_subcommands)] from root struct attributes.
/// When present on a struct root with a subcommand, the root is excluded from the MCP tool list.
fn has_clap_mcp_skip_root_when_subcommands(attrs: &[syn::Attribute]) -> bool {
    for attr in attrs {
        if !attr.path().is_ident("clap_mcp") {
            continue;
        }
        let mut found = false;
        let _ = attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("skip_root_when_subcommands") {
                found = true;
            }
            Ok(())
        });
        if found {
            return true;
        }
    }
    false
}

/// Parses variant-level #[clap_mcp(requires = "arg1,arg2")] - comma-separated list.
fn get_clap_mcp_requires_variant(attrs: &[syn::Attribute]) -> Option<Vec<String>> {
    for attr in attrs {
        if !attr.path().is_ident("clap_mcp") {
            continue;
        }
        let mut result = None;
        let _ = attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("requires") && meta.input.peek(syn::token::Eq) {
                let value: Expr = meta.value()?.parse()?;
                if let Expr::Lit(lit) = value
                    && let Lit::Str(s) = &lit.lit
                {
                    result = Some(
                        s.value()
                            .split(',')
                            .map(|p| p.trim().to_string())
                            .filter(|p| !p.is_empty())
                            .collect(),
                    );
                }
            }
            Ok(())
        });
        if result.is_some() {
            return result;
        }
    }
    None
}

/// Parses `#[clap_mcp_output_from = "run"]` (or path like `my_mod::run`) from enum attributes.
/// When present, execute_for_mcp is generated by calling this function and converting the result.
fn get_clap_mcp_output_from(attrs: &[syn::Attribute]) -> Option<Path> {
    for attr in attrs {
        if !attr.path().is_ident("clap_mcp_output_from") {
            continue;
        }
        if let Meta::NameValue(MetaNameValue { value, .. }) = &attr.meta
            && let Expr::Lit(lit) = value
            && let Lit::Str(s) = &lit.lit
            && let Ok(path) = syn::parse_str::<Path>(&s.value())
        {
            return Some(path);
        }
    }
    None
}

fn get_clap_mcp_output_from_with_state(attrs: &[syn::Attribute]) -> Option<Path> {
    for attr in attrs {
        if !attr.path().is_ident("clap_mcp_output_from_with_state") {
            continue;
        }
        if let Meta::NameValue(MetaNameValue { value, .. }) = &attr.meta
            && let Expr::Lit(lit) = value
            && let Lit::Str(s) = &lit.lit
            && let Ok(path) = syn::parse_str::<Path>(&s.value())
        {
            return Some(path);
        }
    }
    None
}

fn get_clap_mcp_state_type(attrs: &[syn::Attribute]) -> Option<syn::Type> {
    for attr in attrs {
        if !attr.path().is_ident("clap_mcp_state_type") {
            continue;
        }
        if let Meta::NameValue(MetaNameValue { value, .. }) = &attr.meta
            && let Expr::Lit(lit) = value
            && let Lit::Str(s) = &lit.lit
            && let Ok(ty) = syn::parse_str::<syn::Type>(&s.value())
        {
            return Some(ty);
        }
    }
    None
}

/// Parses `#[clap_mcp_output_type = "TypeName"]` from enum attributes (for output schema).
fn get_clap_mcp_output_type(attrs: &[syn::Attribute]) -> Option<syn::Type> {
    for attr in attrs {
        if !attr.path().is_ident("clap_mcp_output_type") {
            continue;
        }
        if let Meta::NameValue(MetaNameValue { value, .. }) = &attr.meta
            && let Expr::Lit(lit) = value
            && let Lit::Str(s) = &lit.lit
            && let Ok(ty) = syn::parse_str::<syn::Type>(&s.value())
        {
            return Some(ty);
        }
    }
    None
}

/// Parses `#[clap_mcp_output_one_of = "T1, T2, T3"]` from enum attributes (for oneOf output schema).
fn get_clap_mcp_output_one_of(attrs: &[syn::Attribute]) -> Option<Vec<syn::Type>> {
    for attr in attrs {
        if !attr.path().is_ident("clap_mcp_output_one_of") {
            continue;
        }
        if let Meta::NameValue(MetaNameValue { value, .. }) = &attr.meta
            && let Expr::Lit(lit) = value
            && let Lit::Str(s) = &lit.lit
        {
            let types: Result<Vec<syn::Type>, _> = s
                .value()
                .split(',')
                .map(|p| syn::parse_str::<syn::Type>(p.trim()))
                .collect();
            return types.ok();
        }
    }
    None
}

/// Parses #[clap_mcp(requires)] or #[clap_mcp(requires = "arg_name")] from field attributes.
/// Returns Some(arg_name) when present; empty string means use the field's own ident.
fn get_clap_mcp_requires(attrs: &[syn::Attribute]) -> Option<String> {
    for attr in attrs {
        if !attr.path().is_ident("clap_mcp") {
            continue;
        }
        let mut result = None;
        let _ = attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("requires") {
                if meta.input.peek(syn::token::Eq) {
                    let value: Expr = meta.value()?.parse()?;
                    if let Expr::Lit(lit) = value
                        && let Lit::Str(s) = &lit.lit
                    {
                        result = Some(s.value());
                    }
                } else {
                    result = Some(String::new()); // use field ident
                }
            }
            Ok(())
        });
        if result.is_some() {
            return result;
        }
    }
    None
}

/// Returns true if the field has #[arg(long)] or #[arg(short)] (i.e. is a flag/option, not positional).
fn field_has_arg_long_or_short(attrs: &[syn::Attribute]) -> bool {
    for attr in attrs {
        if !attr.path().is_ident("arg") {
            continue;
        }
        let mut has = false;
        let _ = attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("long") || meta.path.is_ident("short") {
                has = true;
            }
            Ok(())
        });
        if has {
            return true;
        }
    }
    false
}

/// Returns true if the field has #[arg(index = ...)].
fn field_has_arg_index(attrs: &[syn::Attribute]) -> bool {
    for attr in attrs {
        if !attr.path().is_ident("arg") {
            continue;
        }
        let mut has = false;
        let _ = attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("index") {
                has = true;
            }
            Ok(())
        });
        if has {
            return true;
        }
    }
    false
}

/// Heuristic: true if the field looks like a positional (no long/short, or has index).
fn field_looks_positional(attrs: &[syn::Attribute]) -> bool {
    field_has_arg_index(attrs) || !field_has_arg_long_or_short(attrs)
}

/// Gets command name from #[command(name = "x")] or converts ident to kebab-case.
fn get_command_name(attrs: &[syn::Attribute], ident: &syn::Ident) -> String {
    for attr in attrs {
        if !attr.path().is_ident("command") {
            continue;
        }
        let mut name = None;
        let _ = attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("name") {
                let value: Expr = meta.value()?.parse()?;
                if let Expr::Lit(lit) = value
                    && let Lit::Str(s) = &lit.lit
                {
                    name = Some(s.value());
                }
            }
            Ok(())
        });
        if let Some(n) = name {
            return n;
        }
    }
    ident_to_kebab(ident)
}

fn inner_type_if_option(ty: &Type) -> Option<&Type> {
    let Type::Path(type_path) = ty else {
        return None;
    };
    let last = type_path.path.segments.last()?;
    if last.ident != "Option" {
        return None;
    }
    let PathArguments::AngleBracketed(args) = &last.arguments else {
        return None;
    };
    args.args.first().and_then(|a| {
        if let GenericArgument::Type(t) = a {
            Some(t)
        } else {
            None
        }
    })
}

fn ident_to_kebab(ident: &syn::Ident) -> String {
    let s = ident.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            out.push('-');
        }
        for c in c.to_lowercase() {
            out.push(c);
        }
    }
    out
}

/// Returns true if the type is `Option<T>`.
fn is_option_type(ty: &Type) -> bool {
    let Type::Path(type_path) = ty else {
        return false;
    };
    let Some(last) = type_path.path.segments.last() else {
        return false;
    };
    if last.ident != "Option" {
        return false;
    }
    let PathArguments::AngleBracketed(args) = &last.arguments else {
        return false;
    };
    let type_args: Vec<_> = args
        .args
        .iter()
        .filter_map(|a| {
            if let GenericArgument::Type(_) = a {
                Some(())
            } else {
                None
            }
        })
        .collect();
    type_args.len() == 1
}

fn field_is_repeated_mcp_scalar(ty: &Type) -> bool {
    is_positional_scalar_field(ty)
}

fn is_positional_scalar_field(ty: &Type) -> bool {
    let ty = inner_type_if_option(ty).unwrap_or(ty);
    let Type::Path(type_path) = ty else {
        return true;
    };
    let Some(last) = type_path.path.segments.last() else {
        return true;
    };
    if last.ident == "Vec" {
        return false;
    }
    true
}

fn has_ambiguous_mcp_positionals<'a, I>(fields: I) -> bool
where
    I: IntoIterator<Item = &'a syn::Field>,
{
    let mut positional_scalars = 0usize;
    for field in fields {
        if has_clap_mcp_skip(&field.attrs)
            || !field_looks_positional(&field.attrs)
            || !field_is_repeated_mcp_scalar(&field.ty)
        {
            continue;
        }
        positional_scalars += 1;
        if positional_scalars > 1 {
            return true;
        }
    }
    false
}

fn strip_option_type(ty: &Type) -> Type {
    inner_type_if_option(ty)
        .map(|t| (*t).clone())
        .unwrap_or_else(|| ty.clone())
}

fn subcommand_field_type_from_enum(data: &syn::DataEnum) -> Option<Type> {
    let mut found: Option<Type> = None;
    for variant in &data.variants {
        for field in variant.fields.iter() {
            if !field_has_command_subcommand(&field.attrs) {
                continue;
            }
            let ty = strip_option_type(&field.ty);
            if let Some(prev) = &found {
                if prev.to_token_stream().to_string() != ty.to_token_stream().to_string() {
                    return None;
                }
            } else {
                found = Some(ty);
            }
        }
    }
    found
}

fn nested_subcommand_type_paths_from_enum(data: &syn::DataEnum) -> Vec<syn::Path> {
    let mut paths = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for variant in &data.variants {
        for field in variant.fields.iter() {
            if !field_has_command_subcommand(&field.attrs) {
                continue;
            }
            let sub_ty = inner_type_if_option(&field.ty).unwrap_or(&field.ty);
            if let syn::Type::Path(tp) = sub_ty {
                let key = tp.path.to_token_stream().to_string();
                if seen.insert(key) {
                    paths.push(tp.path.clone());
                }
            }
        }
    }
    paths
}

/// Derive macro for `ClapMcpConfigProvider` and `ClapMcpToolExecutor`.
///
/// Use on a clap `Parser` enum to expose it over MCP. Implements execution safety
/// config and tool output generation.
///
/// ## Struct root with subcommand
///
/// When your CLI has a **struct root** with `#[command(subcommand)]`, derive
/// `ClapMcp` on **both** the root struct and the subcommand enum. Put
/// `#[clap_mcp(...)]` on the **root struct** (the type that implements
/// `parse_or_serve_mcp()`); put `#[clap_mcp_output_from = "run"]` on the
/// **subcommand enum**. The root delegates tool execution to the subcommand.
///
/// # Attributes
///
/// ## `#[clap_mcp(...)]` (on the enum)
///
/// - `parallel_safe` / `parallel_safe = true|false` — If true, tool calls may run concurrently.
/// - `reinvocation_safe` / `reinvocation_safe = true|false` — If true, uses in-process execution.
/// - `share_runtime` / `share_runtime = true|false` — When reinvocation_safe, whether async tools
///   (via `clap_mcp::run_async_tool`) share the MCP server's tokio runtime (`true`) or use a
///   dedicated thread (`false`, default). Ignored when reinvocation_safe is false.
/// - `catch_in_process_panics` / `catch_in_process_panics = true|false` — When reinvocation_safe,
///   if true, panics in tool code are caught and returned as MCP errors instead of crashing the
///   server. Default is false. See [`ClapMcpConfig::catch_in_process_panics`].
/// - `allow_mcp_without_subcommand` / `allow_mcp_without_subcommand = true|false` — When true
///   (default), `myapp --mcp` starts MCP even when the root has `subcommand_required = true`
///   (argv is checked before clap). Does **not** change non-MCP CLI behavior; do not switch to
///   `Option<Commands>` solely for MCP. See [`ClapMcpConfig::allow_mcp_without_subcommand`].
/// - `mcp_flag = "long_name"` — Rename the stdio MCP flag long name (default `"mcp"`). clap arg
///   id stays [`CLAP_MCP_STDIO_FLAG_ID`](clap_mcp::CLAP_MCP_STDIO_FLAG_ID).
/// - `mcp_http_flag = "long_name"` — Rename the HTTP MCP flag (requires `http` feature).
/// - `export_skills_flag = "long_name"` — Rename the export-skills flag.
/// - `task_augmented_tools` / `task_augmented_tools = true|false` — When true, advertise MCP task
///   support and handle task-augmented `tools/call` (in-process only). Requires
///   `reinvocation_safe`; combining with `reinvocation_safe = false` is a **compile error**.
///   With `parallel_safe = false`, task and plain tool bodies share one serialization queue.
///   With `parallel_safe = true`, task bodies may overlap with each other and with plain
///   `tools/call`; logging during tasks uses per-task context so `meta.taskId` stays correct.
///   `catch_in_process_panics = true` maps panics in task-scheduled work to task error payloads.
/// - `stateful` / `stateful = true|false` — On a struct root (or delegating enum) with a
///   subcommand field, implement [`ClapMcpToolExecutorWithState`] by delegating to the
///   subcommand. Requires `reinvocation_safe`.
///
/// ## `#[clap_mcp(task)]` (on variant)
///
/// When `task_augmented_tools` is enabled, marks this subcommand as eligible for task-augmented
/// `tools/call`. If **no** variant has `#[clap_mcp(task)]`, **all** tools are eligible.
///
/// ## `#[clap_mcp_output_from = "run"]` (on the enum)
///
/// When present, tool execution is driven by a single function instead of per-variant attributes.
/// The value is the path to a function (e.g. `"run"` or `"my_mod::run"`) that takes the CLI type
/// by value and returns a type implementing `IntoClapMcpResult` (e.g. `String`, `AsStructured<T>`,
/// `Option<O>`, `Result<O, E>`). The macro generates `execute_for_mcp(self)` as
/// `run(self).into_tool_result()`. **Required** for enums.
///
/// ## `#[clap_mcp_output_type = "TypeName"]` (on the enum, requires `output-schema` feature)
///
/// When present and the crate is built with `output-schema`, the type's JSON schema (via
/// `schemars::JsonSchema`) is set on [`ClapMcpSchemaMetadata::output_schema`] so each tool
/// gets an `output_schema` for MCP clients.
///
/// ## `#[clap_mcp_output_one_of = "T1, T2, T3"]` (on the enum, requires `output-schema` feature)
///
/// When present and the crate is built with `output-schema`, builds a JSON schema with `oneOf`
/// from the listed types (each must implement `schemars::JsonSchema`) and sets it on
/// [`ClapMcpSchemaMetadata::output_schema`]. Use when you want an explicit list of output
/// types without a wrapper enum. If both `output_type` and `output_one_of` are set,
/// `output_one_of` is used.
///
/// ## Stateful tools (`#[clap_mcp_output_from_with_state]`, `#[clap_mcp(stateful)]`)
///
/// For session state across MCP tool calls (requires `reinvocation_safe`), see
/// [`ClapMcpToolExecutorWithState`]. On the **leaf** subcommand enum:
///
/// - `#[clap_mcp_output_from_with_state = "run"]` — path to `run(cmd, state: &State) -> T`
/// - `#[clap_mcp_state_type = "Type"]` — must match the second parameter of `run` (without `&`)
///
/// On struct roots or intermediate subcommand enums that delegate to a stateful subcommand:
///
/// - `#[clap_mcp(stateful)]` — implements `ClapMcpToolExecutorWithState` with
///   `type State = <Subcommand as ClapMcpToolExecutorWithState>::State` (no duplicate
///   `state_type`).
///
/// ## Positional arguments and MCP
///
/// MCP clients send **named** JSON; clap-mcp rebuilds argv for tool execution. Two or more
/// bare positional scalar fields on the same variant (non-`Vec`) are a **compile error** —
/// use `#[arg(long)]` on each field or `#[clap_mcp(skip)]`. See [PR #12](https://github.com/canardleteer/clap-mcp/pull/12).
///
/// **Trailing / passthrough args:** For cargo-style trailing argv, use
/// `#[arg(last = true, allow_hyphen_values = true)] command: Vec<String>` on direct CLI;
/// MCP clients pass `command` as a JSON array. `build_tool_argv` inserts `--` before
/// trailing multi-value positionals when rebuilding argv. An explicit
/// `#[arg(long)] args: Vec<String>` is often clearer for MCP. Pre-clap MCP detection
/// ignores tokens after the shell's first `--`.
///
/// ## `#[clap_mcp(skip)]` (on variant or field)
///
/// Exclude the subcommand or argument from MCP exposure.
///
/// ## `#[clap_mcp(skip_root_when_subcommands)]` (on root struct with subcommand)
///
/// When present on a struct root that has `#[command(subcommand)]`, the root command
/// is excluded from the MCP tool list; only subcommands appear as tools. Equivalent to
/// setting `ClapMcpSchemaMetadata::skip_root_command_when_subcommands = true` imperatively.
///
/// ## `#[clap_mcp(requires)]` / `#[clap_mcp(requires = "arg_name")]` (on field)
///
/// Make the argument required in the MCP tool schema even if optional in clap.
/// Use `requires` for the field's own id, or `requires = "name"` to specify.
///
/// ## `#[clap_mcp(serialized)]` / `#[clap_mcp(serialized = "arg1, arg2")]` (on variant)
///
/// When [`ClapMcpConfig::parallel_safe`] is true, serializes concurrent MCP invocations of this
/// tool. Shorthand `serialized` locks the whole tool; `serialized = "output"` locks by arg id
/// (comma-separated for multiple ids). See the execution-safety guide for documented use.
///
/// Optional: `#[clap_mcp(serialize_topic)]` on a field listed in `serialized = "..."` uses
/// [`ClapMcpSerializeTopic`] for that arg when you opt into typed topic keys.
///
/// ## `#[clap_mcp(requires = "arg1,arg2")]` (on variant)
///
/// Variant-level alternative: one or more optional args to make required (single name or
/// comma-separated list). The MCP tool schema will mark each listed argument as required.
/// Prefer this when declaring multiple required args. When the client omits a required
/// arg, a clear error is returned.
///
/// # Example (idiomatic: single `run` function, no duplicated logic)
///
/// ```rust,ignore
/// use clap::Parser;
///
/// #[derive(Debug, Parser, clap_mcp::ClapMcp)]
/// #[clap_mcp(reinvocation_safe, parallel_safe = false)]
/// #[clap_mcp_output_from = "run"]
/// enum Cli {
///     Greet { #[arg(long)] name: Option<String> },
///     Add { #[arg(long)] a: i32, #[arg(long)] b: i32 },
/// }
///
/// fn run(cmd: Cli) -> String {
///     match cmd {
///         Cli::Greet { name } => format!("Hello, {}!", name.as_deref().unwrap_or("world")),
///         Cli::Add { a, b } => (a + b).to_string(),
///     }
/// }
/// ```
#[proc_macro_derive(
    ClapMcp,
    attributes(
        clap_mcp,
        clap_mcp_output_from,
        clap_mcp_output_from_with_state,
        clap_mcp_state_type,
        clap_mcp_output_type,
        clap_mcp_output_one_of,
        command,
        arg
    )
)]
pub fn derive_clap_mcp(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    match &input.data {
        syn::Data::Enum(data) => {
            for variant in &data.variants {
                if has_clap_mcp_skip(&variant.attrs) {
                    continue;
                }
                if has_ambiguous_mcp_positionals(variant.fields.iter()) {
                    return TokenStream::from(
                        syn::Error::new_spanned(
                            &variant.ident,
                            "clap_mcp: multiple positional scalar arguments are ambiguous for MCP \
                             tool calls; use #[arg(long)] on each field or #[clap_mcp(skip)]",
                        )
                        .to_compile_error(),
                    );
                }
                if let Some(serialized) = get_clap_mcp_serialized(&variant.attrs) {
                    let field_ids: std::collections::HashSet<String> =
                        variant_field_ids(&variant.fields).into_iter().collect();
                    if let ClapMcpSerialized::Args(arg_ids) = &serialized {
                        for arg_id in arg_ids {
                            if !field_ids.contains(arg_id) {
                                return TokenStream::from(
                                    syn::Error::new_spanned(
                                        &variant.ident,
                                        format!(
                                            "clap_mcp: serialized = \"{arg_id}\" — no field or arg \
                                             with id `{arg_id}` on this variant"
                                        ),
                                    )
                                    .to_compile_error(),
                                );
                            }
                        }
                    }
                    for (i, f) in variant.fields.iter().enumerate() {
                        if !has_clap_mcp_serialize_topic(&f.attrs) {
                            continue;
                        }
                        let arg_id = f
                            .ident
                            .as_ref()
                            .map(|i| i.to_string())
                            .unwrap_or_else(|| format!("__f{i}"));
                        let Some(scope_args) = serialized_scope_arg_ids(&serialized) else {
                            return TokenStream::from(
                                syn::Error::new_spanned(
                                    f.ident.as_ref().unwrap_or(&variant.ident),
                                    "clap_mcp: #[clap_mcp(serialize_topic)] requires arg-scoped \
                                     #[clap_mcp(serialized = \"arg_id\")] on the same variant",
                                )
                                .to_compile_error(),
                            );
                        };
                        if !scope_args.contains(&arg_id) {
                            return TokenStream::from(
                                syn::Error::new_spanned(
                                    f.ident.as_ref().unwrap_or(&variant.ident),
                                    format!(
                                        "clap_mcp: #[clap_mcp(serialize_topic)] on `{arg_id}` \
                                         requires that arg in #[clap_mcp(serialized = \"...\")]"
                                    ),
                                )
                                .to_compile_error(),
                            );
                        }
                    }
                }
            }
        }
        syn::Data::Struct(data) => {
            let subcommand_field = data
                .fields
                .iter()
                .find(|f| field_has_command_subcommand(&f.attrs));
            if has_ambiguous_mcp_positionals(
                data.fields
                    .iter()
                    .filter(|f| !subcommand_field.is_some_and(|sf| std::ptr::eq(sf, *f))),
            ) {
                return TokenStream::from(
                    syn::Error::new_spanned(
                        &input.ident,
                        "clap_mcp: multiple positional scalar arguments are ambiguous for MCP \
                         tool calls; use #[arg(long)] on each field or #[clap_mcp(skip)]",
                    )
                    .to_compile_error(),
                );
            }
        }
        _ => {}
    }

    let output_from_with_state = get_clap_mcp_output_from_with_state(&input.attrs);
    let state_type = get_clap_mcp_state_type(&input.attrs);
    let schema_only = has_clap_mcp_schema_only(&input.attrs);
    if schema_only {
        if !matches!(&input.data, syn::Data::Enum(_)) {
            return TokenStream::from(
                syn::Error::new_spanned(
                    &input.ident,
                    "clap_mcp: #[clap_mcp(schema_only)] is only supported on subcommand enums",
                )
                .to_compile_error(),
            );
        }
        if get_clap_mcp_output_from(&input.attrs).is_some()
            || output_from_with_state.is_some()
            || get_clap_mcp_state_type(&input.attrs).is_some()
        {
            return TokenStream::from(
                syn::Error::new_spanned(
                    &input.ident,
                    "clap_mcp: #[clap_mcp(schema_only)] cannot be combined with \
                     #[clap_mcp_output_from], #[clap_mcp_output_from_with_state], or \
                     #[clap_mcp_state_type]",
                )
                .to_compile_error(),
            );
        }
    }
    if state_type.is_some() && output_from_with_state.is_none() {
        return TokenStream::from(
            syn::Error::new_spanned(
                &input.ident,
                "clap_mcp: #[clap_mcp_state_type = \"Type\"] requires \
                 #[clap_mcp_output_from_with_state = \"run\"] on the same type",
            )
            .to_compile_error(),
        );
    }

    let name = &input.ident;
    let (
        parallel_safe,
        reinvocation_safe,
        share_runtime,
        catch_in_process_panics,
        allow_mcp_without_subcommand,
        task_augmented_tools,
        stateful,
        mcp_flag,
        mcp_http_flag,
        export_skills_flag,
    ) = parse_clap_mcp_attrs(&input.attrs);
    let stateful_effective = stateful.unwrap_or(false);

    let reinvocation_effective = reinvocation_safe.unwrap_or(false);
    if task_augmented_tools == Some(true) && !reinvocation_effective {
        return TokenStream::from(
            syn::Error::new(
                proc_macro2::Span::call_site(),
                "clap_mcp: task_augmented_tools requires reinvocation_safe (in-process execution); subprocess mode cannot use MCP task-augmented tools/call",
            )
            .to_compile_error(),
        );
    }
    if output_from_with_state.is_some() && !reinvocation_effective {
        return TokenStream::from(
            syn::Error::new_spanned(
                &input.ident,
                "clap_mcp: stateful MCP tools require reinvocation_safe (in-process execution)",
            )
            .to_compile_error(),
        );
    }
    if stateful_effective && !reinvocation_effective {
        return TokenStream::from(
            syn::Error::new_spanned(
                &input.ident,
                "clap_mcp: #[clap_mcp(stateful)] requires reinvocation_safe (in-process execution)",
            )
            .to_compile_error(),
        );
    }
    if schema_only && stateful_effective {
        return TokenStream::from(
            syn::Error::new_spanned(
                &input.ident,
                "clap_mcp: #[clap_mcp(schema_only)] cannot be combined with #[clap_mcp(stateful)]",
            )
            .to_compile_error(),
        );
    }

    let parallel_safe_expr = parallel_safe
        .map(|b| quote! { #b })
        .unwrap_or_else(|| quote! { clap_mcp::ClapMcpConfig::default().parallel_safe });
    let reinvocation_safe_expr = reinvocation_safe
        .map(|b| quote! { #b })
        .unwrap_or_else(|| quote! { clap_mcp::ClapMcpConfig::default().reinvocation_safe });
    let share_runtime_expr = share_runtime
        .map(|b| quote! { #b })
        .unwrap_or_else(|| quote! { clap_mcp::ClapMcpConfig::default().share_runtime });
    let catch_in_process_panics_expr = catch_in_process_panics
        .map(|b| quote! { #b })
        .unwrap_or_else(|| quote! { clap_mcp::ClapMcpConfig::default().catch_in_process_panics });
    let allow_mcp_without_subcommand_expr = allow_mcp_without_subcommand
        .map(|b| quote! { #b })
        .unwrap_or_else(
            || quote! { clap_mcp::ClapMcpConfig::default().allow_mcp_without_subcommand },
        );

    let mut builtin_flag_stmts = Vec::new();
    if let Some(long) = mcp_flag {
        builtin_flag_stmts.push(quote! { flags = flags.with_stdio_long(#long); });
    }
    if let Some(long) = mcp_http_flag {
        builtin_flag_stmts.push(quote! { flags = flags.with_http_long(#long); });
    }
    if let Some(long) = export_skills_flag {
        builtin_flag_stmts.push(quote! { flags = flags.with_export_skills_long(#long); });
    }
    let builtin_flags_impl = if builtin_flag_stmts.is_empty() {
        quote! { clap_mcp::ClapMcpBuiltinFlags::default() }
    } else {
        quote! {{
            let mut flags = clap_mcp::ClapMcpBuiltinFlags::default();
            #(#builtin_flag_stmts)*
            flags
        }}
    };

    let config_provider = quote! {
        impl clap_mcp::ClapMcpConfigProvider for #name {
            fn clap_mcp_config() -> clap_mcp::ClapMcpConfig {
                clap_mcp::ClapMcpConfig {
                    parallel_safe: #parallel_safe_expr,
                    reinvocation_safe: #reinvocation_safe_expr,
                    share_runtime: #share_runtime_expr,
                    catch_in_process_panics: #catch_in_process_panics_expr,
                    allow_mcp_without_subcommand: #allow_mcp_without_subcommand_expr,
                    builtin_flags: #builtin_flags_impl,
                }
            }
        }
    };

    let executor_impl = match &input.data {
        syn::Data::Enum(data) => {
            if schema_only {
                quote! {}
            } else {
                let run_path = get_clap_mcp_output_from(&input.attrs);
                let run_with_state = output_from_with_state.as_ref();
                let state_ty = state_type.as_ref();
                let projected_sub = subcommand_field_type_from_enum(data);
                match (run_path, run_with_state, state_ty) {
                    (Some(run), None, None) => quote! {
                        impl clap_mcp::ClapMcpToolExecutor for #name {
                            fn execute_for_mcp(self) -> std::result::Result<clap_mcp::ClapMcpToolOutput, clap_mcp::ClapMcpToolError> {
                                clap_mcp::IntoClapMcpResult::into_tool_result(#run(self))
                            }
                        }
                    },
                    (None, Some(run), Some(st)) => quote! {
                        impl clap_mcp::ClapMcpToolExecutorWithState for #name {
                            type State = #st;
                            fn execute_for_mcp_with_state(
                                self,
                                state: &Self::State,
                            ) -> std::result::Result<clap_mcp::ClapMcpToolOutput, clap_mcp::ClapMcpToolError> {
                                clap_mcp::IntoClapMcpResult::into_tool_result(#run(self, state))
                            }
                        }
                    },
                    (None, Some(run), None) => {
                        if let Some(sub_ty) = projected_sub {
                            quote! {
                                impl clap_mcp::ClapMcpToolExecutorWithState for #name {
                                    type State = <#sub_ty as clap_mcp::ClapMcpToolExecutorWithState>::State;
                                    fn execute_for_mcp_with_state(
                                        self,
                                        state: &Self::State,
                                    ) -> std::result::Result<clap_mcp::ClapMcpToolOutput, clap_mcp::ClapMcpToolError> {
                                        clap_mcp::IntoClapMcpResult::into_tool_result(#run(self, state))
                                    }
                                }
                            }
                        } else {
                            let err = syn::Error::new_spanned(
                                &input.ident,
                                "clap_mcp: #[clap_mcp_output_from_with_state = \"run\"] on a leaf enum \
                             requires #[clap_mcp_state_type = \"Type\"] matching the second \
                             parameter of run (e.g. run(cmd, state: &Mutex<S>) → \
                             #[clap_mcp_state_type = \"Mutex<S>\"])",
                            );
                            return TokenStream::from(err.to_compile_error());
                        }
                    }
                    (Some(run), Some(run_st), Some(st)) => quote! {
                        impl clap_mcp::ClapMcpToolExecutor for #name {
                            fn execute_for_mcp(self) -> std::result::Result<clap_mcp::ClapMcpToolOutput, clap_mcp::ClapMcpToolError> {
                                clap_mcp::IntoClapMcpResult::into_tool_result(#run(self))
                            }
                        }
                        impl clap_mcp::ClapMcpToolExecutorWithState for #name {
                            type State = #st;
                            fn execute_for_mcp_with_state(
                                self,
                                state: &Self::State,
                            ) -> std::result::Result<clap_mcp::ClapMcpToolOutput, clap_mcp::ClapMcpToolError> {
                                clap_mcp::IntoClapMcpResult::into_tool_result(#run_st(self, state))
                            }
                        }
                    },
                    _ => {
                        let err = syn::Error::new_spanned(
                            &input.ident,
                            "clap_mcp: enum must have #[clap_mcp_output_from = \"run\"] and/or \
                         #[clap_mcp_output_from_with_state = \"run\"] (with \
                         #[clap_mcp_state_type = \"Type\"] on leaf enums), or \
                         #[clap_mcp(schema_only)] when an ancestor owns tool execution",
                        );
                        return TokenStream::from(err.to_compile_error());
                    }
                }
            }
        }
        syn::Data::Struct(data) => {
            let struct_run_path = get_clap_mcp_output_from(&input.attrs);
            let subcommand_field = data
                .fields
                .iter()
                .find(|f| field_has_command_subcommand(&f.attrs));
            match subcommand_field {
                Some(field) => {
                    if let Some(run) = struct_run_path {
                        if stateful_effective || output_from_with_state.is_some() {
                            let err = syn::Error::new_spanned(
                                &input.ident,
                                "clap_mcp: #[clap_mcp_output_from] on a struct root cannot be \
                                 combined with #[clap_mcp(stateful)] or \
                                 #[clap_mcp_output_from_with_state]; use subcommand delegation \
                                 or a manual ClapMcpToolExecutor instead",
                            );
                            return TokenStream::from(err.to_compile_error());
                        }
                        quote! {
                            impl clap_mcp::ClapMcpToolExecutor for #name {
                                fn execute_for_mcp(self) -> std::result::Result<clap_mcp::ClapMcpToolOutput, clap_mcp::ClapMcpToolError> {
                                    clap_mcp::IntoClapMcpResult::into_tool_result(#run(self))
                                }
                            }
                        }
                    } else {
                        let field_ident = match &field.ident {
                            Some(id) => id.clone(),
                            None => {
                                let err = syn::Error::new_spanned(
                                    field,
                                    "clap_mcp: subcommand field must be named",
                                );
                                return TokenStream::from(err.to_compile_error());
                            }
                        };
                        let body = if is_option_type(&field.ty) {
                            quote! {
                                self.#field_ident.map_or_else(
                                    || Ok(clap_mcp::ClapMcpToolOutput::Text(String::new())),
                                    |c| c.execute_for_mcp(),
                                )
                            }
                        } else {
                            quote! {
                                self.#field_ident.execute_for_mcp()
                            }
                        };
                        let state_body = if is_option_type(&field.ty) {
                            quote! {
                                self.#field_ident.map_or_else(
                                    || Ok(clap_mcp::ClapMcpToolOutput::Text(String::new())),
                                    |c| c.execute_for_mcp_with_state(state),
                                )
                            }
                        } else {
                            quote! {
                                self.#field_ident.execute_for_mcp_with_state(state)
                            }
                        };
                        let mut impls = proc_macro2::TokenStream::new();
                        if !stateful_effective {
                            impls.extend(quote! {
                            impl clap_mcp::ClapMcpToolExecutor for #name {
                                fn execute_for_mcp(self) -> std::result::Result<clap_mcp::ClapMcpToolOutput, clap_mcp::ClapMcpToolError> {
                                    #body
                                }
                            }
                        });
                        } else {
                            let sub_ty = strip_option_type(&field.ty);
                            impls.extend(quote! {
                            impl clap_mcp::ClapMcpToolExecutorWithState for #name {
                                type State = <#sub_ty as clap_mcp::ClapMcpToolExecutorWithState>::State;
                                fn execute_for_mcp_with_state(
                                    self,
                                    state: &Self::State,
                                ) -> std::result::Result<clap_mcp::ClapMcpToolOutput, clap_mcp::ClapMcpToolError> {
                                    #state_body
                                }
                            }
                        });
                        }
                        impls
                    }
                }
                None => {
                    if output_from_with_state.is_some() || stateful_effective {
                        let err = syn::Error::new_spanned(
                            &input.ident,
                            "clap_mcp: #[clap_mcp_output_from_with_state] and #[clap_mcp(stateful)] \
                             require a named subcommand field",
                        );
                        return TokenStream::from(err.to_compile_error());
                    }
                    if let Some(run) = struct_run_path {
                        quote! {
                            impl clap_mcp::ClapMcpToolExecutor for #name {
                                fn execute_for_mcp(self) -> std::result::Result<clap_mcp::ClapMcpToolOutput, clap_mcp::ClapMcpToolError> {
                                    clap_mcp::IntoClapMcpResult::into_tool_result(#run(self))
                                }
                            }
                        }
                    } else {
                        quote! {
                            impl clap_mcp::ClapMcpToolExecutor for #name {
                                fn execute_for_mcp(self) -> std::result::Result<clap_mcp::ClapMcpToolOutput, clap_mcp::ClapMcpToolError> {
                                    Ok(clap_mcp::ClapMcpToolOutput::Text(format!("{:?}", self)))
                                }
                            }
                        }
                    }
                }
            }
        }
        _ => quote! {
            impl clap_mcp::ClapMcpToolExecutor for #name {
                fn execute_for_mcp(self) -> std::result::Result<clap_mcp::ClapMcpToolOutput, clap_mcp::ClapMcpToolError> {
                    Ok(clap_mcp::ClapMcpToolOutput::Text(format!("{:?}", self)))
                }
            }
        },
    };

    let schema_metadata_impl = build_schema_metadata_impl(&input);

    let expanded = quote! {
        #config_provider
        #executor_impl
        #schema_metadata_impl
    };

    TokenStream::from(expanded)
}

/// Builds the ClapMcpSchemaMetadataProvider impl from #[clap_mcp(skip)], #[clap_mcp(requires)], and #[clap_mcp(task)].
fn serialize_topic_bindings_quote(
    target: &syn::Ident,
    bindings: &[(String, String, syn::Type)],
) -> proc_macro2::TokenStream {
    let entries = bindings.iter().map(|(cmd, arg, ty)| {
        let cmd_lit = syn::LitStr::new(cmd, proc_macro2::Span::call_site());
        let arg_lit = syn::LitStr::new(arg, proc_macro2::Span::call_site());
        quote! {
            #target.serialize_topic_args
                .entry(#cmd_lit.to_string())
                .or_default()
                .insert(
                    #arg_lit.to_string(),
                    <#ty as clap_mcp::ClapMcpSerializeTopic>::serialize_topic_segment,
                );
        }
    });
    quote! { #(#entries)* }
}

/// Builds the ClapMcpSchemaMetadataProvider impl from #[clap_mcp(skip)], #[clap_mcp(requires)], and #[clap_mcp(task)].
fn build_schema_metadata_impl(input: &DeriveInput) -> proc_macro2::TokenStream {
    let name = &input.ident;
    let (_, _, _, _, _, task_augmented_tools, _, _, _, _) = parse_clap_mcp_attrs(&input.attrs);
    let task_augmented_tools_expr = task_augmented_tools
        .map(|b| quote! { #b })
        .unwrap_or(quote! { false });
    let mut skip_commands = Vec::<String>::new();
    let mut skip_args: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    let mut requires_args: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    let mut task_tool_names = Vec::<String>::new();
    let mut serialize_tools: std::collections::HashMap<String, ClapMcpSerialized> =
        std::collections::HashMap::new();
    let mut serialize_topic_bindings: Vec<(String, String, syn::Type)> = Vec::new();
    let mut warn_optional_positional = false;

    let optional_positional_warn_block: proc_macro2::TokenStream = quote! {
        #[deprecated(note = "optional positional argument(s) without #[clap_mcp(requires)] or #[clap_mcp(skip)] may expose stdin to MCP; add one of these attributes for intentional behavior (see clap_mcp docs)")]
        fn _clap_mcp_optional_positional_warn() {}
        _clap_mcp_optional_positional_warn();
    };

    let output_schema_assign: proc_macro2::TokenStream =
        if let Some(types) = get_clap_mcp_output_one_of(&input.attrs) {
            if types.is_empty() {
                quote! {}
            } else {
                quote! { m.output_schema = clap_mcp::output_schema_one_of!(#(#types),*); }
            }
        } else if let Some(ty) = get_clap_mcp_output_type(&input.attrs) {
            quote! { m.output_schema = clap_mcp::output_schema_for_type::<#ty>(); }
        } else {
            quote! {}
        };

    match &input.data {
        syn::Data::Enum(data) => {
            for v in &data.variants {
                let cmd_name = get_command_name(&v.attrs, &v.ident);
                let variant_reqs = get_clap_mcp_requires_variant(&v.attrs).unwrap_or_default();
                if has_clap_mcp_skip(&v.attrs) {
                    skip_commands.push(cmd_name.clone());
                }
                if has_clap_mcp_task(&v.attrs) {
                    task_tool_names.push(cmd_name.clone());
                }
                if let Some(serialized) = get_clap_mcp_serialized(&v.attrs) {
                    serialize_tools.insert(cmd_name.clone(), serialized);
                }
                requires_args
                    .entry(cmd_name.clone())
                    .or_default()
                    .extend(variant_reqs.clone());
                for (i, f) in v.fields.iter().enumerate() {
                    let arg_id = f
                        .ident
                        .as_ref()
                        .map(|i| i.to_string())
                        .unwrap_or_else(|| format!("__f{i}"));
                    if is_option_type(&f.ty)
                        && field_looks_positional(&f.attrs)
                        && !has_clap_mcp_skip(&f.attrs)
                        && get_clap_mcp_requires(&f.attrs).is_none()
                        && !variant_reqs.contains(&arg_id)
                    {
                        warn_optional_positional = true;
                    }
                    if has_clap_mcp_skip(&f.attrs) {
                        skip_args
                            .entry(cmd_name.clone())
                            .or_default()
                            .push(arg_id.clone());
                    }
                    if has_clap_mcp_serialize_topic(&f.attrs) {
                        serialize_topic_bindings.push((
                            cmd_name.clone(),
                            arg_id.clone(),
                            f.ty.clone(),
                        ));
                    }
                    if let Some(req) = get_clap_mcp_requires(&f.attrs) {
                        let req_id = if req.is_empty() { arg_id } else { req };
                        requires_args
                            .entry(cmd_name.clone())
                            .or_default()
                            .push(req_id);
                    }
                }
            }
        }
        syn::Data::Struct(data) => {
            let root_name = get_command_name(&input.attrs, name);
            let subcommand_field = data
                .fields
                .iter()
                .find(|f| field_has_command_subcommand(&f.attrs));
            for f in &data.fields {
                if subcommand_field.is_some_and(|sf| std::ptr::eq(sf, f)) {
                    continue;
                }
                let Some(ref field_ident) = f.ident else {
                    continue;
                };
                let arg_id = field_ident.to_string();
                if is_option_type(&f.ty)
                    && field_looks_positional(&f.attrs)
                    && !has_clap_mcp_skip(&f.attrs)
                    && get_clap_mcp_requires(&f.attrs).is_none()
                {
                    warn_optional_positional = true;
                }
                if has_clap_mcp_skip(&f.attrs) {
                    skip_args
                        .entry(root_name.clone())
                        .or_default()
                        .push(arg_id.clone());
                }
                if let Some(req) = get_clap_mcp_requires(&f.attrs) {
                    let req_id = if req.is_empty() { arg_id } else { req };
                    requires_args
                        .entry(root_name.clone())
                        .or_default()
                        .push(req_id);
                }
            }
            if let Some(sub_field) = subcommand_field {
                let sub_ty = inner_type_if_option(&sub_field.ty).unwrap_or(&sub_field.ty);
                if let syn::Type::Path(tp) = sub_ty {
                    let sub_path = &tp.path;
                    let skip_root_assign_local =
                        if has_clap_mcp_skip_root_when_subcommands(&input.attrs) {
                            quote! { local.skip_root_command_when_subcommands = true; }
                        } else {
                            quote! {}
                        };
                    let output_schema_assign_local: proc_macro2::TokenStream = if let Some(types) =
                        get_clap_mcp_output_one_of(&input.attrs)
                    {
                        if types.is_empty() {
                            quote! {}
                        } else {
                            quote! { local.output_schema = clap_mcp::output_schema_one_of!(#(#types),*); }
                        }
                    } else if let Some(ty) = get_clap_mcp_output_type(&input.attrs) {
                        quote! { local.output_schema = clap_mcp::output_schema_for_type::<#ty>(); }
                    } else {
                        quote! {}
                    };
                    let skip_root_assign = if has_clap_mcp_skip_root_when_subcommands(&input.attrs)
                    {
                        quote! { m.skip_root_command_when_subcommands = true; }
                    } else {
                        quote! {}
                    };
                    let merge = !skip_commands.is_empty()
                        || !skip_args.is_empty()
                        || !requires_args.is_empty()
                        || !task_tool_names.is_empty()
                        || !serialize_tools.is_empty()
                        || !serialize_topic_bindings.is_empty();
                    if merge {
                        let skip_commands_lit = skip_commands.iter().map(|s| {
                            let lit = syn::LitStr::new(s, proc_macro2::Span::call_site());
                            quote! { #lit.to_string() }
                        });
                        let task_tool_names_lit = task_tool_names.iter().map(|s| {
                            let lit = syn::LitStr::new(s, proc_macro2::Span::call_site());
                            quote! { #lit.to_string() }
                        });
                        let skip_args_entries = skip_args.iter().map(|(k, v)| {
                            let k_lit = syn::LitStr::new(k, proc_macro2::Span::call_site());
                            let vs = v
                                .iter()
                                .map(|s| {
                                    let lit = syn::LitStr::new(s, proc_macro2::Span::call_site());
                                    quote! { #lit.to_string() }
                                });
                            quote! {
                                local.skip_args.entry(#k_lit.to_string()).or_default().extend([#(#vs),*]);
                            }
                        });
                        let requires_args_entries = requires_args.iter().map(|(k, v)| {
                            let k_lit = syn::LitStr::new(k, proc_macro2::Span::call_site());
                            let vs = v
                                .iter()
                                .map(|s| {
                                    let lit = syn::LitStr::new(s, proc_macro2::Span::call_site());
                                    quote! { #lit.to_string() }
                                });
                            quote! {
                                local.requires_args.entry(#k_lit.to_string()).or_default().extend([#(#vs),*]);
                            }
                        });
                        let serialize_tools_entries = serialize_tools.iter().map(|(k, scope)| {
                            let k_lit = syn::LitStr::new(k, proc_macro2::Span::call_site());
                            match scope {
                                ClapMcpSerialized::Tool => quote! {
                                    local.serialize_tools.insert(
                                        #k_lit.to_string(),
                                        clap_mcp::ClapMcpSerializeScope::Tool,
                                    );
                                },
                                ClapMcpSerialized::Args(args) => {
                                    let arg_lits = args.iter().map(|s| {
                                        let lit = syn::LitStr::new(s, proc_macro2::Span::call_site());
                                        quote! { #lit.to_string() }
                                    });
                                    quote! {
                                        local.serialize_tools.insert(
                                            #k_lit.to_string(),
                                            clap_mcp::ClapMcpSerializeScope::Args(vec![#(#arg_lits),*]),
                                        );
                                    }
                                }
                            }
                        });
                        let serialize_topic_entries = serialize_topic_bindings_quote(
                            &quote::format_ident!("local"),
                            &serialize_topic_bindings,
                        );
                        let warn_block = if warn_optional_positional {
                            optional_positional_warn_block.clone()
                        } else {
                            quote! {}
                        };
                        return quote! {
                            impl clap_mcp::ClapMcpSchemaMetadataProvider for #name {
                                fn clap_mcp_schema_metadata() -> clap_mcp::ClapMcpSchemaMetadata {
                                    #warn_block
                                    let mut m = <#sub_path as clap_mcp::ClapMcpSchemaMetadataProvider>::clap_mcp_schema_metadata();
                                    let mut local = clap_mcp::ClapMcpSchemaMetadata::default();
                                    local.skip_commands.extend([#(#skip_commands_lit),*]);
                                    local.task_tool_names.extend([#(#task_tool_names_lit),*]);
                                    local.task_augmented_tools = #task_augmented_tools_expr;
                                    #(#skip_args_entries)*
                                    #(#requires_args_entries)*
                                    #(#serialize_tools_entries)*
                                    #serialize_topic_entries
                                    #skip_root_assign_local
                                    #output_schema_assign_local
                                    m.merge_from(local);
                                    m
                                }
                            }
                        };
                    } else {
                        let warn_block = if warn_optional_positional {
                            optional_positional_warn_block.clone()
                        } else {
                            quote! {}
                        };
                        return quote! {
                            impl clap_mcp::ClapMcpSchemaMetadataProvider for #name {
                                fn clap_mcp_schema_metadata() -> clap_mcp::ClapMcpSchemaMetadata {
                                    #warn_block
                                    let mut m = <#sub_path as clap_mcp::ClapMcpSchemaMetadataProvider>::clap_mcp_schema_metadata();
                                    #skip_root_assign
                                    #output_schema_assign
                                    m
                                }
                            }
                        };
                    }
                }
            }
        }
        _ => {}
    }

    let skip_commands_lit = skip_commands.iter().map(|s| {
        let lit = syn::LitStr::new(s, proc_macro2::Span::call_site());
        quote! { #lit.to_string() }
    });
    let skip_args_entries = skip_args.iter().map(|(k, v)| {
        let k_lit = syn::LitStr::new(k, proc_macro2::Span::call_site());
        let vs = v.iter().map(|s| {
            let lit = syn::LitStr::new(s, proc_macro2::Span::call_site());
            quote! { #lit.to_string() }
        });
        quote! {
            m.skip_args.insert(#k_lit.to_string(), vec![#(#vs),*]);
        }
    });
    let requires_args_entries = requires_args.iter().map(|(k, v)| {
        let k_lit = syn::LitStr::new(k, proc_macro2::Span::call_site());
        let vs = v.iter().map(|s| {
            let lit = syn::LitStr::new(s, proc_macro2::Span::call_site());
            quote! { #lit.to_string() }
        });
        quote! {
            m.requires_args.insert(#k_lit.to_string(), vec![#(#vs),*]);
        }
    });
    let serialize_tools_entries = serialize_tools.iter().map(|(k, scope)| {
        let k_lit = syn::LitStr::new(k, proc_macro2::Span::call_site());
        match scope {
            ClapMcpSerialized::Tool => quote! {
                m.serialize_tools.insert(
                    #k_lit.to_string(),
                    clap_mcp::ClapMcpSerializeScope::Tool,
                );
            },
            ClapMcpSerialized::Args(args) => {
                let arg_lits = args.iter().map(|s| {
                    let lit = syn::LitStr::new(s, proc_macro2::Span::call_site());
                    quote! { #lit.to_string() }
                });
                quote! {
                    m.serialize_tools.insert(
                        #k_lit.to_string(),
                        clap_mcp::ClapMcpSerializeScope::Args(vec![#(#arg_lits),*]),
                    );
                }
            }
        }
    });
    let serialize_topic_entries =
        serialize_topic_bindings_quote(&quote::format_ident!("m"), &serialize_topic_bindings);
    let task_tool_names_lit = task_tool_names.iter().map(|s| {
        let lit = syn::LitStr::new(s, proc_macro2::Span::call_site());
        quote! { #lit.to_string() }
    });

    let warn_block = if warn_optional_positional {
        optional_positional_warn_block
    } else {
        quote! {}
    };

    let nested_merge_stmts = match &input.data {
        syn::Data::Enum(data) => {
            let paths = nested_subcommand_type_paths_from_enum(data);
            paths.iter().map(|p| {
                quote! { m.merge_from(<#p as clap_mcp::ClapMcpSchemaMetadataProvider>::clap_mcp_schema_metadata()); }
            }).collect::<Vec<_>>()
        }
        _ => Vec::new(),
    };

    quote! {
        impl clap_mcp::ClapMcpSchemaMetadataProvider for #name {
            fn clap_mcp_schema_metadata() -> clap_mcp::ClapMcpSchemaMetadata {
                #warn_block
                let mut m = clap_mcp::ClapMcpSchemaMetadata::default();
                #(#nested_merge_stmts)*
                m.skip_commands.extend([#(#skip_commands_lit),*]);
                m.task_tool_names.extend([#(#task_tool_names_lit),*]);
                m.task_augmented_tools = m.task_augmented_tools || #task_augmented_tools_expr;
                #(#skip_args_entries)*
                #(#requires_args_entries)*
                #(#serialize_tools_entries)*
                #serialize_topic_entries
                #output_schema_assign
                m
            }
        }
    }
}
