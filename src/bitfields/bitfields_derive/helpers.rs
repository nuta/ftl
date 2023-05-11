use proc_macro2::Span;
use syn::{
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
    spanned::Spanned,
    Expr, Lit, Token,
};

/// Since syn v2.x, AttributeArgs got removed. Roll our own.
pub struct AttributeArgs<T> {
    span: Span,
    args: Punctuated<T, Token![,]>,
}

impl<T: Parse> AttributeArgs<T> {
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.args.iter()
    }

    pub fn span(&self) -> Span {
        self.span
    }
}

impl<T: Parse> Parse for AttributeArgs<T> {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        Ok(Self {
            span: input.span(),
            args: Punctuated::parse_terminated(input)?,
        })
    }
}

pub fn expr_into_usize(expr: &Option<Box<syn::Expr>>) -> syn::Result<usize> {
    match expr {
        Some(expr) => match **expr {
            Expr::Lit(ref lit) => match &lit.lit {
                Lit::Int(int) => Ok(int.base10_parse::<usize>()?),
                _ => Err(syn::Error::new(expr.span(), "expected an integer")),
            },
            _ => Err(syn::Error::new(expr.span(), "expected integer literal")),
        },
        _ => Err(syn::Error::new(expr.span(), "expected a range")),
    }
}
