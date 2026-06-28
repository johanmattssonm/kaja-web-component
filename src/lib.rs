mod init;

use proc_macro as pm;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::ItemStruct;

#[proc_macro_attribute]
pub fn component(attr: pm::TokenStream, item: pm::TokenStream) -> pm::TokenStream {
    // Add `#[derive(Default)]` to the struct.
    let item_ts2 = TokenStream2::from(item.clone());
    let mut input = match syn::parse2::<ItemStruct>(item_ts2.clone()) {
        Ok(s) => s,
        Err(_) => return init::component_impl(attr, item),
    };

    input.attrs.push(syn::parse_quote!(#[derive(Default)]));

    // Add the functions needed for a complete web component using kaja-web
    let new_item_ts2 = quote!(#input);
    init::component_impl(attr, pm::TokenStream::from(new_item_ts2))
}
