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
    let detach_js = format!("detachComponent{}", struct_name_ident);
    let observed_js = format!("observedComponentAttribute{}", struct_name_ident);
    let changed_js = format!("changedComponentAttribute{}", struct_name_ident);

    let create_class_js_indent = Ident::new(&create_class_js, Span::call_site());
    let init_self_js_ident = Ident::new(&init_self_js, Span::call_site());

    let create_js_class_fn = create_class(
        create_class_js_indent,
        tag_lit.clone(),
        init_self_js.as_str(),
        attach_js.as_str(),
        detach_js.as_str(),
        observed_js.as_str(),
        changed_js.as_str(),
        struct_name_ident,
    );

    let rs_self = gen_rust_self(component_struct_item.clone(), init_self_js_ident);
    let attach_fn = attach_quote(&attach_js, struct_name_ident);
    let detach_fn = detach_quote(&detach_js, struct_name_ident);
    let observed_fn = observed_quote(&observed_js, struct_name_ident);
    let changed_fn = changed_quote(&changed_js, struct_name_ident);
    let self_from_id = component_from_id(component_struct_item.clone());

    let expanded = quote! {
        #global_store
        #rs_self
        #component_struct_item
        #create_js_class_fn
        #attach_fn
        #detach_fn
        #observed_fn
        #self_from_id
        #changed_fn
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
        fn #rs_name_ident(element: HtmlElement, wasm_id: String) {
            gloo::console::log!("componentConnect generated wasm_id: {}", wasm_id.clone());

            let mut store = #store_ident.write().unwrap();

            if store.is_none() {
                error!("Store not created before component not found in connect.");
                return;
            }

            let store_map = store.as_mut().unwrap();

            if let Some(component) = store_map.get_mut(&wasm_id) {
                component.connected(element);
            } else {
                error!("Component id not found in connect.", wasm_id.clone());
            }
        }
    };

    expanded
}

fn detach_quote(detach_state_js_fn: &str, struct_name: &Ident) -> proc_macro2::TokenStream {
    let js_callback_ident = Ident::new(detach_state_js_fn, Span::call_site());
    let rs_name_str = from_pascal_to_snake(detach_state_js_fn);
    let rs_name_ident = Ident::new(&rs_name_str, Span::call_site());

    let store_name_str = store_name(struct_name);
    let store_ident = Ident::new(&store_name_str, Span::call_site());

    let expanded = quote! {
        #[callback(#js_callback_ident)]
        fn #rs_name_ident(element: HtmlElement, wasm_id: String) {
            let mut store = #store_ident.write().unwrap();

            if store.is_none() {
                error!("Store not found in disconnected.");
                return;
            }

            let store_map = store.as_mut().unwrap();

            if let Some(component) = store_map.get_mut(&wasm_id) {
                component.disconnected(element);
            } else {
                error!("Component id not found in disconnected.", wasm_id.clone());
            }

            store_map.remove(&wasm_id);
        }
    };

    expanded
}

fn changed_quote(changed_state_js_fn: &str, struct_name: &Ident) -> proc_macro2::TokenStream {
    let js_callback_ident = Ident::new(changed_state_js_fn, Span::call_site());
    let rs_name_str = from_pascal_to_snake(changed_state_js_fn);
    let rs_name_ident = Ident::new(&rs_name_str, Span::call_site());

    let store_name_str = store_name(struct_name);
    let store_ident = Ident::new(&store_name_str, Span::call_site());

    let expanded = quote! {
        #[callback(#js_callback_ident)]
        fn #rs_name_ident(element: HtmlElement, wasm_id: String, key: String, from: String, to: String) {
            let mut store = #store_ident.write().unwrap();

            if store.is_none() {
                error!("Store not found in attribute change callback.");
                return;
            }

            let store_map = store.as_mut().unwrap();

            if let Some(component) = store_map.get_mut(&wasm_id) {
                let name = key.as_str();
                let old_value = ::wasm_bindgen::JsValue::from_str(&from);
                let new_value = ::wasm_bindgen::JsValue::from_str(&to);
                component.attribute_changed(element, name, &old_value, &new_value);
            } else {
                error!("Component id not found in changed attribute.", wasm_id.clone());
            }
        }
    };

    expanded
}

