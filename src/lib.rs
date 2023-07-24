use proc_macro::TokenStream;

#[proc_macro_attribute]
pub fn enum_from_functions(_args: TokenStream, input: TokenStream) -> TokenStream {
    input
}
