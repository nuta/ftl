use parser::{Accessor, AttributeArgs, Definition, Field};
use proc_macro::TokenStream;
use proc_macro2::{Ident, Span};
use proc_macro_error::proc_macro_error;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

mod parser;

#[proc_macro_error]
#[proc_macro_attribute]
pub fn bitfields(args: TokenStream, item: TokenStream) -> TokenStream {
    let args_input = parse_macro_input!(args as AttributeArgs);
    let struct_input = parse_macro_input!(item as DeriveInput);
    let Definition {
        struct_name,
        struct_width,
        fields,
    } = parser::Parser::new().parse(args_input, struct_input);

    let mut methods = Vec::with_capacity(fields.len());
    let mut prev_fields = Vec::with_capacity(fields.len());
    let mut prev_types = Vec::with_capacity(fields.len());
    for Field {
        ident,
        ty,
        container_ty,
        accessor,
    } in fields
    {
        let (readable, writable) = match accessor {
            Accessor::ReadOnly => (true, false),
            Accessor::WriteOnly => (false, true),
            Accessor::ReadWrite => (true, true),
        };

        // Offset: foo_offset()
        let offset =
            Ident::new(&format!("{}_offset", ident), Span::call_site());
        if prev_fields.is_empty() {
            methods.push(quote! {
                #[inline(always)]
                pub const fn #offset() -> usize {
                    0
                }
            });
        } else {
            methods.push(quote! {
                #[inline(always)]
                pub const fn #offset() -> usize {
                    #(<#prev_types as ::bitfields::BitField>::BITS)+*
                }
            });
        }

        // Width: foo_width()
        let width = Ident::new(&format!("{}_width", ident), Span::call_site());
        methods.push(quote! {
            #[inline(always)]
            pub const fn #width() -> usize {
                #ty::BITS
            }
        });

        // Getter: foo()
        if readable {
            let getter = ident.clone();
            methods.push(quote! {
                #[inline(always)]
                pub fn #getter(&self) -> #container_ty {
                    let mask = ((1 << Self::#width()) - 1) << Self::#offset();
                    let value = (self.raw & mask) >> Self::#offset();
                    value as #container_ty
                }
            });
        }

        // Setter: set_foo()
        if writable {
            let setter =
                Ident::new(&format!("set_{}", ident), Span::call_site());

            methods.push(quote! {
                #[inline(always)]
                pub fn #setter(&mut self, value: #container_ty) {
                    debug_assert!(value < (1 << Self::#width()), concat!("value is too large for the field"));
                    self.raw |= (value as #struct_width) << Self::#offset();
                }
            });
        }

        prev_fields.push(ident);
        prev_types.push(ty);
    }

    quote! {
        struct #struct_name {
            raw: #struct_width,
        }

        impl core::default::Default for #struct_name {
            fn default() -> Self {
                Self {
                    raw: 0,
                }
            }
        }

        impl #struct_name {
            #(#methods)*
        }
    }
    .into()
}
