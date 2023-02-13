use proc_macro::TokenStream;
use proc_macro2::{Ident, TokenStream as TokenStream2};
use quote::quote;
use syn::{
    spanned::Spanned, AngleBracketedGenericArguments, Error, Expr, ExprAssign, ExprLit, ExprPath,
    FieldsNamed, GenericArgument, ItemStruct, Lit, PathArguments, Type, TypePath,
};

struct FieldInfo<'a> {
    ident: &'a Ident,
    ty: &'a Type,
    is_optional: bool,
    builder_attr_ident: Option<Ident>,
}

fn get_inner_type<'a>(ty: &'a Type, outer_ident: &str) -> Option<&'a Type> {
    if let Type::Path(TypePath { path, .. }) = ty {
        if let Some(segment) = path.segments.first() {
            if segment.ident == outer_ident {
                if let PathArguments::AngleBracketed(AngleBracketedGenericArguments {
                    args, ..
                }) = &segment.arguments
                {
                    if let Some(GenericArgument::Type(inner_ty)) = args.first() {
                        return Some(inner_ty);
                    }
                }
            }
        }
    }

    None
}

fn field_setters(info: &FieldInfo) -> TokenStream2 {
    let ident = info.ident;
    let ty = info.ty;

    let mut one_at_a_time = TokenStream2::new();
    let mut all_at_once = quote! {
        fn #ident(&mut self, value: #ty) -> &mut Self {
            self.#ident = Some(value);
            self
        }
    };

    if let Some(builder_attr_ident) = &info.builder_attr_ident {
        if let Some(inner_ty) = get_inner_type(ty, "Vec") {
            one_at_a_time = quote! {
                fn #builder_attr_ident(&mut self, value: #inner_ty) -> &mut Self {
                    self.#ident.get_or_insert_with(Vec::new).push(value);
                    self
                }
            };
        }

        if builder_attr_ident.to_string() == ident.to_string() {
            all_at_once = TokenStream2::new();
        }
    }

    quote! {
        #one_at_a_time

        #all_at_once
    }
}

#[proc_macro_derive(Builder, attributes(builder))]
pub fn derive(input: TokenStream) -> TokenStream {
    let ItemStruct { ident, fields, .. } = syn::parse_macro_input!(input as syn::ItemStruct);

    if let syn::Fields::Named(FieldsNamed { named, .. }) = fields {
        // Construct the field information.
        let mut field_info = Vec::with_capacity(named.pairs().len());
        for pair in named.pairs().into_iter() {
            let field = pair.value();
            let mut info = FieldInfo {
                ident: field.ident.as_ref().expect("Field should be named."),
                ty: &field.ty,
                is_optional: false,
                builder_attr_ident: None,
            };

            for attr in field.attrs.iter() {
                if let Some(ident) = attr.path.get_ident() {
                    if ident == "builder" {
                        if let Ok(ExprAssign { left, right, .. }) = attr.parse_args::<ExprAssign>()
                        {
                            if let Expr::Path(ExprPath { ref path, .. }) = *left {
                                if !path.is_ident("each") {
                                    return Error::new(
                                        attr.parse_meta().unwrap().span(),
                                        "expected `builder(each = \"...\")`",
                                    )
                                    .into_compile_error()
                                    .into();
                                }

                                if let Expr::Lit(ExprLit { lit, .. }) = *right {
                                    if let Lit::Str(lit_str) = lit {
                                        info.builder_attr_ident =
                                            Some(quote::format_ident!("{}", lit_str.value()));
                                    }
                                }
                            }
                        }
                    }
                }
            }

            if let Some(inner_ty) = get_inner_type(&field.ty, "Option") {
                info.ty = inner_ty;
                info.is_optional = true;
            }

            field_info.push(info);
        }

        // Fields for the builder struct.
        let builder_fields = TokenStream2::from_iter(
            field_info
                .iter()
                .map(|FieldInfo { ident, ty, .. }| quote!(#ident: core::option::Option<#ty>,)),
        );

        // Initial builder values.
        let builder_defaults = TokenStream2::from_iter(
            field_info
                .iter()
                .map(|FieldInfo { ident, .. }| quote!(#ident: None,)),
        );

        // Builder setters.
        let setters = TokenStream2::from_iter(field_info.iter().map(field_setters));

        // Body of the build() method.
        let build_body = TokenStream2::from_iter(field_info.iter().map(
            |FieldInfo {
                 ident,
                 is_optional,
                 builder_attr_ident,
                 ..
             }| {
                let value_suffix = if *is_optional {
                    quote!()
                } else if builder_attr_ident.is_some() {
                    quote!(.unwrap_or_default())
                } else {
                    let field_name = ident.to_string();
                    quote!(.expect(&format!("{} should have a value.", #field_name)))
                };

                quote!(#ident: self.#ident.take()#value_suffix,)
            },
        ));

        let builder_ident = quote::format_ident!("{}Builder", ident);
        let implementation = quote! {
            impl #ident {
                pub fn builder() -> #builder_ident {
                    #builder_ident {
                        #builder_defaults
                    }
                }
            }

            pub struct #builder_ident {
                #builder_fields
            }

            impl #builder_ident {
                #setters

                pub fn build(&mut self) -> std::result::Result<#ident, std::boxed::Box<dyn std::error::Error>> {
                    Ok(#ident {
                        #build_body
                    })
                }
            }
        };

        implementation.into()
    } else {
        return Error::new(ident.span(), "#[derive(Builder)] requires named fields.")
            .into_compile_error()
            .into();
    }
}
