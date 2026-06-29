mod init;

use proc_macro as pm;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::ItemStruct;

#[proc_macro_attribute]
pub fn component(attr: pm::TokenStream, item: pm::TokenStream) -> pm::TokenStream {
    // Clone the attribute token stream so we can pass ownership into the helper
    // without risking accidental moves causing "use of moved value" errors when
    // the macro is expanded multiple times.
    let attr_clone = attr.clone();

    // Add `#[derive(Default)]` to the struct.
    let item_ts2 = TokenStream2::from(item.clone());
    let mut input = match syn::parse2::<ItemStruct>(item_ts2.clone()) {
        Ok(s) => s,
        Err(_) => return init::component_impl(attr_clone.clone(), item),
    };

    input.attrs.push(syn::parse_quote!(#[derive(Default)]));

    // Add the functions needed for a complete web component using kaja-web
    let new_item_ts2 = quote!(#input);
    init::component_impl(attr_clone, pm::TokenStream::from(new_item_ts2))
}
