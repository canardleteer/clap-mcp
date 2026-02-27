//! Procedural macros for clap-mcp.
//!
//! Provides `#[derive(ClapMcp)]` for attribute-based execution safety configuration
//! and `ClapMcpRunnable` implementation.

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
            if let Expr::Lit(lit) = value {
                if let Lit::Str(s) = &lit.lit {
                    if let Ok(expr) = syn::parse_str::<Expr>(&s.value()) {
                        return Some(quote! { #expr });
                    }
                }
            }
            // If it's a direct expression (not a string), use it as-is
            return Some(quote! { #value });
        }
    }
    None
}

/// Derive macro for ClapMcpConfigProvider and ClapMcpRunnable.
#[proc_macro_derive(ClapMcp, attributes(clap_mcp, clap_mcp_output))]
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

    let runnable_impl = match &input.data {
        syn::Data::Enum(data) => {
            let arms: Vec<proc_macro2::TokenStream> = data
                .variants
                .iter()
                .map(|v| {
                    let variant_name = &v.ident;
                    let (pat, output) = if v.fields.is_empty() {
                        let pat = quote! { #name::#variant_name };
                        let out = get_clap_mcp_output_expr(&v.attrs)
                            .unwrap_or_else(|| quote! { format!("{:?}", self) });
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
                        let out = get_clap_mcp_output_expr(&v.attrs)
                            .unwrap_or_else(|| quote! { format!("{:?}", self) });
                        (pat, out)
                    };
                    quote! { #pat => #output }
                })
                .collect();

            quote! {
                impl clap_mcp::ClapMcpRunnable for #name {
                    fn run(self) -> String {
                        match self {
                            #(#arms),*
                        }
                    }
                }
            }
        }
        _ => quote! {
            impl clap_mcp::ClapMcpRunnable for #name {
                fn run(self) -> String {
                    format!("{:?}", self)
                }
            }
        },
    };

    let expanded = quote! {
        #config_provider
        #runnable_impl
    };

    TokenStream::from(expanded)
}
