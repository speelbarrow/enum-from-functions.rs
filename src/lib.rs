/*!
This crate contains a procedural macro attribute that can be placed on an `impl` block. It will generate an `enum`
based on the functions defined in the `impl` block. The generated `enum` will have a variant for each function, and a
new function `map` will be added to the `impl` block that will call the appropriate function based on the variant.

An example:
```
# use enum_from_functions::enum_from_functions;
#[enum_from_functions]
impl Enum {
    fn foo() -> &'static str {
        "Foo"
    }
    fn bar() -> &'static str {
        "Bar"
    }

    fn baz() -> &'static str {
        "Baz"
    }
}
# fn main() {
#     assert_eq!(Enum::map(Enum::Foo), "Foo");
#     assert_eq!(Enum::map(Enum::Bar), "Bar");
#     assert_eq!(Enum::map(Enum::Baz), "Baz");
# }
```
expands to:
```ignore
enum Enum {
    Foo,
    Bar,
    Baz,
}

impl Enum {
    fn foo() -> &'static str {
        "Foo"
    }
    fn bar() -> &'static str {
        "Bar"
    }
    fn baz() -> &'static str {
        "Baz"
    }

    fn map(&self) -> &'static str {
        match self {
            Enum::Foo => Enum::foo(),
            Enum::Bar => Enum::bar(),
            Enum::Baz => Enum::baz(),
        }
    }
}
```
The signatures of all the functions in the `impl` block must be the same and must not use the `self` keyword. Aside
from that, any function signature will work with this macro.
```compile_fail
# use enum_from_functions::enum_from_functions;
#[enum_from_functions]
impl Enum {
    // Causes a compile error because the `self` argument isn't allowed.
    fn foo(self) -> &'static str {
        "Foo"
    }
}
```
```compile_fail
# use enum_from_functions::enum_from_functions;
// Causes a compile error because the return types don't match.
#[enum_from_functions]
impl Enum {
    fn foo() -> &'static str {
        "Foo"
    }
    fn bar() -> String {
        "Bar".to_owned()
    }
}
```
```compile_fail
# use enum_from_functions::enum_from_functions;
// Causes a compile error because the argument types don't match.
#[enum_from_functions]
impl Enum {
    fn foo(_: i32) -> &'static str {
        "Foo"
    }
    fn bar(_: bool) -> &'static str {
        "Bar"
    }
}
```
You can also create an empty `enum` by not providing any functions in the `impl` block (though I'm not sure why you
would want to do this).
```
# use enum_from_functions::enum_from_functions;
#[enum_from_functions]
impl EmptyEnum {}
```
If you need to export the generated `enum` type out of its parent module, provide the `pub` argument to the macro
attribute.
```
mod internal {
#   use enum_from_functions::enum_from_functions;
    #[enum_from_functions(pub)]
    impl Visible {
        fn example() -> bool {
            true
        }
    }
}

// Will compile because the generated `enum` is visible outside of the `internal` module.
use internal::Visible;
```
```compile_fail
mod internal {
#   use enum_from_functions::enum_from_functions;
    #[enum_from_functions]
    impl NotVisible {
        fn example() -> bool {
            false
        }
    }
}

// Causes a compile error because the generated `enum` is not visible outside of the `internal` module.
use internal::NotVisible;
```
Items in the `impl` block that are not functions will be ignored and passed through to the output unchanged.
Similarly, any attributes applied before *or* after the macro attribute will be applied to the generated `enum`
declaration.
```
# use enum_from_functions::enum_from_functions;
#[enum_from_functions]
##[derive(Debug)]
impl Enum {
    const FOO: &'static str = "Foo";
    fn foo() -> &'static str {
        Self::FOO
    }

    const BAR: &'static str = "Bar";
    fn bar() -> &'static str {
        Self::BAR
    }

    const BAZ: &'static str = "Baz";
    fn baz() -> &'static str {
        Self::BAZ
    }
}
# fn main() {
#     assert_eq!(Enum::map(Enum::Foo), "Foo");
#     assert_eq!(Enum::map(Enum::Bar), "Bar");
#     assert_eq!(Enum::map(Enum::Baz), "Baz");
#     let _ = format!("{:?}", Enum::Foo);
# }
```
*/

