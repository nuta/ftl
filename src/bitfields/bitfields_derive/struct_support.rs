use proc_macro::TokenStream;
use proc_macro2::Span;
use proc_macro_error::abort;
use quote::quote;
use syn::{
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
    spanned::Spanned,
    DataStruct, Fields, FieldsNamed, Ident, Token, Type,
};

use crate::helpers::AttributeArgs;

pub struct Field {
    pub ident: Ident,
    pub ty: Type,
    pub accessor: Accessor,
    pub hidden: bool,
}

pub struct StructDef {
    pub struct_width: Ident,
    pub fields: Vec<Field>,
}

#[derive(Debug)]
struct ArgsDefinition {
    struct_width: Option<Ident>,
}

// `u32` in #[bitfields(u32)]
#[derive(Debug)]
pub enum StructAttr {
    Width(Ident),
}

impl Parse for StructAttr {
    fn parse(input: ParseStream<'_>) -> syn::Result<StructAttr> {
        let name: Ident = input.parse()?;
        let name_str = name.to_string();

        if input.peek(Token![=]) {
            abort!(name, "unknown attribute");
        } else {
            // attributes without values.
            match name_str.as_str() {
                "u8" | "u16" | "u32" | "u64" => Ok(StructAttr::Width(name)),
                _ => abort!(name, "unknown attribute"),
            }
        }
    }
}

pub enum Accessor {
    ReadOnly,
    WriteOnly,
    ReadWrite,
}

// `readonly` in #[bitfield(readonly)]
pub enum FieldAttr {
    Accessor(Accessor),
    Hidden,
}

impl Parse for FieldAttr {
    fn parse(input: ParseStream<'_>) -> syn::Result<FieldAttr> {
        let ident: Ident = input.parse()?;

        // if input.peek(Token![=]) {
        //     // attributes with values.
        //     let _ = input.parse::<Token![=]>()?;

        //     match ident.to_string().as_ref() {
        //         _ => Err(syn::Error::new(ident.span(), "unknown attribute")),
        //     }
        // } else {
        match ident.to_string().as_ref() {
            "readonly" => Ok(FieldAttr::Accessor(Accessor::ReadOnly)),
            "writeonly" => Ok(FieldAttr::Accessor(Accessor::WriteOnly)),
            "hidden" => Ok(FieldAttr::Hidden),
            _ => Err(syn::Error::new(ident.span(), "unknown attribute")),
        }
        // }
    }
}

fn parse(
    args_input: AttributeArgs<StructAttr>,
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

    let args = visit_args(&args_input);
    let struct_width = match args.struct_width {
        Some(ref w) => w.clone(),
        None => {
            abort!(
                args_input.span(),
                "a bitfield struct requires a width attribute like #[bitfield(u32)]"
            );
        }
    };

    let fields = visit_fields(fields);

    StructDef {
        struct_width,
        fields: fields,
    }
}

fn visit_args(args: &AttributeArgs<StructAttr>) -> ArgsDefinition {
    let mut def = ArgsDefinition { struct_width: None };
    for arg in args.iter() {
        match arg {
            StructAttr::Width(w) => {
                def.struct_width = Some(w.clone());
            }
        }
    }
    def
}

fn visit_fields(input: &FieldsNamed) -> Vec<Field> {
    let mut fields = Vec::with_capacity(input.named.len());

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
                    abort!(
                        attr.span(),
                        "a field can only be annotated with `bitfield` once"
                    );
                }

                // Parse #[bitfield(...)] attribute.
                attr_args = Some(
                    match attr.parse_args_with(
                        Punctuated::<FieldAttr, Token![,]>::parse_terminated,
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
                    },
                );
            }
        }

        // Visit each attribute argument.
        let mut accessor = None;
        let mut hidden = false;
        if let Some(attr_args) = attr_args {
            for arg in attr_args {
                match arg {
                    FieldAttr::Accessor(acc) => {
                        accessor = Some(acc);
                    }
                    FieldAttr::Hidden => {
                        hidden = true;
                    }
                }
            }
        }

        let field: Field = Field {
            ident: field_ident,
            ty: field.ty.clone(),
            accessor: accessor.unwrap_or(Accessor::ReadWrite),
            hidden,
        };

        fields.push(field);
    }

    fields
}

