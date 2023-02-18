use proc_macro::TokenStream;
use proc_macro2::{Ident, Span, TokenStream as TokenStream2};
use quote::quote;
use syn::{Fields, ItemStruct};

#[proc_macro_attribute]
pub fn bitfield(_args: TokenStream, input: TokenStream) -> TokenStream {
    let ItemStruct { ident, fields, .. } = syn::parse_macro_input!(input as ItemStruct);

    // Define the specifier types.
    let mut output = (1usize..=64)
        .map(|n| {
            let ident = Ident::new(&format!("B{}", n), Span::call_site());
            quote! {
                pub enum #ident {}

                impl bitfield::Specifier for #ident {
                    const BITS: usize = #n;

                    fn get_data_range(data: &[u8], offset: usize) -> u64 {
                        data[offset].into()
                    }

                    fn set_data_range(data: &mut [u8], offset: usize, value: u64) {
                        let value = value.to_be_bytes();

                        let mut i = value.len() - 1;
                        let bound = offset + (Self::BITS / 8);
                        for bit in &mut data[offset..bound] {
                            *bit = value[i];
                            i -= 1;
                        }
                    }
                }
            }
        })
        .collect::<Vec<_>>();

    // Add the byte array representation of the input struct.
    if let Fields::Named(fields) = fields {
        let size = fields
            .named
            .iter()
            .map(|field| &field.ty)
            .map(|ty| quote!(<#ty as bitfield::Specifier>::BITS))
            .fold(quote!(0), |acc, item| quote!(#acc + #item));

        // Bits to bytes.
        let size = quote!((#size) / 8);

        output.push(quote! {
            #[repr(C)]
            pub struct #ident {
                data: [u8; #size],
            }
        });

        // Generate the getters and setters.
        let mut acc_offset = quote!(0);
        let accessors = TokenStream2::from_iter(fields.named.iter().map(|field| {
            let field_ident = field.ident.as_ref().expect("Field is named.");
            let get_ident = Ident::new(&format!("get_{}", field_ident), Span::call_site());
            let set_ident = Ident::new(&format!("set_{}", field_ident), Span::call_site());
            let field_ty = &field.ty;

            let accs = quote! {
                fn #get_ident(&self) -> u64 {
                    <#field_ty as bitfield::Specifier>::get_data_range(&self.data, #acc_offset)
                }

                fn #set_ident(&mut self, value: u64) {
                    <#field_ty as bitfield::Specifier>::set_data_range(&mut self.data, #acc_offset, value)
                }
            };
            acc_offset = quote!(#acc_offset + <#field_ty as bitfield::Specifier>::BITS);

            accs
        }));

        output.push(quote! {
            impl #ident {
                fn new() -> Self {
                    Self { data: [0; #size] }
                }

                #accessors
            }
        });
    }

    TokenStream2::from_iter(output).into()
}
