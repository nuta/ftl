//! The attribute parser. Partially based on structopt's parser:
//! https://github.com/TeXitoi/structopt/blob/c0933257c769e0a74445b82b083b14070c81ce9e/structopt-derive/src/parse.rs#L12
//! (Apache-2.0 OR MIT).
use proc_macro::TokenStream;
use proc_macro2::Span;
use proc_macro_error::abort;
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input,
    punctuated::Punctuated,
    Data, DeriveInput, Fields, FieldsNamed, Ident, Lit, LitInt, Meta, Token,
};

pub struct Field {
    pub ident: Ident,
    pub offset: usize,
    pub width: usize,
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

fn consume_litint_as_usize(input: &mut ParseStream<'_>) -> syn::Result<usize> {
    Ok(input.parse::<LitInt>()?.base10_parse()?)
}

#[derive(Debug)]
pub enum BitStructArg {
    Width(usize),
}

impl Parse for BitStructArg {
    fn parse(mut input: ParseStream<'_>) -> syn::Result<BitStructArg> {
        let name: Ident = input.parse()?;
        let name_str = name.to_string();

        if input.peek(Token![=]) {
            // `name = value` attributes.
            let assign_token = input.parse::<Token![=]>()?; // skip '='

            match &*name_str {
                "width" => Ok(BitStructArg::Width(consume_litint_as_usize(
                    &mut input,
                )?)),
                _ => abort!(assign_token, "unexpected toke",),
            }
        } else {
            // attributes without values.
            match name_str.as_ref() {
                "foo" => {
                    panic!("foo")
                }
                _ => abort!(name, "unexpected attribute"),
            }
        }
    }
}

#[derive(Debug)]
pub enum BitStructAttr {
    Offset(usize),
    Width(usize),
}

impl Parse for BitStructAttr {
    fn parse(mut input: ParseStream<'_>) -> syn::Result<BitStructAttr> {
        let name: Ident = input.parse()?;
        let name_str = name.to_string();

        if input.peek(Token![=]) {
            // `name = value` attributes.
            let assign_token = input.parse::<Token![=]>()?; // skip '='

            match &*name_str {
                "offset" => Ok(BitStructAttr::Offset(consume_litint_as_usize(
                    &mut input,
                )?)),
                "width" => Ok(BitStructAttr::Width(consume_litint_as_usize(
                    &mut input,
                )?)),
                _ => abort!(assign_token, "unexpected toke",),
            }
        } else {
            // attributes without values.
            match name_str.as_ref() {
                "foo" => {
                    panic!("foo")
                }
                _ => abort!(name, "unexpected attribute"),
            }
        }
    }
}

/// Since syn v2.x, AttributeArgs got removed. Roll our own.
pub struct AttributeArgs(Punctuated<BitStructArg, Token![,]>);

impl AttributeArgs {
    pub fn iter(&self) -> impl Iterator<Item = &BitStructArg> {
        self.0.iter()
    }
}

impl Parse for AttributeArgs {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        Ok(Self(Punctuated::parse_terminated(input)?))
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
            Data::Struct(data) => data,
            _ => panic!("BitStruct can only be derived for structs"),
        };

        let fields = match data.fields {
            Fields::Named(fields) => fields,
            _ => panic!(
                "BitStruct can only be derived for structs with named fields"
            ),
        };

        let args = self.visit_args(args_input);
        let struct_width = match args.struct_width {
            Some(8) => Ident::new("u8", Span::call_site()),
            Some(16) => Ident::new("u16", Span::call_site()),
            Some(32) => Ident::new("u32", Span::call_site()),
            Some(64) => Ident::new("u64", Span::call_site()),
            Some(w) => {
                panic!("a bitfield struct must have a width of 8, 16, 32, or 64 bits, not {}", w);
            }
            _ => {
                panic!("a bitfield struct requires a width attribute");
            }
        };

        let fields = self.visit_fields(fields);
        Definition {
            struct_name: struct_input.ident,
            struct_width,
            fields: fields,
        }
    }

    fn visit_args(&mut self, args: AttributeArgs) -> ArgsDefinition {
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

    fn visit_fields(&mut self, input: FieldsNamed) -> Vec<Field> {
        let mut next_offset = 0;
        let mut fields = Vec::with_capacity(input.named.len());
        for field in input.named {
            let field_ident = field.ident.unwrap();

            let mut offset = None;
            let mut width = None;
            for attr in field
                .attrs
                .iter()
                .filter(|attr| attr.path().is_ident("bit_types"))
                .flat_map(|attr| {
                    attr.parse_args_with(
                        Punctuated::<BitStructAttr, Token![,]>::parse_terminated,
                    )
                    .expect("failed to parse bit_types attribute")
                })
            {
                match attr {
                    BitStructAttr::Offset(o) => {
                        offset = Some(o);
                    }
                    BitStructAttr::Width(w) => {
                        width = Some(w);
                    }
                }
            }

            let field: Field = Field {
                ident: field_ident,
                offset: offset.unwrap_or(next_offset),
                width: width.unwrap_or(1),
            };
            next_offset = field.offset + field.width;

            fields.push(field);
        }

        fields
    }
}
