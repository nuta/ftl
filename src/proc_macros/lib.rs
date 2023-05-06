use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Ident, ItemFn};

#[proc_macro_attribute]
pub fn test(_args: TokenStream, input: TokenStream) -> TokenStream {
    let func = parse_macro_input!(input as ItemFn);
    let func_name = &func.sig.ident;
    let func_block = &func.block;

    let first_ident = match func.sig.inputs.first() {
        Some(syn::FnArg::Receiver(_)) => unimplemented!(),
        Some(syn::FnArg::Typed(typed)) => match &*typed.pat {
            syn::Pat::Ident(pat_ident) => pat_ident.ident.clone(),
            _ => unimplemented!(),
        },
        None => Ident::new("__testing__", proc_macro2::Span::call_site()),
    };

    let output = quote! {
        #[test_case]
        fn #func_name(#first_ident: &mut crate::test::Testing) -> () {
            (#first_ident).set_name(stringify!(#func_name));
            #func_block
        }
    };

    TokenStream::from(output)
}
