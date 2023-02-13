use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use std::collections::HashSet;
use syn::{
    AngleBracketedGenericArguments, Error, Fields, FieldsNamed, GenericArgument, ItemStruct, Meta,
    MetaList, MetaNameValue, NestedMeta, PathArguments, PathSegment, Type, TypePath,
};

#[proc_macro_derive(CustomDebug, attributes(debug))]
pub fn derive(input: TokenStream) -> TokenStream {
    // Parse the input stream.
    let ItemStruct {
        ident,
        fields,
        mut generics,
        attrs,
        ..
    } = syn::parse_macro_input!(input as syn::ItemStruct);

    let mut generics_clone = generics.clone();
    let where_clause = generics_clone.make_where_clause();

    // Check for a `debug(bound = "...")` attribute.
    let bound_attr = attrs.into_iter().find_map(|attr| {
        if let Ok(Meta::List(MetaList { path, nested, .. })) = attr.parse_meta() {
            if path.is_ident("debug") {
                if let Some(NestedMeta::Meta(Meta::NameValue(MetaNameValue {
                    path, lit, ..
                }))) = nested.first()
                {
                    if path.is_ident("bound") {
                        if let syn::Lit::Str(bound) = lit {
                            return Some(bound.value());
                        }
                    }
                }
            }
        }

        None
    });

    if let Fields::Named(FieldsNamed { named, .. }) = fields {
        let ident_str = ident.to_string();

        // Keep track of which type parameters don't need a trait bound.
        let mut unbounded_type_params = HashSet::new();

        let fmt_fields = TokenStream2::from_iter(named.into_iter().map(|field| {
            if let Type::Path(TypePath { path, .. }) = field.ty {
                if let Some(PathSegment {
                    ident, arguments, ..
                }) = path.segments.first()
                {
                    if let PathArguments::AngleBracketed(AngleBracketedGenericArguments {
                        args,
                        ..
                    }) = arguments
                    {
                        if let Some(GenericArgument::Type(Type::Path(TypePath { path, .. }))) =
                            args.first()
                        {
                            // Check if this is a PhantomData field.
                            if ident == "PhantomData" {
                                if let Some(ident) = path.get_ident() {
                                    unbounded_type_params.insert(ident.clone());
                                }
                            }
                            // Check for associated types.
                            else if path.segments.iter().count() > 1 {
                                let type_ident = &path
                                    .segments
                                    .first()
                                    .expect("Path should have multiple segments")
                                    .ident;
                                if generics
                                    .type_params()
                                    .any(|param| param.ident == *type_ident)
                                {
                                    unbounded_type_params.insert(type_ident.clone());

                                    // Add trait bound to the associated type.
                                    where_clause
                                        .predicates
                                        .push(syn::parse_quote!(#path: std::fmt::Debug));
                                }
                            }
                        }
                    }
                }
            }

            // Check if we need to use a specific format.
            let format_string = field.attrs.into_iter().find_map(|attr| {
                if let Ok(Meta::NameValue(MetaNameValue { path, lit, .. })) = attr.parse_meta() {
                    if path.is_ident("debug") {
                        if let syn::Lit::Str(lit_str) = lit {
                            return Some(lit_str.value());
                        }
                    }
                }

                None
            });

            let name = field.ident.expect("Field should have a name.");
            let name_str = name.to_string();
            let value = if let Some(format_string) = format_string {
                quote!(&std::format_args!(#format_string, &self.#name))
            } else {
                quote!(&self.#name)
            };

            quote!(.field(#name_str, #value))
        }));

        if let Some(bound_attr) = bound_attr {
            where_clause.predicates.clear();
            where_clause
                .predicates
                .push(syn::parse_str(&bound_attr).expect("Value should be a valid trait bound."));
        } else {
            // Make sure non-phantom type params implement Debug.
            for param in generics.type_params_mut() {
                if !unbounded_type_params.contains(&param.ident) {
                    param.bounds.push(syn::parse_quote!(std::fmt::Debug));
                }
            }
        }

        let (impl_generics, ty_generics, _) = generics.split_for_impl();

        // Generate the Debug implementation.
        let debug_impl = quote! {
            impl #impl_generics std::fmt::Debug for #ident #ty_generics #where_clause {
                fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
                    fmt.debug_struct(#ident_str)
                        #fmt_fields
                        .finish()
                }
            }
        };

        debug_impl.into()
    } else {
        return Error::new(
            ident.span(),
            "Only structs with named fields are supported.",
        )
        .to_compile_error()
        .into();
    }
}
