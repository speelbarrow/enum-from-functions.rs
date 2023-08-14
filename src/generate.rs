use convert_case::{Case, Casing};
use proc_macro::Span;
use syn::{
    parse_quote,
    punctuated::{Pair, Punctuated},
    Field, FieldsNamed, FnArg, Ident, Pat, Signature, Token, Variant,
};

use crate::extract::Functions;

pub struct Variants(pub Vec<Variant>);
impl Variants {
    fn convert_single(signature: &Signature) -> Variant {
        let variant_name = Ident::new(
            &signature.ident.to_string().to_case(Case::Pascal),
            Span::call_site().into(),
        );
        let fields: Option<FieldsNamed> = {
            if !signature.inputs.is_empty() {
                let mut inputs = signature.inputs.iter().peekable();
                if let Some(FnArg::Receiver(_)) = inputs.peek() {
                    inputs.next();
                }
                Some(parse_quote!({ #(#inputs),* }))
            } else {
                None
            }
        };

        parse_quote!(#variant_name #fields)
    }
}
impl From<&Functions<'_>> for Variants {
    fn from(input: &Functions<'_>) -> Self {
        let mut r = Vec::new();
        for signature in &input.signatures {
            r.push(Variants::convert_single(signature));
        }

        Self(r)
    }
}

pub trait WithoutTypes: Sized {
    fn without_types(from: &Punctuated<Self, Token![,]>) -> Punctuated<Ident, Token![,]>;
}

impl WithoutTypes for FnArg {
    fn without_types(from: &Punctuated<Self, Token![,]>) -> Punctuated<Ident, Token![,]> {
        Punctuated::from_iter(from.pairs().filter_map(|pair| {
            if let FnArg::Typed(pat_type) = pair.value() {
                match pat_type.pat.as_ref() {
                    Pat::Ident(pat_ident) => Some(pat_ident.ident.clone()),
                    Pat::Wild(_) => None,
                    _ => unreachable!(),
                }
            } else {
                None
            }
        }))
    }
}

impl WithoutTypes for Field {
    fn without_types(from: &Punctuated<Self, Token![,]>) -> Punctuated<Ident, Token![,]> {
        Punctuated::from_iter(from.pairs().map(|pair| {
            Pair::new(
                pair.value().ident.as_ref().unwrap().clone(),
                pair.punct().map(|p| **p),
            )
        }))
    }
}
