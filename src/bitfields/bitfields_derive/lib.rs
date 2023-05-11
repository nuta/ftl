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
    for Field {
        ident,
        container_ty,
        accessor,
        offset,
        width,
    } in fields
    {
        let (readable, writable) = match accessor {
            Accessor::ReadOnly => (true, false),
            Accessor::WriteOnly => (false, true),
            Accessor::ReadWrite => (true, true),
        };

        // foo()
        if readable {
            let getter = ident.clone();
            methods.push(quote! {
                #[inline(always)]
                pub fn #getter(&self) -> #container_ty {
                    let mask = ((1 << #width) - 1) << #offset;
                    let value = (self.raw & mask) >> #offset;
                    value as #container_ty
                }
            });
        }

        // set_foo()
        if writable {
            let setter =
                Ident::new(&format!("set_{}", ident), Span::call_site());

            methods.push(quote! {
                #[inline(always)]
                pub fn #setter(&mut self, value: #container_ty) {
                    debug_assert!(value < (1 << #width), concat!("value is too large for ", #width, "-bits field"));
                    self.raw |= (value as #struct_width) << #offset;
                }
            });
        }
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
