use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::ToTokens;
use syn::{
    spanned::Spanned,
    visit_mut::{self, VisitMut},
    AttributeArgs, Error, ExprMatch, Item, ItemFn, Pat, PatIdent, PatTupleStruct,
};

struct Checker {
    errors: Vec<TokenStream2>,
}

impl VisitMut for Checker {
    fn visit_expr_match_mut(&mut self, expr: &mut ExprMatch) {
        if expr.attrs.iter().any(|attr| attr.path.is_ident("sorted")) {
            expr.attrs.retain(|attr| !attr.path.is_ident("sorted"));

            let mut sorted = Vec::with_capacity(expr.arms.len());

            for (i, pat) in expr.arms.iter().map(|arm| &arm.pat).enumerate() {
                let (ident, span) = match pat {
                    Pat::Ident(PatIdent { ident, .. }) => (ident.to_string(), ident.span()),
                    Pat::TupleStruct(PatTupleStruct { path, .. }) => {
                        if let Some(ident) = path.get_ident() {
                            (ident.to_string(), ident.span())
                        } else if path.segments.len() == 2 {
                            let (first, last) = (&path.segments[0].ident, &path.segments[1].ident);
                            (format!("{}::{}", first, last), path.span())
                        } else {
                            break;
                        }
                    }
                    Pat::Wild(_) if i == expr.arms.len() - 1 => break,
                    _ => {
                        self.errors
                            .push(error(pat.span(), "unsupported by #[sorted]".into()).into());
                        break;
                    }
                };

                sorted_insert(&mut sorted, ident, span, &mut self.errors);
            }
        }

        visit_mut::visit_expr_match_mut(self, expr);
    }
}

#[proc_macro_attribute]
pub fn sorted(args: TokenStream, input: TokenStream) -> TokenStream {
    let _args = syn::parse_macro_input!(args as AttributeArgs);
    let input = syn::parse_macro_input!(input as Item);

    if let Item::Enum(input) = input {
        let mut sorted = Vec::with_capacity(input.variants.len());
        let mut output = vec![input.clone().into_token_stream()];

        for (ident, span) in input
            .variants
            .into_iter()
            .map(|variant| (variant.ident.to_string(), variant.ident.span()))
        {
            sorted_insert(&mut sorted, ident, span, &mut output);
        }

        TokenStream2::from_iter(output.into_iter()).into()
    } else {
        error(
            Span::call_site(),
            "expected enum or match expression".into(),
        )
    }
}

#[proc_macro_attribute]
pub fn check(_args: TokenStream, input: TokenStream) -> TokenStream {
    let mut input = syn::parse_macro_input!(input as ItemFn);

    let mut checker = Checker { errors: vec![] };
    checker.visit_item_fn_mut(&mut input);

    let mut output = checker.errors;
    output.push(input.to_token_stream());
    TokenStream2::from_iter(output).into()
}

fn error(span: Span, msg: String) -> TokenStream {
    Error::new(span, msg).into_compile_error().into()
}

fn sorted_insert(
    sorted: &mut Vec<String>,
    ident: String,
    ident_span: Span,
    errors: &mut Vec<TokenStream2>,
) {
    let idx = sorted.partition_point(|v: &String| v.cmp(&ident).is_lt());
    if idx != sorted.len() {
        errors.push(
            error(
                ident_span,
                format!("{} should sort before {}", ident, sorted[idx]),
            )
            .into(),
        );
    } else {
        sorted.push(ident);
    }
}
