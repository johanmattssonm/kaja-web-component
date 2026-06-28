mod init;

use proc_macro as pm;

#[proc_macro_attribute]
pub fn component(attr: pm::TokenStream, item: pm::TokenStream) -> pm::TokenStream {
    init::component_impl(attr, item)
}
