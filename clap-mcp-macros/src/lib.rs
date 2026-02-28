//! Procedural macros for clap-mcp.
//!
//! Provides `#[derive(ClapMcp)]` for attribute-based execution safety configuration
//! and `ClapMcpToolExecutor` implementation.

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput, Expr, Lit, Meta, MetaNameValue};

/// Parses `#[clap_mcp(...)]` attributes to extract parallel_safe and reinvocation_safe.
fn parse_clap_mcp_attrs(attrs: &[syn::Attribute]) -> (Option<bool>, Option<bool>) {
    let mut parallel_safe = None;
    let mut reinvocation_safe = None;

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
            }
            Ok(())
        });
    }

    (parallel_safe, reinvocation_safe)
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

/// Parses `#[clap_mcp_output_type = "TypeName"]` from a variant's attributes.
fn get_clap_mcp_output_type(attrs: &[syn::Attribute]) -> Option<syn::Ident> {
    for attr in attrs {
        if !attr.path().is_ident("clap_mcp_output_type") {
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
///
/// ## `#[clap_mcp_output = "expr"]` (on each variant)
///
/// Rust expression (as a string) that produces the tool output. Use `format!(...)` for text.
/// Variant field names are in scope.
///
/// ## `#[clap_mcp_output_type = "TypeName"]` (on variant, with `clap_mcp_output`)
///
/// When present, output is structured JSON. The expression must produce a `Serialize` type.
///
/// # Example
///
/// ```rust,ignore
/// use clap::Parser;
///
/// #[derive(Debug, Parser, clap_mcp::ClapMcp)]
/// #[clap_mcp(reinvocation_safe, parallel_safe = false)]
/// enum Cli {
///     #[clap_mcp_output = "format!(\"Hello, {}!\", name.as_deref().unwrap_or(\"world\"))"]
///     Greet { #[arg(long)] name: Option<String> },
///     #[clap_mcp_output_type = "SumResult"]
///     #[clap_mcp_output = "SumResult { sum: a + b }"]
///     Add { #[arg(long)] a: i32, #[arg(long)] b: i32 },
/// }
/// ```
#[proc_macro_derive(ClapMcp, attributes(clap_mcp, clap_mcp_output, clap_mcp_output_type))]
pub fn derive_clap_mcp(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let name = &input.ident;
    let (parallel_safe, reinvocation_safe) = parse_clap_mcp_attrs(&input.attrs);

    let parallel_safe_expr = parallel_safe
        .map(|b| quote! { #b })
        .unwrap_or_else(|| quote! { clap_mcp::ClapMcpConfig::default().parallel_safe });
    let reinvocation_safe_expr = reinvocation_safe
        .map(|b| quote! { #b })
        .unwrap_or_else(|| quote! { clap_mcp::ClapMcpConfig::default().reinvocation_safe });

    let config_provider = quote! {
        impl clap_mcp::ClapMcpConfigProvider for #name {
            fn clap_mcp_config() -> clap_mcp::ClapMcpConfig {
                clap_mcp::ClapMcpConfig {
                    parallel_safe: #parallel_safe_expr,
                    reinvocation_safe: #reinvocation_safe_expr,
                }
            }
        }
    };

    let executor_impl = match &input.data {
        syn::Data::Enum(data) => {
            let arms: Vec<proc_macro2::TokenStream> = data
                .variants
                .iter()
                .map(|v| {
                    let variant_name = &v.ident;
                    let (pat, output) = if v.fields.is_empty() {
                        let pat = quote! { #name::#variant_name };
                        let out = build_output_expr(v, quote! { format!("{:?}", self) });
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
                    fn execute_for_mcp(self) -> clap_mcp::ClapMcpToolOutput {
                        match self {
                            #(#arms),*
                        }
                    }
                }
            }
        }
        _ => quote! {
            impl clap_mcp::ClapMcpToolExecutor for #name {
                fn execute_for_mcp(self) -> clap_mcp::ClapMcpToolOutput {
                    clap_mcp::ClapMcpToolOutput::Text(format!("{:?}", self))
                }
            }
        },
    };

    let expanded = quote! {
        #config_provider
        #executor_impl
    };

    TokenStream::from(expanded)
}

/// Builds the output expression for a variant: either Text(expr) or Structured(serde_json::to_value(expr).unwrap()).
fn build_output_expr(
    v: &syn::Variant,
    default: proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    let output_expr = get_clap_mcp_output_expr(&v.attrs).unwrap_or(default);
    if let Some(_output_type) = get_clap_mcp_output_type(&v.attrs) {
        quote! {
            clap_mcp::ClapMcpToolOutput::Structured(::serde_json::to_value(#output_expr).expect("structured output must serialize"))
        }
    } else {
        quote! {
            clap_mcp::ClapMcpToolOutput::Text(#output_expr)
        }
    }
}
