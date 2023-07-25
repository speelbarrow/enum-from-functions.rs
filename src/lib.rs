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
#     assert_eq!(Enum::map(&Enum::Foo), "Foo");
#     assert_eq!(Enum::map(&Enum::Bar), "Bar");
#     assert_eq!(Enum::map(&Enum::Baz), "Baz");
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

fn main() {
# assert!((|| { return
    Visible::map(&Visible::Example);
# })());
}
```
```compile_fail
#   mod internal {
#       #[enum_from_functions::enum_from_functions]
#       impl NotVisible {
#           fn example() -> bool {
#               false
#           }
#       }
#   }
#
#   fn main() {
#       assert!(!NotVisible::map(&NotVisible::Example));
#   }
```
Items in the `impl` block that are not functions will be ignored and passed through to the output unchanged.
```
# use enum_from_functions::enum_from_functions;
#[enum_from_functions]
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
#     assert_eq!(Enum::map(&Enum::Foo), "Foo");
#     assert_eq!(Enum::map(&Enum::Bar), "Bar");
#     assert_eq!(Enum::map(&Enum::Baz), "Baz");
# }
```
*/

use proc_macro::TokenStream;

/**
A procedural macro attribute that generates an `enum` based on the functions defined in the `impl` block it annotates.
See the crate documentation for more information.
*/
#[proc_macro_attribute]
pub fn enum_from_functions(_args: TokenStream, input: TokenStream) -> TokenStream {
    input
}
