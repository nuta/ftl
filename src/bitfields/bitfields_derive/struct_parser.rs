use std::ops::RangeInclusive;

use proc_macro2::Span;
use proc_macro_error::abort;
use syn::{
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
    spanned::Spanned,
    DataStruct, ExprRange, Fields, FieldsNamed, Ident, RangeLimits, Token,
    Type,
};

use crate::helpers::{parse_expr_lit, AttributeArgs};

pub struct Field {
    pub ident: Ident,
    pub ty: Type,
    pub accessor: Accessor,
}

pub struct StructDef {
    pub struct_width: Ident,
    pub fields: Vec<Field>,
}

#[derive(Debug)]
struct ArgsDefinition {
    struct_width: Option<usize>,
}

// `width` in #[bitfields(width = 32)]
#[derive(Debug)]
pub enum StructArg {
    Width(usize),
}

impl Parse for StructArg {
    fn parse(input: ParseStream<'_>) -> syn::Result<StructArg> {
        let name: Ident = input.parse()?;
        let name_str = name.to_string();

        if input.peek(Token![=]) {
            abort!(name, "unknown attribute");
        } else {
            // attributes without values.
            match name_str.as_str() {
                "u8" => Ok(StructArg::Width(8)),
                "u16" => Ok(StructArg::Width(16)),
                "u32" => Ok(StructArg::Width(32)),
                "u64" => Ok(StructArg::Width(64)),
                _ => abort!(name, "unknown attribute"),
            }
        }
    }
}

#[derive(Debug)]
pub enum Accessor {
    ReadOnly,
    WriteOnly,
    ReadWrite,
}

// `0..=1` in #[bitfield(0..=1)]
#[derive(Debug)]
pub enum BitStructAttr {
    Range(RangeInclusive<usize>),
    Accessor(Accessor),
}

impl Parse for BitStructAttr {
    fn parse(input: ParseStream<'_>) -> syn::Result<BitStructAttr> {
        if input.peek(Ident) {
            let ident: Ident = input.parse()?;
            match ident.to_string().as_ref() {
                "readonly" => Ok(BitStructAttr::Accessor(Accessor::ReadOnly)),
                "writeonly" => Ok(BitStructAttr::Accessor(Accessor::WriteOnly)),
                "readwrite" => Ok(BitStructAttr::Accessor(Accessor::ReadWrite)),
                _ => Err(syn::Error::new(ident.span(), "unknown attribute")),
            }
        } else {
            let expr: ExprRange = input.parse()?;
            if !matches!(expr.limits, RangeLimits::Closed(_)) {
                return Err(syn::Error::new(
                expr.span(),
                "must be a closed range (..=) because of the author's strong opinions",
            ));
            }

            let start = parse_expr_lit(&expr.start)?;
            let end = parse_expr_lit(&expr.end)?;
            Ok(BitStructAttr::Range(start..=end))
        }
    }
}

pub struct StructParser {}

impl StructParser {
    pub fn new() -> Self {
        Self {}
    }

    pub fn parse(
        mut self,
        args_input: AttributeArgs<StructArg>,
        struct_input: &DataStruct,
        struct_span: Span,
    ) -> StructDef {
        let fields = match struct_input.fields {
            Fields::Named(ref fields) => fields,
            _ => abort!(
                struct_span,
                "#[bitields] can only be derived for structs with named fields"
            ),
        };

        let args = self.visit_args(&args_input);
        let struct_width = match args.struct_width {
            Some(8) => Ident::new("u8", Span::call_site()),
            Some(16) => Ident::new("u16", Span::call_site()),
            Some(32) => Ident::new("u32", Span::call_site()),
            Some(64) => Ident::new("u64", Span::call_site()),
            Some(w) => {
                abort!(args_input.span(), "a bitfield struct must have a width of 8, 16, 32, or 64 bits, not {}", w);
            }
            _ => {
                abort!(
                    args_input.span(),
                    "a bitfield struct requires a width attribute"
                );
            }
        };

        let fields = self.visit_fields(fields);

        StructDef {
            struct_width,
            fields: fields,
        }
    }

    fn visit_args(
        &mut self,
        args: &AttributeArgs<StructArg>,
    ) -> ArgsDefinition {
        let mut def = ArgsDefinition { struct_width: None };
        for arg in args.iter() {
            match arg {
                StructArg::Width(w) => {
                    def.struct_width = Some(*w);
                }
            }
        }
        def
    }

    fn visit_fields(&mut self, input: &FieldsNamed) -> Vec<Field> {
        let mut fields = Vec::with_capacity(input.named.len());
        let mut ranges = Vec::with_capacity(input.named.len());
        // Visit each field in the struct.
        for field in &input.named {
            let field_ident =
                field.ident.clone().expect("failed to get the field ident");

            // Look for #[bitfield] attribute.
            let mut attr_args = None;
            for attr in field.attrs.iter() {
                if attr.path().is_ident("bitfields") {
                    abort!(attr.span(), "the a field must be annotated with `bitfield` instead of `bitfields` (I mean, singular!)");
                }

                if attr.path().is_ident("bitfield") {
                    if attr_args.is_some() {
                        abort!(attr.span(), "a field can only be annotated with `bitfield` once");
                    }

                    // Parse #[bitfield(...)] attribute.
                    attr_args = Some(match attr.parse_args_with(
                        Punctuated::<BitStructAttr, Token![,]>::parse_terminated,
                    ) {
                        Ok(parsed) => parsed,
                        Err(err) => {
                            abort!(
                                err.span(),
                                format!(
                                    "failed to parse bitfield attribute: {}",
                                    err
                                ),
                            );
                        }
                    });
                }
            }

            // Visit each attribute argument.
            let mut accessor = None;
            if let Some(attr_args) = attr_args {
                for arg in attr_args {
                    match arg {
                        BitStructAttr::Accessor(acc) => {
                            accessor = Some(acc);
                        }
                        BitStructAttr::Range(range) => {
                            if range.start() > range.end() {
                                abort!(field_ident, "invalid bitfield range: end must be greater than or equal to start");
                            }

                            if ranges.iter().any(|r: &RangeInclusive<usize>| {
                                (range.start() >= r.start()
                                    && range.start() <= r.end())
                                    || (range.end() >= r.start()
                                        && range.end() <= r.end())
                            }) {
                                abort!(field_ident, "invalid bitfield range: overlaps with another field");
                            }

                            ranges.push(range);
                        }
                    }
                }
            }

            let field: Field = Field {
                ident: field_ident,
                ty: field.ty.clone(),
                accessor: accessor.unwrap_or(Accessor::ReadWrite),
            };

            fields.push(field);
        }

        fields
    }
}
