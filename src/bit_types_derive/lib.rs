use parser::{AttributeArgs, Definition};
use proc_macro::TokenStream;
use proc_macro_error::proc_macro_error;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

mod parser;

#[proc_macro_error]
#[proc_macro_attribute]
pub fn bit_struct(args: TokenStream, item: TokenStream) -> TokenStream {
    let args_input = parse_macro_input!(args as AttributeArgs);
    let struct_input = parse_macro_input!(item as DeriveInput);
    let Definition {
        struct_name,
        struct_width,
        fields,
    } = parser::Parser::new().parse(args_input, struct_input);

    quote! {
        struct #struct_name {
            raw: #struct_width,
        }
    }
    .into()
}
