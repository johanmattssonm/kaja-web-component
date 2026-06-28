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

    let global_store = generate_component_store(struct_name_ident.clone());

    let create_html_component_fn_name = format!("createComponent{}", struct_name_ident);
    let component_rs_state_js_fn = format!("initComponentState{}", struct_name_ident);
    let attachjs_fn_name = format!("attachComponent{}", struct_name_ident);

    let fn_create_ident = Ident::new(&create_html_component_fn_name, Span::call_site());
    let component_rs_state_js_fn_ident = Ident::new(&component_rs_state_js_fn, Span::call_site());

    let component_rs_state_rs_fn = from_pascal_to_snake(component_rs_state_js_fn.as_str());
    let component_rs_state_rs_js_ident = Ident::new(&component_rs_state_rs_fn, Span::call_site());

    let create_js_fn = generate_register_component_fn(
        fn_create_ident,
        tag_lit.clone(),
        component_rs_state_js_fn.as_str(),
        attachjs_fn_name.as_str(),
    );

    let rs_state_init_fn = gen_rust_self(
        component_rs_state_rs_js_ident,
        component_rs_state_js_fn_ident,
        struct_name_ident.clone(),
    );

    let attach_fn = attach_quote(&attachjs_fn_name);

    let expanded = quote! {
        #global_store
        #rs_state_init_fn
        #component_struct_item
        #create_js_fn
        #attach_fn
    };

    expanded.into()
}

fn attach_quote(atach_state_js_fn: &str) -> proc_macro2::TokenStream {
    let js_callback_ident = Ident::new(atach_state_js_fn, Span::call_site());
    let rs_name_str = from_pascal_to_snake(atach_state_js_fn);
    let rs_name_ident = Ident::new(&rs_name_str, Span::call_site());

    let expanded = quote! {
        #[callback(#js_callback_ident)]
        fn #rs_name_ident(element: HtmlElement, wasm_id: u32) {
            gloo::console::log!("counterComponentConnect generated but hard coded wasm_id: {}", wasm_id);

            let mut store = COUNTER_COMPONENTS.write().unwrap();

            if store.is_none() {
                error!("Component not found in connect.");
                return;
            }

            let store_map = store.as_mut().unwrap();

            if let Some(counter) = store_map.get_mut(&wasm_id) {
                counter.connected(element);
            } else {
                error!("Component id {} not found in connect.", wasm_id);
            }
        }
    };

    expanded
}

fn store_name(struct_name: &Ident) -> String {
    let store_name = from_pascal_to_snake(&struct_name.to_string()).to_uppercase() + "_COMPONENTS";
    store_name
}

fn counter_name(struct_name: &Ident) -> String {
    let store = store_name(struct_name);
    let id_counter_name = format!("{}{}", store, "_NEXT_ID");
    id_counter_name
}

fn generate_component_store(struct_name: Ident) -> proc_macro2::TokenStream {
    // Generate:
    // pub static THETHING_COMPONENTS_COUNTER (AtomicU32)
    // pub static THETHING_COMPONENTS: RwLock<Option<HashMap<u32, Counter>>> = RwLock::new(None);
    let store_name = store_name(&struct_name);
    let id_counter_name = Ident::new(&counter_name(&struct_name), Span::call_site());
    let store_ident = Ident::new(&store_name, Span::call_site());

    let expanded = quote! {
        pub static #id_counter_name: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(1);
        pub static #store_ident: std::sync::RwLock<Option<std::collections::HashMap<u32, #struct_name>>> = std::sync::RwLock::new(None);
    };

    expanded
}

fn from_pascal_to_snake(fn_name: &str) -> String {
    let mut result = String::new();

    for (i, c) in fn_name.chars().enumerate() {
        if c.is_uppercase() {
            if i != 0 {
                result.push('_');
            }

            result.extend(c.to_lowercase());
        } else {
            result.push(c);
        }
    }

    result
}

fn gen_rust_self(rust_name: Ident, js_name: Ident, struct_name: Ident) -> proc_macro2::TokenStream {
    let store_name = store_name(&struct_name);
    let counter_name = counter_name(&struct_name);

    let counter_ident = Ident::new(&counter_name, Span::call_site());
    let store_ident = Ident::new(&store_name, Span::call_site());

    let expanded = quote! {
        #[callback(#js_name)]
        #[allow(non_snake_case)]
        pub fn #rust_name() -> u32 {  // init_component_state_struct_name, returns id for the instance of the element
            log!("Run counter init rs state.");
            let counter = #struct_name::default();
            let wasm_id = #counter_ident.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

            let mut store = #store_ident.write().expect("failed to acquire write lock");

            if store.is_none() {
                *store = Some(std::collections::HashMap::new());
            }

            let store_map = store.as_mut().unwrap();
            store_map.insert(wasm_id, counter);
            wasm_id
        }
    };

    expanded
}

fn generate_register_component_fn(
    name: Ident,
    tag: LitStr,
    init_state_fn: &str,
    atach_state_fn: &str,
) -> proc_macro2::TokenStream {
    let name_str_lit = LitStr::new(&name.to_string(), Span::call_site());

    let expanded = quote! {
        ::inventory::submit! {
            InitComponentFn(#name)
        }

        #[allow(non_snake_case)]
        pub fn #name() { // createComponentTheStructName
            let wasm_id = 0;

            let component_name = #name_str_lit;
            let tag_name = #tag;
            log!("init component", component_name, tag_name);

            let js_init = String::from(
                r#"
                    let componentId = -1;

                    return class extends HTMLElement {
                        connectedCallback() {
                            if (componentId == -1) {
                                componentId = __INIT_CALLBACK();
                            }

                            __ATTACH_CALLBACK(this, componentId);
                        }

                        disconnectedCallback() {
                            //counterComponentDisconnect(this, componentId);
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
            let js_init_with_state = js_init_with_state.replace("__ATTACH_CALLBACK", #atach_state_fn);

            let component = js_sys::Function::new_no_args(js_init_with_state.as_str());

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
