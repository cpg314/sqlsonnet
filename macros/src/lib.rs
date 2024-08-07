extern crate proc_macro;
use proc_macro::TokenStream;
use quote::quote;

/// Macro for inline sqlsonnet (actually simply jsonnet), reporting syntax errors as build errors.
/// ```
/// sqlsonnet!( { select: { fields: [1+1] } } );
/// ```
#[proc_macro]
pub fn sqlsonnet(input: TokenStream) -> TokenStream {
    let input = input.to_string();
    let source = jrsonnet_parser::Source::new_virtual("source".into(), input.as_str().into());
    if let Some(err) =
        jrsonnet_parser::parse(&input, &jrsonnet_parser::ParserSettings { source }).err()
    {
        let err = err.to_string();
        quote! { compile_error!(#err); }.into()
    } else {
        quote! { #input }.into()
    }
}
