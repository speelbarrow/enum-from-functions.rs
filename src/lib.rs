/*!
This crate contains a procedural macro attribute that can be placed on an `impl` block. It will generate an `enum`
based on the functions defined in the `impl` block. The generated `enum` will have a variant for each function, and a
new function `map` will be added to the `impl` block that will call the appropriate function based on the variant.

An example:
```
# use enum_from_functions::enum_from_functions;
#[enum_from_functions]
impl Enum {
    async fn foo() -> &'static str {
        "Foo"
    }
    unsafe fn bar(baz: i32) -> &'static str {
        "Bar"
    }
}
# fn main() {
#     futures::executor::block_on(
#         async {
#             unsafe {
#                 assert_eq!(Enum::map(Enum::Foo).await, "Foo");
#                 assert_eq!(Enum::map(Enum::Bar { baz: 1337 }).await, "Bar");
#             }
#         }
#     )
# }
```
expands to:
```ignore
enum Enum {
    Foo,
    Bar {
        baz: i32
    },
}

impl Enum {
    async fn foo() -> &'static str {
        "Foo"
    }
    unsafe fn bar(baz: i32) -> &'static str {
        "Bar"
    }

    async unsafe fn map(&self) -> &'static str {
        match self {
            Enum::Foo => Enum::foo().await,
            Enum::Bar(baz) => Enum::bar(baz),
        }
    }
}
```
The signatures of functions in the `impl` block may be different, so long as they all have the same return type.

Note that `fn f() -> T` and `async fn f() -> T` are considered to return the same type, even though the latter
technically returns a `impl Future<Output = T>`. See
[the `async` keyword documentation](https://doc.rust-lang.org/std/keyword.async.html) for more information.
```
# use enum_from_functions::enum_from_functions;
#[enum_from_functions]
impl Enum {
    fn foo(baz: i32) -> &'static str {
        "Foo"
    }
    async fn bar(&self, baz: bool) -> &'static str {
        "Bar"
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
`async`, `const` and `unsafe` functions are supported. The presence of any of these keywords will result in the
generated `map` function having the same keyword. For this reason, `async` and `const` functions cannot be present in
the same `impl` block (though `unsafe` functions can be present with either of the other two).
```compile_fail
# use enum_from_functions::enum_from_functions;
#[enum_from_functions]
impl Enum {
    async fn foo() -> &'static str {
        "Foo"
    }
    const fn bar() -> &'static str {
        "Bar"
    }

    // This would result in `async const map(...` which is not supported in Rust.
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

mod extract;
mod generate;

use generate::WithoutTypes;
use proc_macro::TokenStream;
use proc_macro_error::{abort, emit_error, proc_macro_error};
use quote::quote;
use syn::{parse_macro_input, parse_quote, ExprBlock, Field, Fields, ItemImpl};

/**
A procedural macro attribute that generates an `enum` based on the functions defined in the `impl` block it annotates.
See the crate documentation for more information.
*/
#[proc_macro_error]
#[proc_macro_attribute]
pub fn enum_from_functions(args: TokenStream, input: TokenStream) -> TokenStream {
    let pub_token = match extract::pub_token(args) {
        Ok(pub_token) => pub_token,
        Err(err) => {
            emit_error!(err.span(), err);
            None
        }
    };

    let (parsed_input, attributes) = {
        let mut parsed_input = parse_macro_input!(input as ItemImpl);
        let attributes = parsed_input.attrs.clone();
        parsed_input.attrs.clear();
        (parsed_input, attributes)
    };

    let enum_name = &*parsed_input.self_ty;
    let functions = match extract::Functions::try_from(&parsed_input) {
        Ok(functions) => functions,
        Err(err) => abort!(err.span(), err),
    };

    // Unpack the struct here because we can't in the `quote` block.
    let (return_type, asyncness, constness, unsafety, calls, variants) = {
        (
            &functions.return_type,
            functions.asyncness,
            functions.constness,
            functions.unsafety,
            &functions.calls,
            generate::Variants::from(&functions),
        )
    };

    let variants_iter = variants.0.iter();
    let variant_names = variants.0.iter().map(|variant| &variant.ident);
    let variant_fields = variants.0.iter().map(|variant| -> Option<ExprBlock> {
        if let Fields::Named(fields) = &variant.fields {
            let no_types = Field::without_types(&fields.named);
            Some(parse_quote! { { #no_types } })
        } else {
            None
        }
    });

    quote! {
        #(#attributes)*
        #pub_token enum #enum_name {
            #(#variants_iter,)*
        }

        #parsed_input

        impl #enum_name {
            #pub_token #asyncness #constness #unsafety fn map(self) #return_type {
                match self {
                    #(Self::#variant_names #variant_fields => #calls,)*
                }
            }
        }
    }
    .into()
}
