use proc_macro::TokenStream;
use proc_macro2::Span;
use proc_macro_error::abort;
use quote::quote;
use syn::{
    parse::{Parse, ParseStream},
    spanned::Spanned,
    Expr, Ident, ItemEnum, Token,
};

use crate::helpers::{expr_into_usize, AttributeArgs};

pub struct EnumDef {
    pub enum_width: usize,
}

pub enum EnumAttr {
    Width(usize),
}

impl Parse for EnumAttr {
    fn parse(input: ParseStream<'_>) -> syn::Result<EnumAttr> {
        let name: Ident = input.parse()?;
        let name_str = name.to_string();

        if input.peek(Token![=]) {
            // attributes with values.
            let _ = input.parse::<Token![=]>()?;

            match name_str.as_str() {
                "bits" => {
                    let expr = input.parse::<Expr>()?;
                    let bits: usize = expr_into_usize(&expr)?;
                    Ok(EnumAttr::Width(bits))
                }
                _ => abort!(name, "unknown attribute"),
            }
        } else {
            abort!(name, "unknown attribute");
        }
    }
}

fn parse(
    args: AttributeArgs<EnumAttr>,
    _enum_input: &ItemEnum,
    enum_span: Span,
) -> EnumDef {
    let mut enum_width = None;
    for arg in args.iter() {
        match arg {
            EnumAttr::Width(width) => {
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
    args_input: AttributeArgs<EnumAttr>,
    enum_input: ItemEnum,
    enum_span: Span,
) -> TokenStream {
    let EnumDef { enum_width } = parse(args_input, &enum_input, enum_span);

    let mut from_raw_patterns = Vec::with_capacity(enum_input.variants.len());
    let mut debug_patterns = Vec::with_capacity(enum_input.variants.len());
    let mut static_asserts = Vec::with_capacity(enum_input.variants.len());
    for variant in &enum_input.variants {
        match &variant.discriminant {
            Some((_, expr)) => {
                let variant = &variant.ident;
                from_raw_patterns.push(quote! {
                    (#expr) => #enum_name::#variant,
                });
                debug_patterns.push(quote! {
                    #enum_name::#variant => { write!(f, stringify!(#variant))?; },
                });

                let enum_width_str = enum_width.to_string();
                static_asserts.push(quote! {
                    const _: () = assert!(
                        (#expr as usize) < (1usize << #enum_width),
                        concat!(
                            "discriminant of ",
                            stringify!(#enum_name),
                            "::",
                            stringify!(#variant),
                            " is out of range for ",
                            #enum_width_str,
                            "-bit enum",
                        )
                    );
                });
            }
            None => {
                abort!(variant.span(), "each variant must have a discriminant");
            }
        }
    }

    if from_raw_patterns.len() != 1 << enum_width {
        abort!(
            enum_span,
            "enum must be exhaustive (expect {} more possible patterns)",
            (1 << enum_width) - from_raw_patterns.len()
        );
    }

    // The enum is exhaustive (assuming Rust will deny multiple variants that
    // have the same value). It should be safe to use `unreachable_unchecked()` here.
    from_raw_patterns.push(quote! {
        _ => unsafe { ::core::hint::unreachable_unchecked() },
    });

    quote! {
        #enum_input
        #(#static_asserts)*

        impl ::bitfields::BitField for #enum_name {
            const BITS: usize = #enum_width;
            type AccessorValueType = Self;

            fn from_u64(value: u64) -> Self {
                match value {
                    #(#from_raw_patterns)*
                }
            }

            fn into_u64(value: #enum_name) -> u64 {
                let raw = value as u64;
                debug_assert!(raw < (1 << <#enum_name as ::bitfields::BitField>::BITS as u64));
                raw
            }
        }

        impl ::core::fmt::Debug for #enum_name {
            fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
                write!(f, "{}::", stringify!(#enum_name))?;
                match self {
                    #(#debug_patterns)*
                }
                Ok(())
            }
        }
    }
    .into()
}
