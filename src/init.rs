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

    let create_class_js = format!("createComponentClass{}", struct_name_ident);
    let init_self_js = format!("initSelf{}", struct_name_ident);
    let attach_js = format!("attachComponent{}", struct_name_ident);

    let create_class_js_indent = Ident::new(&create_class_js, Span::call_site());
    let init_self_js_ident = Ident::new(&init_self_js, Span::call_site());

    let create_js_class_fn = create_class(
        create_class_js_indent,
        tag_lit.clone(),
        init_self_js.as_str(),
        attach_js.as_str(),
    );

    let rs_self = gen_rust_self(component_struct_item.clone(), init_self_js_ident);
    let attach_fn = attach_quote(&attach_js, struct_name_ident);

    let expanded = quote! {
        #global_store
        #rs_self
        #component_struct_item
        #create_js_class_fn
        #attach_fn
    };

    expanded.into()
}

fn attach_quote(atach_state_js_fn: &str, struct_name: &Ident) -> proc_macro2::TokenStream {
    let js_callback_ident = Ident::new(atach_state_js_fn, Span::call_site());
    let rs_name_str = from_pascal_to_snake(atach_state_js_fn);
    let rs_name_ident = Ident::new(&rs_name_str, Span::call_site());

    let store_name_str = store_name(struct_name);
    let store_ident = Ident::new(&store_name_str, Span::call_site());

    let expanded = quote! {
        #[callback(#js_callback_ident)]
        fn #rs_name_ident(element: HtmlElement, wasm_id: u32) {
            gloo::console::log!("componentConnect generated wasm_id: {}", wasm_id);

            let mut store = #store_ident.write().unwrap();

            if store.is_none() {
                error!("Component not found in connect.");
                return;
            }

            let store_map = store.as_mut().unwrap();

            if let Some(component) = store_map.get_mut(&wasm_id) {
                component.connected(element);
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

fn generate_component_store(struct_name: Ident) -> proc_macro2::TokenStream {
    // Generate only the per-component store (HashMap) here.
    let store_name = store_name(&struct_name);
    let store_ident = Ident::new(&store_name, Span::call_site());

    let expanded = quote! {
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

fn gen_rust_self(struct_item: ItemStruct, js_name: Ident) -> proc_macro2::TokenStream {
    let struct_ident = &struct_item.ident;
    let store_name_str = store_name(struct_ident);
    let store_ident = Ident::new(&store_name_str, Span::call_site());

    // Check whether the struct has a named field called `value` so we can initialize it.
    let mut has_value_field = false;
    if let syn::Fields::Named(fields_named) = &struct_item.fields {
        for field in fields_named.named.iter() {
            if let Some(ident) = &field.ident {
                if ident == "value" {
                    has_value_field = true;
                    break;
                }
            }
        }
    }

    let set_value_ts = if has_value_field {
        quote! {
            counter.value = wasm_id as usize;
        }
    } else {
        quote! {}
    };

    let struct_name_str = struct_ident.to_string();
    let rust_name = format!(
        "init_component_self_{}",
        from_pascal_to_snake(struct_name_str.as_str())
    );
    let rust_ident = Ident::new(&rust_name, Span::call_site());

    let expanded = quote! {
        #[callback(#js_name)]
        #[allow(non_snake_case)]
        pub fn #rust_ident() -> u32 {  // init_component_state_struct_name, returns id for the instance of the element
            log!("Run counter init rs state.");
            let mut counter = #struct_ident::default();
            let wasm_id = ::kaja_web::NEXT_COMPONENT_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

            #set_value_ts

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

fn create_class(
    class_creator: Ident,
    tag: LitStr,
    init_state_fn: &str,
    atach_state_fn: &str,
) -> proc_macro2::TokenStream {
    let name_str_lit = LitStr::new(&class_creator.to_string(), Span::call_site());

    let expanded = quote! {
        ::inventory::submit! {
            InitComponentFn(#class_creator)
        }

        #[allow(non_snake_case)]
        pub fn #class_creator() { // createComponentTheStructName
            let wasm_id = 0;

            let component_name = #name_str_lit;
            let tag_name = #tag;
            log!("init component", component_name, tag_name);

            let js_init = String::from(
                            r#"
                                return class extends HTMLElement {
                                    constructor() {
                                        super();
                                        this.__componentId = -1;
                                    }

                                    connectedCallback() {
                                        if (this.__componentId === -1) {
                                            this.__componentId = __INIT_CALLBACK();
                                        }

                                        __ATTACH_CALLBACK(this, this.__componentId);
                                    }

                                    disconnectedCallback() {
                                        //counterComponentDisconnect(this, this.__componentId);
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
