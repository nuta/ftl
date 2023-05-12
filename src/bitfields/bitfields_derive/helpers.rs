use once_cell::sync::Lazy;
use proc_macro2::Span;
use proc_macro_error::abort;
use regex::Regex;
use syn::{
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
    spanned::Spanned,
    Expr, Lit, Token, Type,
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

pub fn expr_into_usize(expr: &syn::Expr) -> syn::Result<usize> {
    match expr {
        Expr::Lit(ref lit) => match &lit.lit {
            Lit::Int(int) => Ok(int.base10_parse::<usize>()?),
            _ => Err(syn::Error::new(expr.span(), "expected an integer")),
        },
        _ => Err(syn::Error::new(expr.span(), "expected integer literal")),
    }
}

static BIT_TYPE_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^B\d+$").unwrap());

/// Returns true if the given type is a bitfield type (e.g. B1, B2, B3, ...).
pub fn is_bit_type(ty: &Type) -> bool {
    let ty_ident = match ty {
        Type::Path(path) => {
            let segments = &path.path.segments;
            if segments.len() != 1 {
                abort!(ty.span(), "a field type must be a single path segment");
            }
            &segments[0].ident
        }
        _ => abort!(ty.span(), "a field type must be a single path segment"),
    };

    BIT_TYPE_REGEX.is_match(&ty_ident.to_string())
}
