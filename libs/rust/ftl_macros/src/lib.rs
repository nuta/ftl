use proc_macro::TokenStream;
use quote::quote;
use syn::ItemFn;
use syn::ReturnType;
use syn::parse_macro_input;

#[proc_macro_attribute]
pub fn main(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);
    do_main(input)
        .unwrap_or_else(|error| error.to_compile_error())
        .into()
}

fn do_main(input: ItemFn) -> syn::Result<proc_macro2::TokenStream> {
    let ItemFn {
        attrs,
        vis,
        mut sig,
        block,
    } = input;

    if sig.ident != "main" {
        return Err(syn::Error::new_spanned(
            &sig.ident,
            "`#[ftl::main]` must be applied to a function named `main`",
        ));
    }

    if !matches!(sig.output, ReturnType::Default) {
        return Err(syn::Error::new_spanned(
            &sig.output,
            "`#[ftl::main]` entry points must not return a value",
        ));
    }

    let is_async = sig.asyncness.take().is_some();
    if is_async {
        Ok(quote! {
            #(#attrs)*
            #[unsafe(no_mangle)]
            #vis #sig {
                ::ftl::aio::run(async move #block);
            }
        })
    } else {
        Ok(quote! {
            #(#attrs)*
            #[unsafe(no_mangle)]
            #vis #sig #block
        })
    }
}
