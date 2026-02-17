use proc_macro::TokenStream;

#[proc_macro_attribute]
pub fn main(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut output = "#[unsafe(no_mangle)]".parse::<TokenStream>().unwrap();
    output.extend(item);
    output
}
