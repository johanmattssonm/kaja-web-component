use proc_macro as pm;
use proc_macro2::Span;
use quote::quote;
use syn::parse2;
use syn::{Ident, ItemStruct, LitStr};

pub fn component_impl(attr: pm::TokenStream, item: pm::TokenStream) -> pm::TokenStream {
    // Parse this #[component("my-tag")]) for a struct
    let html_tag_ts = proc_macro2::TokenStream::from(attr.clone());
    let tag_lit = match parse2::<LitStr>(html_tag_ts) {
        Ok(lit) => lit,
        Err(err) => return err.to_compile_error().into(),
    };

    if tag_lit.value().is_empty() {
        return syn::Error::new_spanned(
            &tag_lit,
            "expected a tag name or component: #[component(\"example-component-tag\")]",
        )
        .to_compile_error()
        .into();
    }

    // Parse the annotated item as a struct
    let component_struct_ts = proc_macro2::TokenStream::from(item.clone());
    let component_struct_item = match syn::parse2::<ItemStruct>(component_struct_ts.clone()) {
        Ok(s) => s,
        Err(_) => {
            return syn::Error::new_spanned(
                component_struct_ts,
                "the #[component] attribute can only be applied to structs",
            )
            .to_compile_error()
            .into();
        }
    };

    let struct_name_ident = &component_struct_item.ident;
    let create_html_component_fn_name = format!("createComponent{}", struct_name_ident);
    let fn_create_ident = Ident::new(&create_html_component_fn_name, Span::call_site());

    let component_rs_state_fn = format!("initComponentState{}", struct_name_ident);
    let component_rs_state_fn_ident = Ident::new(&component_rs_state_fn, Span::call_site());

    let create_fn = generate_create_component_fn(
        fn_create_ident,
        tag_lit.clone(),
        component_rs_state_fn.as_str(),
    );

    let rs_state_fn = generate_default_component_rs_fn(component_rs_state_fn_ident);

    let expanded = quote! {
        #component_struct_item
        #rs_state_fn
        #create_fn
    };

    expanded.into()
}

fn generate_default_component_rs_fn(name: Ident) -> proc_macro2::TokenStream {
    let expanded = quote! {
        #[allow(non_snake_case)]
        pub fn #name() { // initComponentStateStructName
        }
    };

    expanded
}

fn generate_create_component_fn(
    name: Ident,
    tag: LitStr,
    init_state_fn: &str,
) -> proc_macro2::TokenStream {
    let js_init = String::from(
        r#"
            let componentId = -1;

            return class extends HTMLElement {
                connectedCallback() {
                    if (componentId == -1) {
                        componentId = __INIT_CALLBACK;
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

    let js_init_with_state = js_init.replace("__INIT_CALLBACK", #init_state_fn);

    let expanded = quote! {
        #[allow(non_snake_case)]
        pub fn #name() { // createComponentTheStructName
            let wasm_id = 0;
            let class_init_fn = format!()
            let component = Function::new_no_args(r#"js_init_with_state"#);

            let class = component.call0(&JsValue::NULL);

            if class.is_err() {
                error!("Failed to register class for {}", #tag);
                return;
            }

            let class_fn = class.unwrap();
            let registry = web_sys::window().unwrap().custom_elements();

            let result = registry.define(#tag, &class_fn.unchecked_into());

            if result.is_err() {
                error!("Failed to register component {}", #tag);
            }
        }
    };

    expanded
}
