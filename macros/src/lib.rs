extern crate proc_macro;
use proc_macro::TokenStream;
use quote::quote;

fn crate_() -> proc_macro2::TokenStream {
    if std::env::var("CARGO_CRATE_NAME").unwrap() == "sqlsonnet"
        // Avoid catching the doctest
        && std::env::var("UNSTABLE_RUSTDOC_TEST_PATH").is_err()
    {
        quote! {crate}
    } else {
        quote! {sqlsonnet}
    }
}

#[derive(Debug)]
struct Input {
    sqlsonnet: String,
    variables: Vec<Variable>,
    resolver: syn::Expr,
}

#[derive(Debug)]
struct Variable {
    name: syn::Ident,
    value: syn::Expr,
}
impl syn::parse::Parse for Variable {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let name = input.parse()?;
        input.parse::<syn::token::Eq>()?;
        let value = input.parse()?;
        Ok(Self { name, value })
    }
}
impl syn::parse::Parse for Input {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let resolver = if input.peek(syn::token::Brace) {
            let crate_ = crate_();
            syn::parse_quote! { #crate_::jsonnet::FsResolver::default() }
        } else {
            let resolver: syn::Expr = input.parse()?;
            input.parse::<syn::token::Comma>()?;
            resolver
        };
        let content;
        syn::braced!(content in input);
        let sqlsonnet: proc_macro2::TokenStream = content.parse()?;
        let variables = if input.parse::<syn::token::Comma>().is_ok() {
            syn::punctuated::Punctuated::<Variable, syn::token::Comma>::parse_terminated(input)?
                .into_iter()
                .collect()
        } else {
            vec![]
        };
        Ok(Self {
            sqlsonnet: format!("{{{}}}", sqlsonnet),
            variables,
            resolver,
        })
    }
}

/// Similar to [`sqlsonnet_lit!`], but constructs a `Query`.
/// If the resolver (first argument) is not passed, the default `FsResolver` is used.
/// Variables can be passed as `extVar`.
/// ```ignore
/// sqlsonnet_query!( { select: { fields: [std.extVar("test")] } });
/// sqlsonnet_query!( { select: { fields: [std.extVar("test")] } }, test=10);
/// sqlsonnet_query!( resolver, { select: { fields: [std.extVar("test")] } }, test=10);
/// ```
#[proc_macro]
pub fn sqlsonnet_query(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as Input);
    let sqlsonnet = input.sqlsonnet;
    let vars = input.variables.into_iter().map(|v| {
        let name = v.name.to_string();
        let value = v.value;
        quote! { options.add_var(#name, #value)?; }
    });
    let resolver = input.resolver;
    let crate_ = crate_();
    let syntax = check_syntax(&sqlsonnet);
    let q = quote! {
        {
            #syntax
            let mut options = #crate_::jsonnet::Options::new(#resolver, "");
            #(#vars)*
            #crate_::Query::from_jsonnet(&#sqlsonnet, options)
        }
    };
    q.into()
}

/// Macro for inline sqlsonnet (actually simply jsonnet), reporting syntax errors as build errors.
/// ```ignore
/// sqlsonnet_lit!( { select: { fields: [1+1] } } );
/// ```
#[proc_macro]
pub fn sqlsonnet_lit(input: TokenStream) -> TokenStream {
    let input = input.to_string();
    let syntax = check_syntax(&input);
    quote! {
        #syntax
        #input
    }
    .into()
}

fn check_syntax(input: &str) -> proc_macro2::TokenStream {
    let source = jrsonnet_parser::Source::new_virtual("source".into(), input.into());
    if let Some(err) =
        jrsonnet_parser::parse(input, &jrsonnet_parser::ParserSettings { source }).err()
    {
        let err = err.to_string();
        quote! { compile_error!(#err); }
    } else {
        quote! {}
    }
}
