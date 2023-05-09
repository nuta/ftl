use proc_macro::TokenStream;
use proc_macro_error::proc_macro_error;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

mod parser;

#[proc_macro_error]
#[proc_macro_derive(BitStruct, attributes(bitstruct))]
pub fn bitstruct(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let st = parser::Parser::new().parse(input);
    TokenStream::from(quote! {})
}
