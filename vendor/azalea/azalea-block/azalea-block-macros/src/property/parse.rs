use proc_macro2::Ident;
use quote::quote;
use syn::{
    LitStr, Token, braced,
    ext::IdentExt,
    parenthesized,
    parse::{self, Parse, ParseStream},
    punctuated::Punctuated,
    token,
};

use crate::{PropertyKind, name_to_ident};

#[derive(Debug)]
pub struct PropertyWithNameAndDefault {
    pub name: String,
    pub name_ident: Ident,
    pub property_type: Ident,
    pub property_value_type: Ident,
    pub kind: PropertyKind,
    pub default: proc_macro2::TokenStream,
}
impl Parse for PropertyWithNameAndDefault {
    fn parse(input: ParseStream) -> parse::Result<Self> {
        let property_name = input.parse::<LitStr>()?.value();
        input.parse::<Token![:]>()?;

        let first_ident = input.call(Ident::parse_any)?;
        let mut property_default = quote! { #first_ident };

        let property_type: Ident;
        let property_value_type: Ident;
        let mut kind = PropertyKind::Bool;

        if input.parse::<Token![::]>().is_ok() {
            kind = PropertyKind::Enum;
            property_type = first_ident.clone();
            property_value_type = first_ident;
            let variant = input.parse::<Ident>()?;
            property_default = quote! { properties::#property_default::#variant };
        } else {
            let content;
            let _paren_token: token::Paren = parenthesized!(content in input);
            let unit_struct_inner = content.call(Ident::parse_any)?;
            let unit_struct_inner_string = unit_struct_inner.to_string();

            if matches!(unit_struct_inner_string.as_str(), "true" | "false") {
                property_value_type = Ident::new("bool", first_ident.span());
                property_type = first_ident;
                property_default = quote! { #unit_struct_inner };
            } else {
                return Err(input.error("Expected a boolean or an enum variant"));
            }
        };

        let property_name_ident = name_to_ident(&property_name);

        Ok(PropertyWithNameAndDefault {
            name: property_name,
            name_ident: property_name_ident,
            property_type,
            property_value_type,
            kind,
            default: property_default,
        })
    }
}

pub struct PropertyDefinition {
    pub name: LitStr,
    pub data: PropertyData,
}
impl Parse for PropertyDefinition {
    fn parse(input: ParseStream) -> parse::Result<Self> {

        let name = input.parse()?;
        input.parse::<Token![=>]>()?;
        let property_type = input.parse()?;

        input.parse::<Token![,]>()?;
        Ok(PropertyDefinition {
            name,
            data: property_type,
        })
    }
}

pub enum PropertyData {
    Enum {
        enum_name: Ident,
        variants: Vec<PropertyVariant>,
    },
    Bool { struct_name: Ident },
}
impl Parse for PropertyData {
    fn parse(input: ParseStream) -> parse::Result<Self> {
        let keyword = Ident::parse(input)?;

        fn parse_braced(
            input: ParseStream,
        ) -> parse::Result<Punctuated<PropertyVariant, Token![,]>> {
            let content;
            braced!(content in input);
            let variants = content.parse_terminated(parse_variant, Token![,])?;
            Ok(variants)
        }

        fn parse_variant(input: ParseStream) -> parse::Result<PropertyVariant> {
            let ident = Ident::parse(input)?;
            input.parse::<Token![=]>()?;
            let name = input.parse::<syn::LitStr>()?;
            Ok(PropertyVariant { ident, name })
        }

        fn parse_paren(input: ParseStream) -> parse::Result<Ident> {
            let content;
            parenthesized!(content in input);
            let inner = content.parse::<Ident>()?;
            Ok(inner)
        }

        if let Ok(variants) = parse_braced(input) {
            Ok(Self::Enum {
                enum_name: keyword,
                variants: variants.into_iter().collect(),
            })
        } else if let Ok(inner) = parse_paren(input) {
            assert_eq!(
                inner.to_string(),
                "bool",
                "Currently only bool unit structs are supported"
            );
            Ok(Self::Bool {
                struct_name: keyword,
            })
        } else {
            Err(input.error("Expected a unit struct or an enum"))
        }
    }
}

pub struct PropertyVariant {
    pub ident: Ident,
    pub name: LitStr,
}

pub fn parse_property_definitions(input: ParseStream) -> parse::Result<Vec<PropertyDefinition>> {
    let mut property_definitions = Vec::new();
    while !input.is_empty() {
        property_definitions.push(input.parse()?);
    }
    Ok(property_definitions)
}
