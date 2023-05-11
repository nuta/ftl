use proc_macro2::Span;
use proc_macro_error::abort;
use syn::{
    parse::{Parse, ParseStream},
    DataEnum, Expr, Ident, Token,
};

use crate::helpers::{parse_expr_lit, AttributeArgs};

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
                    let bits: usize = parse_expr_lit(&Some(Box::new(expr)))?; // FIXME: No Option
                    Ok(EnumArg::Width(bits))
                }
                _ => abort!(name, "unknown attribute"),
            }
        } else {
            abort!(name, "unknown attribute");
        }
    }
}

pub struct EnumParser {}

impl EnumParser {
    pub fn new() -> Self {
        Self {}
    }

    pub fn parse(
        self,
        args: AttributeArgs<EnumArg>,
        _enum_input: &DataEnum,
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
}
