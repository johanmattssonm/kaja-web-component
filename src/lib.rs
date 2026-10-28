mod init;

use proc_macro as pm;

/// ```rust,ignore
/// use kaja_web::prelude::*;
///
/// #[component("main-component")]
/// struct Main {
///     #[attribute]
///     test: bool,
/// }
///
/// impl Component for MainComponent {
///     fn connected(&self) {
///         log!("Connected:", self.get_id());
///     }
///
///     fn disconnected(&self) {
///         log!("Disconnected:", self.get_id());
///     }
///
///     fn attribute_changed(&self, name: &str, _old: &str, _new: &str) {
///         if name == "test" {
///             let value: bool = self.get_attribute("test").unwrap_or_default();
///             log!("Test:", value);
///         }
///     }
/// }
/// ```
///
/// ```html
/// <main-component test="true"></main-component>
/// ```
#[proc_macro_attribute]
pub fn component(attr: pm::TokenStream, item: pm::TokenStream) -> pm::TokenStream {
    init::component_impl(attr, item)
}
