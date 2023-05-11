use proc_macro::TokenStream;
use proc_macro2::Span;
use proc_macro_error::abort;
use quote::quote;
use syn::{
    parse::{Parse, ParseStream},
    spanned::Spanned,
    Expr, Ident, ItemEnum, Token, Variant,
};

use crate::helpers::{expr_into_usize, AttributeArgs};

pub struct EnumDef {
    pub enum_width: usize,
}

pub enum EnumArg {
    Width(usize),
}

impl Parse for EnumArg {
    fn parse(input: ParseStream<'_>) -> syn::Result<EnumArg> {
        let name: Ident = input.parse()?;
        let name_str = name.to_string();

        if input.peek(Token![=]) {
            // attributes with values.
            let _ = input.parse::<Token![=]>()?;

            match name_str.as_str() {
                "bits" => {
                    let expr = input.parse::<Expr>()?;
                    let bits: usize = expr_into_usize(&expr)?;
                    Ok(EnumArg::Width(bits))
                }
                _ => abort!(name, "unknown attribute"),
            }
        } else {
            abort!(name, "unknown attribute");
        }
    }
}

fn parse(
    args: AttributeArgs<EnumArg>,
    _enum_input: &ItemEnum,
    enum_span: Span,
) -> EnumDef {
    let mut enum_width = None;
    for arg in args.iter() {
        match arg {
            EnumArg::Width(width) => {
                enum_width = Some(*width);
            }
        }
    }

    let enum_width = match enum_width {
        Some(width) if width == 0 => {
            abort!(enum_span, "zero-width enum is not allowed");
        }
        Some(width) if width > 64 => {
            abort!(
                enum_span,
                "enum width must be less than or equal to 64 bits"
            );
        }
        Some(width) => width,
        None => {
            abort!(enum_span, "missing `bits` attribute");
        }
    };

    EnumDef { enum_width }
}

pub fn bitfields_enum(
    enum_name: &Ident,
    args_input: AttributeArgs<EnumArg>,
    mut enum_input: ItemEnum,
    enum_span: Span,
) -> TokenStream {
    let EnumDef { enum_width } = parse(args_input, &enum_input, enum_span);

    let mut from_raw_patterns = Vec::with_capacity(enum_input.variants.len());
    for variant in &enum_input.variants {
        match &variant.discriminant {
            Some((_, expr)) => {
                let variant = &variant.ident;
                from_raw_patterns
                    .push(quote! { (#expr) => #enum_name::#variant, });
            }
            None => {
                abort!(variant.span(), "each variant must have a discriminant");
            }
        }
    }

    if from_raw_patterns.len() == 1 << enum_width {
        // The enum is not exhaustive (assuming Rust will deny multiple variants that
        // have the same value). It should be safe to use `unreachable_unchecked()` here.
        from_raw_patterns.push(quote! {
            _ => unsafe { ::core::hint::unreachable_unchecked() },
        });
    } else {
        // The enum is not exhaustive. We need a catch-all pattern.
        enum_input.variants.push(Variant {
            ident: Ident::new("__NonExhaustive", Span::call_site()),
            attrs: Vec::new(),
            fields: syn::Fields::Unit,
            discriminant: None,
        });

        from_raw_patterns.push(quote! {
            _ => { #enum_name::__NonExhaustive },
        });
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
                    let raw = value as u8;
                    debug_assert!(raw < (1 << <#enum_name as ::bitfields::BitField>::BITS as u8));
                    raw
                }
            }

            impl From<u8> for #enum_name {
                fn from(value: u8) -> #enum_name {
                    debug_assert!(value < (1 << <#enum_name as ::bitfields::BitField>::BITS as u8));
                    match value {
                        #(#from_raw_patterns)*
                    }
                }
            }
        });
    }

    if enum_width <= 16 {
        impls.push(quote! {
            impl From<#enum_name> for u16 {
                fn from(value: #enum_name) -> u16 {
                    let raw = value as u16;
                    debug_assert!(raw < (1 << <#enum_name as ::bitfields::BitField>::BITS as u16));
                    raw
                }
            }

            impl From<u16> for #enum_name {
                fn from(value: u16) -> #enum_name {
                    debug_assert!(value < (1 << <#enum_name as ::bitfields::BitField>::BITS as u16));
                    match value {
                        #(#from_raw_patterns)*
                    }
                }
            }
        });
    }

    if enum_width <= 32 {
        impls.push(quote! {
            impl From<#enum_name> for u32 {
                fn from(value: #enum_name) -> u32 {
                    let raw = value as u32;
                    debug_assert!(raw < (1 << <#enum_name as ::bitfields::BitField>::BITS as u32));
                    raw
                }
            }

            impl From<u32> for #enum_name {
                fn from(value: u32) -> #enum_name {
                    debug_assert!(value < (1 << <#enum_name as ::bitfields::BitField>::BITS as u32));
                    match value {
                        #(#from_raw_patterns)*
                    }
                }
            }
        });
    }

    impls.push(quote! {
        impl From<#enum_name> for u64 {
            fn from(value: #enum_name) -> u64 {
                let raw = value as u64;
                debug_assert!(raw < (1 << <#enum_name as ::bitfields::BitField>::BITS as u64));
                raw
            }
        }

        impl From<u64> for #enum_name {
            fn from(value: u64) -> Self {
                match value {
                    #(#from_raw_patterns)*
                    _ => panic!("invalid value for {}: {:x}", stringify!(#enum_name), value),
                }
            }
        }
    });

    quote! {
        #enum_input
        #(#impls)*
    }
    .into()
}
