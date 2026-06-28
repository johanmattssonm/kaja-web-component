use proc_macro2::Span;
use proc_macro2::TokenStream;
use quote::quote;
use syn::parse2;
use syn::{Ident, ItemStruct, LitStr};

#[proc_macro_attribute]
pub fn component(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let html_tag_name = proc_macro2::TokenStream::from(attr.clone());
    let tag_value = parse2::<LitStr>(html_tag_name.clone());

    if tag_value.is_empty() {
        return syn::Error::new_spanned(
            tag_value,
            "expected a tag name or component: #[component(example-component-tag)]",
        )
        .to_compile_error()
        .into();
    }

    let component_struct_name = proc_macro2::TokenStream::from(item.clone());
    let component_struct_item = match syn::parse2::<ItemStruct>(component_struct_name.clone()) {
        Ok(s) => s,
        Err(_) => {
            return syn::Error::new_spanned(
                component_struct_name,
                "the #[component] attribute can only be applied to structs",
            )
            .to_compile_error()
            .into();
        }
    };

    let struct_name_ident = &component_struct_item.ident;
    let create_html_component_fn_name = format!("createComponent{}", struct_name_ident);
    let fn_create_ident = Ident::new(&create_html_component_fn_name, Span::call_site());
    let html_tag = LitStr::new(&tag_value, Span::call_site());

    let create_fn = generate_create_component_fn(fn_create_ident);

    let expanded = quote! {
        #component_struct_item
        #create_fn
    };

    TokenStream::from(expanded)
}

fn generate_create_component_fn(name: Ident) -> TokenStream {
    let expanded = quote! {
        #[allow(non_snake_case)]
        pub fn #name() { // createComponentTheStructName

            let wasm_id = 0;

            let component = Function::new_no_args(
                r#"
                let componentId = -1;

                return class extends HTMLElement {
                    connectedCallback() {
                        if (componentId == -1) {
                            componentId = counterInit();
                        }

                        counterComponentConnect(this, componentId);
                    }

                    disconnectedCallback() {
                        counterComponentDisconnect(this, componentId);
                    }

                    attributeChangedCallback(name, oldValue, newValue) {
                        //callbacks.attributeChanged(this, name, oldValue, newValue);
                    }

                    static get observedAttributes() {
                        return [];
                    }
                }
            "#,
            );

            let class = component.call0(&JsValue::NULL);

            if class.is_err() {
                error!("Failed to register class for", tag_name);
                return;
            }

            let class_fn = class.unwrap();
            let registry = web_sys::window().unwrap().custom_elements();

            let result = registry.define(tag_name, &class_fn.unchecked_into());

            if result.is_err() {
                error!("Failed to register component", tag_name);
            }
        }
    };

    expanded
}
