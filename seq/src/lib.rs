use proc_macro::TokenStream;
use proc_macro2::{Delimiter, Group, Literal, TokenStream as TokenStream2, TokenTree};
use std::ops::Range;
use syn::{
    parse::{Parse, ParseStream},
    Ident, LitInt, Result, Token,
};

struct Sequence {
    ident: Ident,
    range: Range<usize>,
    content: TokenStream2,
}

impl Parse for Sequence {
    fn parse(input: ParseStream) -> Result<Self> {
        let ident = input.parse()?;
        input.parse::<Token![in]>()?;
        let start = input.parse::<LitInt>()?;
        let start = start.base10_parse::<usize>()?;
        input.parse::<Token![..]>()?;
        let lookahead = input.lookahead1();
        let end = if lookahead.peek(Token![=]) {
            input.parse::<Token![=]>()?;
            let end = input.parse::<LitInt>()?;
            end.base10_parse::<usize>().map(|end| end + 1)?
        } else {
            let end = input.parse::<LitInt>()?;
            end.base10_parse::<usize>()?
        };
        let content;
        syn::braced!(content in input);
        let content = content.parse::<TokenStream2>()?;

        Ok(Sequence {
            ident,
            range: start..end,
            content,
        })
    }
}

#[proc_macro]
pub fn seq(input: TokenStream) -> TokenStream {
    let Sequence {
        ident,
        range,
        content,
    } = syn::parse_macro_input!(input as Sequence);
    let original = content.clone();

    let (mut sequence, has_section) = parse_content(content, &ident, &range);

    // If there is no repetition section, repeat the whole content.
    if !has_section {
        sequence.clear();
        repeat_section(original, &range, &ident, &mut sequence);
    }

    TokenStream2::from_iter(sequence).into()
}

fn parse_content(
    content: TokenStream2,
    n_ident: &Ident,
    range: &Range<usize>,
) -> (Vec<TokenTree>, bool) {
    let mut result = Vec::new();
    let mut has_section = false;

    let content = content.into_iter().collect::<Vec<_>>();
    let mut i = 0;
    while i < content.len() {
        // Check if we have a repetition section.
        if i + 2 < content.len() {
            if let (
                TokenTree::Punct(start_punct),
                TokenTree::Group(group),
                TokenTree::Punct(end_punct),
            ) = (&content[i], &content[i + 1], &content[i + 2])
            {
                if start_punct.as_char() == '#'
                    && end_punct.as_char() == '*'
                    && group.delimiter() == Delimiter::Parenthesis
                {
                    has_section = true;
                    repeat_section(group.stream(), range, n_ident, &mut result);
                    i += 3;

                    continue;
                }
            }
        }
        // Recurse with groups.
        if let TokenTree::Group(group) = &content[i] {
            let (content, group_has_section) = parse_content(group.stream(), n_ident, range);
            has_section = has_section || group_has_section;
            let content = TokenStream2::from_iter(content);
            let mut group_tree = TokenTree::from(Group::new(group.delimiter(), content));
            group_tree.set_span(group.span());
            result.push(group_tree);
        } else {
            result.push(content[i].clone());
        }

        i += 1;
    }

    (result, has_section)
}

fn repeat_section(
    content: TokenStream2,
    range: &Range<usize>,
    n_ident: &Ident,
    result: &mut Vec<TokenTree>,
) {
    for n in range.clone().into_iter() {
        let mut section = section(content.clone(), n_ident, n);
        result.append(&mut section);
    }
}

fn section(content: TokenStream2, n_ident: &Ident, n: usize) -> Vec<TokenTree> {
    let mut result = Vec::new();

    let content = content.into_iter().collect::<Vec<_>>();
    let mut i = 0;
    while i < content.len() {
        let transformed = match &content[i] {
            TokenTree::Ident(ident) => {
                if ident == n_ident {
                    // Replace the identifier by loop counter.
                    TokenTree::Literal(Literal::usize_unsuffixed(n))
                } else {
                    // Check if we have a ~N sequence.
                    let mut ident = ident.clone();
                    if i + 2 < content.len() {
                        if let (TokenTree::Punct(punct), TokenTree::Ident(next_indent)) =
                            (&content[i + 1], &content[i + 2])
                        {
                            if punct.as_char() == '~' && next_indent == n_ident {
                                i += 2;
                                ident = Ident::new(&format!("{}{}", ident, n), ident.span());
                            }
                        }
                    }
                    TokenTree::Ident(ident)
                }
            }
            // Recurse with groups.
            TokenTree::Group(group) => {
                let content = section(group.stream(), n_ident, n);
                let content = TokenStream2::from_iter(content);
                let mut group_tree = TokenTree::from(Group::new(group.delimiter(), content));
                group_tree.set_span(group.span());
                group_tree
            }
            token_tree => token_tree.clone(),
        };

        result.push(transformed);
        i += 1;
    }

    result
}