pub fn bitfields_struct(
    struct_name: &Ident,
    args_input: AttributeArgs<StructAttr>,
    struct_input: &DataStruct,
    struct_span: Span,
) -> TokenStream {
    let StructDef {
        struct_width,
        fields,
    } = parse(args_input, struct_input, struct_span);

    let mut methods = Vec::with_capacity(fields.len());
    let mut prev_fields = Vec::with_capacity(fields.len());
    let mut prev_types = Vec::with_capacity(fields.len());
    let mut debug_fields = Vec::with_capacity(fields.len());
    for Field {
        ident,
        ty,
        accessor,
        hidden,
    } in &fields
    {
        let (readable, writable) = match accessor {
            Accessor::ReadOnly => (true, false),
            Accessor::WriteOnly => (false, true),
            Accessor::ReadWrite => (true, true),
        };

        let vis = if *hidden {
            quote! {}
        } else {
            quote! { pub }
        };

        // Offset: foo_offset()
        let offset =
            Ident::new(&format!("{}_offset", ident), Span::call_site());
        if prev_fields.is_empty() {
            methods.push(quote! {
                #[inline]
                #vis const fn #offset() -> usize {
                    0
                }
            });
        } else {
            methods.push(quote! {
                #[inline]
                #vis const fn #offset() -> usize {
                    #(<#prev_types as ::bitfields::BitField>::BITS)+*
                }
            });
        }

        // Width: foo_width()
        let width = Ident::new(&format!("{}_width", ident), Span::call_site());
        methods.push(quote! {
            #[inline]
            #vis const fn #width() -> usize {
                use ::bitfields::BitField;
                #ty::BITS
            }
        });

        if !hidden {
            // Bit range: foo_range()
            let range =
                Ident::new(&format!("{}_range", ident), Span::call_site());
            methods.push(quote! {
                #[inline]
                pub const fn #range() -> ::core::ops::RangeInclusive<usize> {
                    Self::#offset()..=Self::#offset() + Self::#width() - 1
                }
            });

            // Getter: foo()
            if readable {
                let getter = ident.clone();
                methods.push(quote! {
                #[inline(always)]
                pub fn #getter(&self) -> <#ty as ::bitfields::BitField>::AccessorValueType {
                    let mask = ((1 << Self::#width()) - 1) << Self::#offset();
                    let value = (self.raw & mask) >> Self::#offset();
                    <#ty as ::bitfields::BitField>::from_u64(value as u64)
                }
            });
            }

            // Setter: set_foo()
            if writable {
                let setter =
                    Ident::new(&format!("set_{}", ident), Span::call_site());

                methods.push(quote! {
                #[inline(always)]
                pub fn #setter(&mut self, value: <#ty as ::bitfields::BitField>::AccessorValueType) {
                    let value = <#ty as ::bitfields::BitField>::into_u64(value) as #struct_width;
                    debug_assert!(value < (1 << Self::#width()), concat!("value is too large for the field"));
                    self.raw |= value << Self::#offset();
                }
                });
            }

            debug_fields.push(quote! {
                .field(stringify!(#ident), &self.#ident())
            });
        }

        prev_fields.push(ident);
        prev_types.push(ty);
    }

    // from_raw, into_raw
    methods.push(quote! {
        pub const fn from_raw(value: #struct_width) -> Self {
            Self { raw: value }
        }

        pub const fn into_raw(self) -> #struct_width {
            self.raw
        }
    });

    let mut static_asserts = Vec::new();
    if let Some(Field { ident, .. }) = fields.last() {
        let offset =
            Ident::new(&format!("{}_offset", ident), Span::call_site());
        let width = Ident::new(&format!("{}_width", ident), Span::call_site());
        static_asserts.push(quote! {
            const _: () = assert!(
                #struct_name::#offset() + #struct_name::#width() == 8*::core::mem::size_of::<#struct_width>(),
                concat!(
                    stringify!(#struct_name), " is not ", stringify!(#struct_width), "-sized struct (hint: perhaps you forgot to add padding fields?)"
                )
            );
        });
    }

    quote! {
        struct #struct_name {
            raw: #struct_width,
        }

        #(#static_asserts)*

        impl #struct_name {
            pub fn zeroed() -> Self {
                Self { raw: 0 }
            }
        }

        impl ::core::fmt::Debug for #struct_name {
            fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
                f.debug_struct(stringify!(#struct_name))
                    .field("__raw", &self.raw)
                    #(#debug_fields)*
                    .finish()
            }
        }

        impl #struct_name {
            #(#methods)*
        }
    }
    .into()
}