fn observed_quote(observed_js_fn: &str, struct_name: &Ident) -> proc_macro2::TokenStream {
    let js_callback_ident = Ident::new(observed_js_fn, Span::call_site());
    let rs_name_str = from_pascal_to_snake(observed_js_fn);
    let rs_name_ident = Ident::new(&rs_name_str, Span::call_site());

    let struct_ident = struct_name;

    let expanded = quote! {
        #[callback(#js_callback_ident)]
        fn #rs_name_ident() -> &'static [&'static str] {
            #struct_ident::observed_attributes()
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
        pub static #store_ident: std::sync::RwLock<Option<std::collections::HashMap<String, #struct_name>>> = std::sync::RwLock::new(None);
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

fn component_from_id(struct_item: ItemStruct) -> proc_macro2::TokenStream {
    let struct_ident = &struct_item.ident;
    let expanded = quote! {
        pub fn get_counter() -> #struct_ident {  // returns an instance of the component
            log!("Get counter");
            #struct_ident::default()
        }
    };

    expanded
}

fn gen_rust_self(struct_item: ItemStruct, js_name: Ident) -> proc_macro2::TokenStream {
    let struct_ident = &struct_item.ident;
    let store_name_str = store_name(struct_ident);
    let store_ident = Ident::new(&store_name_str, Span::call_site());

    let struct_name_str = struct_ident.to_string();
    let var_name_str = from_pascal_to_snake(struct_name_str.as_str());
    let var_ident = Ident::new(&var_name_str, Span::call_site());

    let rust_name = format!(
        "init_component_self_{}",
        from_pascal_to_snake(struct_name_str.as_str())
    );
    let rust_ident = Ident::new(&rust_name, Span::call_site());

    let log_msg = LitStr::new(
        &format!(
            "Run {} init rs state. Init self for struct.",
            struct_name_str
        ),
        Span::call_site(),
    );

    let expanded = quote! {
        #[callback(#js_name)]
        #[allow(non_snake_case)]
        pub fn #rust_ident() -> String {  // init_component_state_struct_name, returns id for the instance of the element
            gloo::console::log!(#log_msg);

            let mut #var_ident = #struct_ident::default();
            let wasm_id = ::kaja_web::NEXT_COMPONENT_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

            let mut store = #store_ident
                .write()
                .expect("failed to acquire write lock");

            if store.is_none() {
                error!("No store in self init.");
                return "".to_string();
            }

            let key = wasm_id.to_string();
            let store_map = store.as_mut().unwrap();
            store_map.insert(key.clone(), #var_ident);
            key
        }
    };

    expanded
}

fn create_class(
    class_creator: Ident,
    tag: LitStr,
    init_self_fn: &str,
    attach_state_fn: &str,
    detach_state_fn: &str,
    observed_fn: &str,
    change_fn: &str,
    struct_name: &Ident,
) -> proc_macro2::TokenStream {
    let name_str_lit = LitStr::new(&class_creator.to_string(), Span::call_site());
    let store_name_str = store_name(&struct_name);
    let store_ident = Ident::new(&store_name_str, Span::call_site());

    let expanded = quote! {
        ::inventory::submit! {
            InitComponentFn(#class_creator)
        }

        fn init_store() {
            let mut store = #store_ident
                .write()
                .expect("failed to acquire write lock");

            log!("Check if store is initialized.");

            if store.is_none() {
                log!("Init store");
                *store = Some(std::collections::HashMap::new());
            }
        }

        #[allow(non_snake_case)]
        pub fn #class_creator() { // createComponentTheStructName
            init_store();

            let component_name = #name_str_lit;
            let tag_name = #tag;

            log!("init component", component_name, tag_name);

            let js_init = String::from(
                r#"
                    return class extends HTMLElement {
                        constructor() {
                            super();
                            let componentId = __INIT_SELF_CALLBACK();
                            this.setAttribute("componentid", componentId);
                            console.log("created class " + componentId);
                        }

                        connectedCallback() {
                            let componentid = this.getAttribute("componentid");
                            __ATTACH_CALLBACK(this, componentid);
                        }

                        disconnectedCallback() {
                            let componentid = this.getAttribute("componentid");
                            __DETACH_CALLBACK(this, componentid);
                        }

                        attributeChangedCallback(name, oldValue, newValue) {
                            let componentid = this.getAttribute("componentid");
                            console.log(componentid, name, oldValue, newValue);
                            __ATTRIBUTE_CHANGED_CALLBACK(this, componentid, name, oldValue === null ? "" : String(oldValue), newValue === null ? "" : String(newValue));
                        }

                        static get observedAttributes() {
                            return __OBSERVED_CALLBACK();
                        }
                    }
            "#,
            );

            let js_init_with_state = js_init.replace("__INIT_SELF_CALLBACK", #init_self_fn);
            let js_init_with_state = js_init_with_state.replace("__ATTACH_CALLBACK", #attach_state_fn);
            let js_init_with_state = js_init_with_state.replace("__DETACH_CALLBACK", #detach_state_fn);
            let js_init_with_state = js_init_with_state.replace("__ATTRIBUTE_CHANGED_CALLBACK", #change_fn);
            let js_init_with_state = js_init_with_state.replace("__OBSERVED_CALLBACK", #observed_fn);

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
