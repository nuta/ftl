//! The attribute parser. Partially based on structopt's parser:
//! https://github.com/TeXitoi/structopt/blob/c0933257c769e0a74445b82b083b14070c81ce9e/structopt-derive/src/parse.rs#L12
//! (Apache-2.0 OR MIT).
use proc_macro_error::{abort, ResultExt};
use syn::{
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
    Data, DeriveInput, Fields, FieldsNamed, Ident, LitInt, LitStr, Meta, Token,
    Type,
};

pub struct Field {
    pub ident: Ident,
    pub offset: usize,
    pub width: usize,
}

pub struct Definition {
    fields: Vec<Field>,
}

#[derive(Debug)]
pub enum BitStructAttr {
    Offset(usize),
    Width(usize),
}

fn consume_litint_as_usize(input: &mut ParseStream<'_>) -> syn::Result<usize> {
    Ok(input.parse::<LitInt>()?.base10_parse()?)
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

pub struct Parser {
    def: Definition,
}

impl Parser {
    pub fn new() -> Self {
        Self {
            def: Definition { fields: Vec::new() },
        }
    }

    pub fn parse(mut self, input: DeriveInput) -> Definition {
        let struct_ident = input.ident;

        let data = match input.data {
            Data::Struct(data) => data,
            _ => panic!("BitStruct can only be derived for structs"),
        };

        let fields = match data.fields {
            Fields::Named(fields) => fields,
            _ => panic!(
                "BitStruct can only be derived for structs with named fields"
            ),
        };

        self.visit_fields(fields);
        self.def
    }

    fn visit_fields(&mut self, fields: FieldsNamed) {
        let mut next_offset = 0;
        for field in fields.named {
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

            self.def.fields.push(field);
        }
    }
}