use convert_case::{Case, Casing};
use proc_macro::{Span, TokenStream};
use syn::{
    parse_macro_input, parse_quote,
    punctuated::{Pair, Punctuated},
    token::Comma,
    FnArg, ImplItem, Pat,
};

/**
A procedural macro attribute that generates an `enum` based on the functions defined in the `impl` block it annotates.
See the crate documentation for more information.
*/
#[proc_macro_attribute]
pub fn enum_from_functions(args: TokenStream, input: TokenStream) -> TokenStream {
    // Parse the arguments either as empty or as a `pub` token. Any other arguments cause an error.
    let parsed_pub = if !args.is_empty() {
        Some(parse_macro_input!(args as syn::Token![pub]))
    } else {
        None
    };

    // Parse the input as an `impl` block (any other input will cause an error here).
    let mut parsed_impl = parse_macro_input!(input as syn::ItemImpl);

    // Set aside the attributes (if any) on the `impl` block for later, moving them out of the `impl` block.
    let attrs = parsed_impl.attrs.drain(..).collect::<Vec<_>>();

    // Iterate through the items in the `impl` block, looking for functions.
    // Each function has its signature verified against the first found function. Then the name is converted to
    // PascalCase and added to the list of variant identifiers.

    let mut variants = Vec::<syn::Ident>::new();
    let mut function_names = Vec::<syn::Ident>::new();
    let mut first_sig: Option<&syn::Signature> = None;

    for item in parsed_impl.items.iter() {
        // Only proceed if the item is a function.
        if let ImplItem::Fn(function) = item {
            // If `first_sig` has already been set, verify this function's signature against it. Otherwise, assign it.
            if let Some(first_sig) = first_sig {
                macro_rules! anonimize {
                    ($sig:expr) => {{
                        let mut to_anon = $sig.clone();
                        to_anon.ident =
                            syn::Ident::new("anon", proc_macro::Span::call_site().into());
                        to_anon
                    }};
                }

                let (anon_first_sig, anon_func_sig) =
                    (anonimize!(first_sig), anonimize!(&function.sig));
                if anon_first_sig != anon_func_sig {
                    syn::Error::new(
                        Span::call_site().into(),
                        format!(
                            "mismatched signatures:\n\t`{:?}`\nand\n\t`{:?}`",
                            anon_first_sig, anon_func_sig
                        ),
                    )
                    .into_compile_error();
                }
            } else {
                // If the first function has a `self` argument, error out.
                if let Some(syn::FnArg::Receiver(_)) = function.sig.inputs.first() {
                    syn::Error::new(
                        Span::call_site().into(),
                        "the `self` argument is not allowed in functions used by `enum_from_functions`",
                    )
                    .into_compile_error();
                }

                first_sig = Some(&function.sig);
            }

            // Convert the function's name to PascalCase and add it to the list of variant identifiers.
            variants.push(syn::Ident::new(
                &function.sig.ident.to_string().to_case(Case::Pascal),
                Span::call_site().into(),
            ));
            function_names.push(function.sig.ident.clone());
        }
    }

    let enum_name = &parsed_impl.self_ty;
    if let Some(item) = first_sig.map(|some| {
        let mut sig = some.clone();
        sig.ident = syn::Ident::new("map", Span::call_site().into());
        sig.inputs.insert(0, syn::parse_quote!(self));

        let args = Punctuated::<&Box<Pat>, Comma>::from_iter(some.inputs.pairs().map(|pair| {
            match pair.value() {
                FnArg::Typed(arg) => Pair::new(&arg.pat, pair.punct().map(|_| Comma::default())),
                FnArg::Receiver(_) => unreachable!(),
            }
        }));

        parse_quote! {
            pub #sig {
                match self {
                    #(Self::#variants => Self::#function_names(#args)),*
                }
            }
        }
    }) {
        parsed_impl.items.push(item)
    }
    let out = quote::quote! {
        #(#attrs)*
        #parsed_pub enum #enum_name {
            #(#variants),*
        }

        #parsed_impl
    };

    out.into()
}
