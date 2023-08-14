use proc_macro::TokenStream;
use proc_macro_error::{abort, emit_error};
use syn::{
    parse_quote, punctuated::Pair, spanned::Spanned, Expr, FnArg, ImplItem, ItemImpl, ReturnType,
    Signature, Token,
};

use crate::generate::WithoutTypes;

pub fn pub_token(args: TokenStream) -> Result<Option<Token![pub]>, syn::Error> {
    if args.is_empty() {
        Ok(None)
    } else {
        syn::parse::<Token![pub]>(args).map(Some)
    }
}

pub struct Functions<'a> {
    pub signatures: Vec<&'a Signature>,
    pub return_type: ReturnType,
    pub calls: Vec<Expr>,
    pub asyncness: Option<Token![async]>,
    pub constness: Option<Token![const]>,
    pub unsafety: Option<Token![unsafe]>,
}
impl Functions<'_> {
    fn new() -> Self {
        Functions {
            signatures: Vec::new(),
            return_type: ReturnType::Default,
            calls: Vec::new(),
            asyncness: None,
            constness: None,
            unsafety: None,
        }
    }
}
impl<'a> TryFrom<&'a ItemImpl> for Functions<'a> {
    type Error = syn::Error;

    fn try_from(input: &'a ItemImpl) -> Result<Self, Self::Error> {
        let mut r = Functions::new();

        // This will be set once the first function is found, and then used to ensure that all other functions have the
        // same return type.
        let mut return_type: Option<&ReturnType> = None;

        // Iterate over all items in the `input` block.
        for item in &input.items {
            // Only process the item if it is a function.
            if let ImplItem::Fn(function) = item {
                // If the return type has been set, check that it matches.
                if let Some(return_type) = return_type {
                    if return_type != &function.sig.output {
                        emit_error!(
                            return_type.span(),
                            "return type does not match `{:?}`",
                            function.sig.output
                        );
                        emit_error!(
                            function.sig.output,
                            "return type does not match `{:?}`",
                            return_type
                        );
                    }

                // Otherwise, assign `return_type`.
                } else {
                    return_type = Some(&function.sig.output);
                }

                // Check that we aren't mixing `async` and `const` functions (otherwise [`map`] would need to be `async
                // const`, which is not possible).
                let async_const = match (
                    &function.sig.asyncness,
                    &function.sig.constness,
                    &r.asyncness,
                    &r.constness,
                ) {
                    (Some(asyncness), None, None, Some(constness)) => Some((asyncness, constness)),
                    (None, Some(constness), Some(asyncness), None) => Some((asyncness, constness)),
                    _ => None,
                };

                if let Some((asyncness, constness)) = async_const {
                    emit_error!(
                        asyncness,
                        "cannot mix `async` and `const` functions, as this would require `map` to be `async const`"
                    );
                    abort!(
                        constness,
                        "cannot mix `async` and `const` functions, as this would require `map` to be `async const`"
                    );
                }

                // Once all checks have passed, add the function signature to the list and set the modifier flags on
                // the return `struct` (if necessary).
                r.signatures.push(&function.sig);
                r.calls.push({
                    let name = &function.sig.ident;
                    let recv = if let Some(FnArg::Receiver(r)) = &function.sig.inputs.first() {
                        Some(Pair::new(r, Some(<Token![,]>::default())))
                    } else {
                        None
                    };
                    let args = FnArg::without_types(&function.sig.inputs);

                    let mut call = Expr::Call(parse_quote!(Self::#name(#recv #args)));
                    if function.sig.asyncness.is_some() {
                        call = Expr::Await(parse_quote!(#call .await));
                    }

                    call
                });
                macro_rules! set_flag {
                    ( $( $flag:ident ),* ) => {
                        $(
                            if let Some($flag) = function.sig.$flag {
                                r.$flag = Some($flag.clone());
                            }
                        )*
                    };
                }
                set_flag!(asyncness, constness, unsafety);
            }
        }

        if let Some(return_type) = return_type {
            r.return_type = return_type.clone();
        }

        Ok(r)
    }
}
