use std::ops::RangeInclusive;

use proc_macro2::Span;
use proc_macro_error::abort;
use syn::{
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
    spanned::Spanned,
    Data, DeriveInput, Expr, ExprRange, Fields, FieldsNamed, Ident, Lit,
    RangeLimits, Token, Type,
};

pub struct Field {
    pub ident: Ident,
    pub ty: Type,            // B1, ...
    pub container_ty: Ident, // u8, ...
    pub accessor: Accessor,
}

pub struct Definition {
    pub struct_name: Ident,
    pub struct_width: Ident,
    pub fields: Vec<Field>,
}

#[derive(Debug)]
struct ArgsDefinition {
    struct_width: Option<usize>,
}

// `width` in #[bitfields(width = 32)]
#[derive(Debug)]
pub enum BitStructArg {
    Width(usize),
}

impl Parse for BitStructArg {
    fn parse(input: ParseStream<'_>) -> syn::Result<BitStructArg> {
        let name: Ident = input.parse()?;
        let name_str = name.to_string();

        if input.peek(Token![=]) {
            abort!(name, "unexpected attribute");
        } else {
            // attributes without values.
            match name_str.as_ref() {
                "u8" => Ok(BitStructArg::Width(8)),
                "u16" => Ok(BitStructArg::Width(16)),
                "u32" => Ok(BitStructArg::Width(32)),
                "u64" => Ok(BitStructArg::Width(64)),
                _ => abort!(name, "unexpected attribute"),
            }
        }
    }
}

fn parse_expr_lit(expr: &Option<Box<syn::Expr>>) -> syn::Result<usize> {
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
                _ => Err(syn::Error::new(ident.span(), "unexpected attribute")),
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

/// Since syn v2.x, AttributeArgs got removed. Roll our own.
pub struct AttributeArgs {
    span: Span,
    args: Punctuated<BitStructArg, Token![,]>,
}

impl AttributeArgs {
    pub fn iter(&self) -> impl Iterator<Item = &BitStructArg> {
        self.args.iter()
    }

    pub fn span(&self) -> Span {
        self.span
    }
}

impl Parse for AttributeArgs {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        Ok(Self {
            span: input.span(),
            args: Punctuated::parse_terminated(input)?,
        })
    }
}

fn extract_type_name(ty: &Type) -> &Ident {
    match ty {
        Type::Path(ref type_path) => match type_path.path.segments.last() {
            Some(segment) => &segment.ident,
            None => {
                abort!(type_path.span(), "expected a type name");
            }
        },
        _ => panic!("expected a type"),
    }
}

pub struct Parser {}

impl Parser {
    pub fn new() -> Self {
        Self {}
    }

    pub fn parse(
        mut self,
        args_input: AttributeArgs,
        struct_input: DeriveInput,
    ) -> Definition {
        let data = match struct_input.data {
            Data::Struct(ref data) => data,
            _ => abort!(
                struct_input.span(),
                "BitStruct can only be derived for structs"
            ),
        };

        let fields =
            match data.fields {
                Fields::Named(ref fields) => fields,
                _ => abort!(struct_input.span(),
                "BitStruct can only be derived for structs with named fields"
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

        Definition {
            struct_name: struct_input.ident,
            struct_width,
            fields: fields,
        }
    }

    fn visit_args(&mut self, args: &AttributeArgs) -> ArgsDefinition {
        let mut def = ArgsDefinition { struct_width: None };
        for arg in args.iter() {
            match arg {
                BitStructArg::Width(w) => {
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
            let field_type = extract_type_name(&field.ty).clone();

            let cast_as = match field_type.to_string().as_str() {
                "B1" | "B2" | "B3" | "B4" | "B5" | "B6" | "B7" | "B8" => {
                    Ident::new("u8", Span::call_site())
                }
                "B9" | "B10" | "B11" | "B12" | "B13" | "B14" | "B15"
                | "B16" => Ident::new("u16", Span::call_site()),
                "B17" | "B18" | "B19" | "B20" | "B21" | "B22" | "B23"
                | "B24" | "B25" | "B26" | "B27" | "B28" | "B29" | "B30"
                | "B31" | "B32" => Ident::new("u32", Span::call_site()),
                "B33" | "B34" | "B35" | "B36" | "B37" | "B38" | "B39"
                | "B40" | "B41" | "B42" | "B43" | "B44" | "B45" | "B46"
                | "B47" | "B48" | "B49" | "B50" | "B51" | "B52" | "B53"
                | "B54" | "B55" | "B56" | "B57" | "B58" | "B59" | "B60"
                | "B61" | "B62" | "B63" | "B64" => {
                    Ident::new("u64", Span::call_site())
                }
                _ => {
                    abort!(
                        field_type.span(),
                        "a bitfield field must be of type u8, u16, u32, or u64"
                    );
                }
            };

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
                container_ty: cast_as,
                accessor: accessor.unwrap_or(Accessor::ReadWrite),
            };

            fields.push(field);
        }

        fields
    }
}
