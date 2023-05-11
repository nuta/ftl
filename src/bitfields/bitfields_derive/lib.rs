use enum_parser::{EnumArg, EnumDef, EnumParser};
use helpers::AttributeArgs;
use proc_macro::TokenStream;
use proc_macro2::{Ident, Span};
use proc_macro_error::{abort, proc_macro_error};
use quote::quote;
use struct_parser::{Accessor, Field, StructArg, StructDef, StructParser};
use syn::{
    parse_macro_input, spanned::Spanned, Data, DataEnum, DataStruct,
    DeriveInput,
};

mod enum_parser;
mod helpers;
mod struct_parser;

#[proc_macro_error]
#[proc_macro_attribute]
pub fn bitfields(args: TokenStream, item: TokenStream) -> TokenStream {
    let item_input = parse_macro_input!(item as DeriveInput);

    match item_input.data {
        Data::Enum(ref enum_input) => {
            let enum_name = &item_input.ident;
            let args_input = parse_macro_input!(args as AttributeArgs<EnumArg>);
            bitfields_enum(
                enum_name,
                args_input,
                enum_input,
                item_input.span(),
                &item_input,
            )
        }
        Data::Struct(ref struct_input) => {
            let struct_name = &item_input.ident;
            let args_input =
                parse_macro_input!(args as AttributeArgs<StructArg>);
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

fn bitfields_enum(
    enum_name: &Ident,
    args_input: AttributeArgs<EnumArg>,
    enum_input: &DataEnum,
    enum_span: Span,
    enum_ast: &DeriveInput,
) -> TokenStream {
    let EnumDef { enum_width } =
        EnumParser::new().parse(args_input, enum_input, enum_span);

    let mut patterns = Vec::with_capacity(enum_input.variants.len());
    for variant in &enum_input.variants {
        // TODO:
        patterns.push(quote! {})
    }

    let mut impls = Vec::new();
    impls.push(quote! {
        impl ::bitfields::BitField for #enum_name {
            const BITS: usize = #enum_width;
            type ContainerType = Self;
        }
    });

    if enum_width <= 8 {
        impls.push(quote! {
            impl From<#enum_name> for u8 {
                fn from(value: #enum_name) -> u8 {
                    value as u8
                }
            }

            impl From<u8> for #enum_name {
                fn from(value: u8) -> #enum_name {
                    match value {
                        // #(#patterns),*
                        _ => unreachable!(),
                    }
                }
            }
        });
    }

    if enum_width <= 16 {
        impls.push(quote! {
            impl From<#enum_name> for u16 {
                fn from(value: #enum_name) -> u16 {
                    value as u16
                }
            }

            impl From<u16> for #enum_name {
                fn from(value: u16) -> #enum_name {
                    match value {
                        // #(#patterns),*
                        _ => unreachable!(),
                    }
                }
            }
        });
    }

    if enum_width <= 32 {
        impls.push(quote! {
            impl From<#enum_name> for u32 {
                fn from(value: #enum_name) -> u32 {
                    value as u32
                }
            }

            impl From<u32> for #enum_name {
                fn from(value: u32) -> #enum_name {
                    match value {
                        // #(#patterns),*
                        _ => unreachable!(),
                    }
                }
            }
        });
    }

    impls.push(quote! {
        impl From<#enum_name> for u64 {
            fn from(value: #enum_name) -> u64 {
                value as u64
            }
        }

        impl From<u64> for #enum_name {
            fn from(value: u64) -> Self {
                match value {
                    // #(#patterns),*
                    _ => panic!("invalid value for {}: {:x}", stringify!(#enum_name), value),
                }
            }
        }
    });

    quote! {
        #enum_ast
        #(#impls)*
    }
    .into()
}

fn bitfields_struct(
    struct_name: &Ident,
    args_input: AttributeArgs<StructArg>,
    struct_input: &DataStruct,
    struct_span: Span,
) -> TokenStream {
    let StructDef {
        struct_width,
        fields,
    } = StructParser::new().parse(args_input, struct_input, struct_span);

    let mut methods = Vec::with_capacity(fields.len());
    let mut prev_fields = Vec::with_capacity(fields.len());
    let mut prev_types = Vec::with_capacity(fields.len());
    for Field {
        ident,
        ty,
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
                pub fn #getter(&self) -> <#ty as ::bitfields::BitField>::ContainerType {
                    let mask = ((1 << Self::#width()) - 1) << Self::#offset();
                    let value = (self.raw & mask) >> Self::#offset();
                    <#ty as ::bitfields::BitField>::ContainerType::from(value)
                }
            });
        }

        // Setter: set_foo()
        if writable {
            let setter =
                Ident::new(&format!("set_{}", ident), Span::call_site());

            methods.push(quote! {
                #[inline(always)]
                pub fn #setter(&mut self, value: <#ty as ::bitfields::BitField>::ContainerType) {
                    let value: #struct_width = value.into();
                    debug_assert!(value < (1 << Self::#width()), concat!("value is too large for the field"));
                    self.raw |= value << Self::#offset();
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
