//! Procedural macros for clap-mcp.
//!
//! Provides `#[derive(ClapMcp)]` for attribute-based execution safety configuration
//! and `ClapMcpToolExecutor` implementation.

use proc_macro::TokenStream;
use quote::quote;
use syn::{
    DeriveInput, Expr, GenericArgument, Lit, Meta, MetaNameValue, Path, PathArguments, Type,
    parse_macro_input,
};

/// Parses `#[clap_mcp(...)]` attributes to extract parallel_safe, reinvocation_safe, and share_runtime.
fn parse_clap_mcp_attrs(attrs: &[syn::Attribute]) -> (Option<bool>, Option<bool>, Option<bool>) {
    let mut parallel_safe = None;
    let mut reinvocation_safe = None;
    let mut share_runtime = None;

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
            }
            Ok(())
        });
    }

    (parallel_safe, reinvocation_safe, share_runtime)
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

/// Parses `#[clap_mcp_output = "expr"]` from a variant's attributes.
/// The value is a string literal containing Rust code; we parse it as an Expr.
fn get_clap_mcp_output_expr(attrs: &[syn::Attribute]) -> Option<proc_macro2::TokenStream> {
    for attr in attrs {
        if !attr.path().is_ident("clap_mcp_output") {
            continue;
        }
        if let Meta::NameValue(MetaNameValue { value, .. }) = &attr.meta {
            if let Expr::Lit(lit) = value
                && let Lit::Str(s) = &lit.lit
                && let Ok(expr) = syn::parse_str::<Expr>(&s.value())
            {
                return Some(quote! { #expr });
            }
            // If it's a direct expression (not a string), use it as-is
            return Some(quote! { #value });
        }
    }
    None
}

/// Parses `#[clap_mcp_output_json = "expr"]` from a variant's attributes.
/// Single attribute for structured JSON output (replaces clap_mcp_output_type + clap_mcp_output).
fn get_clap_mcp_output_json(attrs: &[syn::Attribute]) -> Option<proc_macro2::TokenStream> {
    for attr in attrs {
        if !attr.path().is_ident("clap_mcp_output_json") {
            continue;
        }
        if let Meta::NameValue(MetaNameValue { value, .. }) = &attr.meta {
            if let Expr::Lit(lit) = value
                && let Lit::Str(s) = &lit.lit
                && let Ok(expr) = syn::parse_str::<Expr>(&s.value())
            {
                return Some(quote! { #expr });
            }
            return Some(quote! { #value });
        }
    }
    None
}

/// Parses `#[clap_mcp_output_literal = "string"]` from a variant's attributes.
/// Generates `"string".to_string()`.
fn get_clap_mcp_output_literal(attrs: &[syn::Attribute]) -> Option<String> {
    for attr in attrs {
        if !attr.path().is_ident("clap_mcp_output_literal") {
            continue;
        }
        if let Meta::NameValue(MetaNameValue { value, .. }) = &attr.meta
            && let Expr::Lit(lit) = value
            && let Lit::Str(s) = &lit.lit
        {
            return Some(s.value());
        }
    }
    None
}

/// Parses `#[clap_mcp_error_type = "TypeName"]` from a variant's attributes.
fn get_clap_mcp_error_type(attrs: &[syn::Attribute]) -> Option<syn::Ident> {
    for attr in attrs {
        if !attr.path().is_ident("clap_mcp_error_type") {
            continue;
        }
        if let Meta::NameValue(MetaNameValue { value, .. }) = &attr.meta
            && let Expr::Lit(lit) = value
            && let Lit::Str(s) = &lit.lit
            && let Ok(ident) = syn::parse_str::<syn::Ident>(&s.value())
        {
            return Some(ident);
        }
    }
    None
}

/// Returns true if the variant has `#[clap_mcp_output_result]` (expression returns Result).
fn has_clap_mcp_output_result(attrs: &[syn::Attribute]) -> bool {
    for attr in attrs {
        if attr.path().is_ident("clap_mcp_output_result") {
            return true;
        }
    }
    false
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

/// Parses #[clap_mcp(skip)] from attributes.
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

/// Derive macro for `ClapMcpConfigProvider` and `ClapMcpToolExecutor`.
///
/// Use on a clap `Parser` enum to expose it over MCP. Implements execution safety
/// config and tool output generation.
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
///
/// ## `#[clap_mcp_output_from = "run"]` (on the enum)
///
/// When present, tool execution is driven by a single function instead of per-variant attributes.
/// The value is the path to a function (e.g. `"run"` or `"my_mod::run"`) that takes the CLI type
/// by value and returns a type implementing `IntoClapMcpResult` (e.g. `String`, `AsStructured<T>`,
/// `Option<O>`, `Result<O, E>`). The macro generates `execute_for_mcp(self)` as
/// `run(self).into_tool_result()`. Per-variant output attributes are ignored when this is set.
///
/// ## `#[clap_mcp_output = "expr"]` (on each variant)
///
/// Rust expression (as a string) that produces the tool output. Use `format!(...)` for text.
/// Variant field names are in scope.
///
/// ## `#[clap_mcp_output_json = "expr"]` (on variant)
///
/// Single attribute for structured JSON output. The expression must produce a `Serialize` type.
///
/// ## `#[clap_mcp_output_literal = "string"]` (on variant)
///
/// Shorthand for constant string output. Generates `"string".to_string()`.
///
/// ## `#[clap_mcp_output_result]` (on variant, with `clap_mcp_output` or `clap_mcp_output_json`)
///
/// When present, the expression returns `Result<T, E>`. `Ok(value)` produces normal output;
/// `Err(e)` produces an MCP error response (`is_error: true`).
///
/// ## `#[clap_mcp_error_type = "TypeName"]` (on variant, with `clap_mcp_output_result`)
///
/// When present and `E: Serialize`, errors are serialized as structured JSON in the response.
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
/// ## `#[clap_mcp(skip)]` (on variant or field)
///
/// Exclude the subcommand or argument from MCP exposure.
///
/// ## `#[clap_mcp(requires)]` / `#[clap_mcp(requires = "arg_name")]` (on field)
///
/// Make the argument required in the MCP tool schema even if optional in clap.
/// Use `requires` for the field's own id, or `requires = "name"` to specify.
///
/// ## `#[clap_mcp(requires = "arg1,arg2")]` (on variant)
///
/// Variant-level alternative: comma-separated list of optional args to make required.
/// Prefer this when declaring multiple required args. When the client omits a required
/// arg, a clear error is returned.
///
/// # Example
///
/// ```rust,ignore
/// use clap::Parser;
///
/// #[derive(Debug, Parser, clap_mcp::ClapMcp)]
/// #[clap_mcp(reinvocation_safe, parallel_safe = false)]
/// enum Cli {
///     #[clap_mcp_output = "format!(\"Hello, {}!\", clap_mcp::opt_str(&name, \"world\"))"]
///     Greet { #[arg(long)] name: Option<String> },
///     #[clap_mcp_output_json = "SumResult { sum: a + b }"]
///     Add { #[arg(long)] a: i32, #[arg(long)] b: i32 },
/// }
/// ```
#[proc_macro_derive(
    ClapMcp,
    attributes(
        clap_mcp,
        clap_mcp_output,
        clap_mcp_output_from,
        clap_mcp_output_json,
        clap_mcp_output_literal,
        clap_mcp_output_result,
        clap_mcp_output_type,
        clap_mcp_output_one_of,
        clap_mcp_error_type,
        command,
        arg
    )
)]
pub fn derive_clap_mcp(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let name = &input.ident;
    let (parallel_safe, reinvocation_safe, share_runtime) = parse_clap_mcp_attrs(&input.attrs);

    let parallel_safe_expr = parallel_safe
        .map(|b| quote! { #b })
        .unwrap_or_else(|| quote! { clap_mcp::ClapMcpConfig::default().parallel_safe });
    let reinvocation_safe_expr = reinvocation_safe
        .map(|b| quote! { #b })
        .unwrap_or_else(|| quote! { clap_mcp::ClapMcpConfig::default().reinvocation_safe });
    let share_runtime_expr = share_runtime
        .map(|b| quote! { #b })
        .unwrap_or_else(|| quote! { clap_mcp::ClapMcpConfig::default().share_runtime });

    let config_provider = quote! {
        impl clap_mcp::ClapMcpConfigProvider for #name {
            fn clap_mcp_config() -> clap_mcp::ClapMcpConfig {
                clap_mcp::ClapMcpConfig {
                    parallel_safe: #parallel_safe_expr,
                    reinvocation_safe: #reinvocation_safe_expr,
                    share_runtime: #share_runtime_expr,
                }
            }
        }
    };

    let executor_impl = match &input.data {
        syn::Data::Enum(data) => {
            if let Some(run_path) = get_clap_mcp_output_from(&input.attrs) {
                quote! {
                    impl clap_mcp::ClapMcpToolExecutor for #name {
                        fn execute_for_mcp(self) -> std::result::Result<clap_mcp::ClapMcpToolOutput, clap_mcp::ClapMcpToolError> {
                            clap_mcp::IntoClapMcpResult::into_tool_result(#run_path(self))
                        }
                    }
                }
            } else {
                let arms: Vec<proc_macro2::TokenStream> = data
                    .variants
                    .iter()
                    .map(|v| {
                        let variant_name = &v.ident;
                        let (pat, output) = if v.fields.is_empty() {
                            let pat = quote! { #name::#variant_name };
                            let default_out = {
                                let kebab = ident_to_kebab(&v.ident);
                                let lit = syn::LitStr::new(&kebab, proc_macro2::Span::call_site());
                                quote! { #lit.to_string() }
                            };
                            let out = build_output_expr(v, default_out);
                            (pat, out)
                        } else {
                            let names: Vec<_> = v
                                .fields
                                .iter()
                                .enumerate()
                                .map(|(i, f)| {
                                    f.ident.as_ref().cloned().unwrap_or_else(|| {
                                        syn::Ident::new(
                                            &format!("__f{}", i),
                                            proc_macro2::Span::call_site(),
                                        )
                                    })
                                })
                                .collect();
                            let pat = quote! { #name::#variant_name { #(#names),* } };
                            let default_out = quote! { format!("{:?}", self) };
                            let out = build_output_expr(v, default_out);
                            (pat, out)
                        };
                        quote! { #pat => #output }
                    })
                    .collect();

                quote! {
                    impl clap_mcp::ClapMcpToolExecutor for #name {
                        fn execute_for_mcp(self) -> std::result::Result<clap_mcp::ClapMcpToolOutput, clap_mcp::ClapMcpToolError> {
                            match self {
                                #(#arms),*
                            }
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
            match subcommand_field {
                Some(field) => {
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
                    quote! {
                        impl clap_mcp::ClapMcpToolExecutor for #name {
                            fn execute_for_mcp(self) -> std::result::Result<clap_mcp::ClapMcpToolOutput, clap_mcp::ClapMcpToolError> {
                                #body
                            }
                        }
                    }
                }
                None => quote! {
                    impl clap_mcp::ClapMcpToolExecutor for #name {
                        fn execute_for_mcp(self) -> std::result::Result<clap_mcp::ClapMcpToolOutput, clap_mcp::ClapMcpToolError> {
                            Ok(clap_mcp::ClapMcpToolOutput::Text(format!("{:?}", self)))
                        }
                    }
                },
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

/// Builds the ClapMcpSchemaMetadataProvider impl from #[clap_mcp(skip)] and #[clap_mcp(requires)].
fn build_schema_metadata_impl(input: &DeriveInput) -> proc_macro2::TokenStream {
    let name = &input.ident;
    let mut skip_commands = Vec::<String>::new();
    let mut skip_args: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    let mut requires_args: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();

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
                if has_clap_mcp_skip(&v.attrs) {
                    skip_commands.push(cmd_name.clone());
                }
                if let Some(variant_reqs) = get_clap_mcp_requires_variant(&v.attrs) {
                    requires_args
                        .entry(cmd_name.clone())
                        .or_default()
                        .extend(variant_reqs);
                }
                for (i, f) in v.fields.iter().enumerate() {
                    let arg_id = f
                        .ident
                        .as_ref()
                        .map(|i| i.to_string())
                        .unwrap_or_else(|| format!("__f{i}"));
                    if has_clap_mcp_skip(&f.attrs) {
                        skip_args
                            .entry(cmd_name.clone())
                            .or_default()
                            .push(arg_id.clone());
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
                    let merge = !skip_commands.is_empty()
                        || !skip_args.is_empty()
                        || !requires_args.is_empty();
                    if merge {
                        let skip_commands_lit = skip_commands.iter().map(|s| {
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
                                m.skip_args.entry(#k_lit.to_string()).or_default().extend([#(#vs),*]);
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
                                m.requires_args.entry(#k_lit.to_string()).or_default().extend([#(#vs),*]);
                            }
                        });
                        return quote! {
                            impl clap_mcp::ClapMcpSchemaMetadataProvider for #name {
                                fn clap_mcp_schema_metadata() -> clap_mcp::ClapMcpSchemaMetadata {
                                    let mut m = <#sub_path as clap_mcp::ClapMcpSchemaMetadataProvider>::clap_mcp_schema_metadata();
                                    m.skip_commands.extend([#(#skip_commands_lit),*]);
                                    #(#skip_args_entries)*
                                    #(#requires_args_entries)*
                                    #output_schema_assign
                                    m
                                }
                            }
                        };
                    } else {
                        return quote! {
                            impl clap_mcp::ClapMcpSchemaMetadataProvider for #name {
                                fn clap_mcp_schema_metadata() -> clap_mcp::ClapMcpSchemaMetadata {
                                    let mut m = <#sub_path as clap_mcp::ClapMcpSchemaMetadataProvider>::clap_mcp_schema_metadata();
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

    quote! {
        impl clap_mcp::ClapMcpSchemaMetadataProvider for #name {
            fn clap_mcp_schema_metadata() -> clap_mcp::ClapMcpSchemaMetadata {
                let mut m = clap_mcp::ClapMcpSchemaMetadata::default();
                m.skip_commands.extend([#(#skip_commands_lit),*]);
                #(#skip_args_entries)*
                #(#requires_args_entries)*
                #output_schema_assign
                m
            }
        }
    }
}

/// Builds the output expression for a variant: produces `Result<ClapMcpToolOutput, ClapMcpToolError>`.
/// For normal expressions: `Ok(Text(expr))` or `Ok(Structured(...))`.
/// For `#[clap_mcp_output_result]`: `match expr { Ok(v) => Ok(...), Err(e) => Err(...) }`.
fn build_output_expr(
    v: &syn::Variant,
    default: proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    let output_expr = get_clap_mcp_output_json(&v.attrs)
        .or_else(|| {
            get_clap_mcp_output_literal(&v.attrs).map(|s| {
                let lit = syn::LitStr::new(&s, proc_macro2::Span::call_site());
                quote! { #lit.to_string() }
            })
        })
        .or_else(|| get_clap_mcp_output_expr(&v.attrs))
        .unwrap_or(default);
    let is_structured = get_clap_mcp_output_json(&v.attrs).is_some();
    let is_result = has_clap_mcp_output_result(&v.attrs);
    let error_type = get_clap_mcp_error_type(&v.attrs);

    let success_output = if is_structured {
        quote! {
            clap_mcp::ClapMcpToolOutput::Structured(::serde_json::to_value(v).expect("structured output must serialize"))
        }
    } else {
        quote! {
            clap_mcp::ClapMcpToolOutput::Text(v)
        }
    };

    if is_result {
        let err_conversion = if error_type.is_some() {
            quote! {
                clap_mcp::ClapMcpToolError::structured(
                    format!("{:?}", e),
                    ::serde_json::to_value(&e).unwrap_or_else(|_| ::serde_json::Value::String(format!("{:?}", e)))
                )
            }
        } else {
            quote! {
                clap_mcp::ClapMcpToolError::text(format!("{:?}", e))
            }
        };
        quote! {
            match #output_expr {
                Ok(v) => Ok(#success_output),
                Err(e) => Err(#err_conversion),
            }
        }
    } else {
        let normal_output = if is_structured {
            quote! {
                clap_mcp::ClapMcpToolOutput::Structured(::serde_json::to_value(#output_expr).expect("structured output must serialize"))
            }
        } else {
            quote! {
                clap_mcp::ClapMcpToolOutput::Text(#output_expr)
            }
        };
        quote! {
            Ok(#normal_output)
        }
    }
}
