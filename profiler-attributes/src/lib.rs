use proc_macro::TokenStream;
use quote::quote;

#[proc_macro_attribute]
pub fn profile(_args: TokenStream, input: TokenStream) -> TokenStream {
	let mut item: syn::Item = syn::parse(input).unwrap();
    let fn_item = match &mut item {
        syn::Item::Fn(fn_item) => fn_item,
        _ => panic!("expected fn")
    };
    fn_item.block.stmts.insert(0,syn::parse(quote!(let ___scope = profiler::Scope::new(profiler::function_name!().to_string());).into()).unwrap());

    use quote::ToTokens;
    item.into_token_stream().into()
}