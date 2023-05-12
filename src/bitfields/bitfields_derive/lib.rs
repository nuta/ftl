use enum_support::{bitfields_enum, EnumAttr};
use helpers::AttributeArgs;
use proc_macro::TokenStream;
use proc_macro_error::{abort, proc_macro_error};
use struct_support::{bitfields_struct, StructAttr};
use syn::{parse_macro_input, spanned::Spanned, Data, DeriveInput, ItemEnum};

mod enum_support;
mod helpers;
mod struct_support;

#[proc_macro_error]
#[proc_macro_attribute]
pub fn bitfields(args: TokenStream, item: TokenStream) -> TokenStream {
    let item_cloned = item.clone();
    let item_input = parse_macro_input!(item_cloned as DeriveInput);

    match item_input.data {
        Data::Enum(_) => {
            let enum_input = parse_macro_input!(item as ItemEnum);
            let enum_name = &item_input.ident;
            let args_input =
                parse_macro_input!(args as AttributeArgs<EnumAttr>);
            bitfields_enum(enum_name, args_input, enum_input, item_input.span())
        }
        Data::Struct(ref struct_input) => {
            let struct_name = &item_input.ident;
            let args_input =
                parse_macro_input!(args as AttributeArgs<StructAttr>);
            bitfields_struct(
                struct_name,
                args_input,
                struct_input,
                item_input.span(),
            )
        }
        _ => abort!(
            item_input.span(),
            "BitStruct can only be derived for structs or enums"
        ),
    }
}
