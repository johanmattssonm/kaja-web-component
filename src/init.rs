use proc_macro as pm;
use proc_macro2::Span;
use quote::quote;
use syn::parse2;
use syn::{Fields, Ident, ItemStruct, LitStr};

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
    let component_impl = component_impl_quote(struct_name_ident);
    let component_full_impl = create_component(&component_struct_item);

    let expanded = quote! {
        #component_struct_item
        #component_impl
        #component_full_impl

        #create_js_class_fn

        #rs_self

        #attach_fn
        #detach_fn
        #observed_fn
        #changed_fn
    };

    expanded.into()
}

fn attach_quote(atach_state_js_fn: &str, struct_name: &Ident) -> proc_macro2::TokenStream {
    let js_callback_ident = Ident::new(atach_state_js_fn, Span::call_site());
    let rs_name_str = from_pascal_to_snake(atach_state_js_fn);
    let rs_name_ident = Ident::new(&rs_name_str, Span::call_site());
    let component_name = Ident::new(&format!("{}Component", struct_name), Span::call_site());

    let expanded = quote! {
        #[callback(#js_callback_ident)]
        fn #rs_name_ident(id: String) {
            if let Some(component) = #component_name::get_component(&id) {
                component.connected();
            } else {
                error!("Component id not found in connect.", id.clone());
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
    let component_name = Ident::new(&format!("{}Component", struct_name), Span::call_site());

    let expanded = quote! {
        #[callback(#js_callback_ident)]
        fn #rs_name_ident(wasm_id: String) {
            if let Some(component) = #component_name::get_component(&wasm_id) {
                component.disconnected();
                #store_ident.with(|store| {
                    store.borrow_mut().remove(&wasm_id);
                });
            } else {
                error!("Component id not found in disconnected.", wasm_id.clone());
            }
        }
    };

    expanded
}

fn component_impl_quote(struct_name: &Ident) -> proc_macro2::TokenStream {
    let snake_name = from_pascal_to_snake(&struct_name.to_string());
    let init_fn = format!("{}_init_component", snake_name);
    let init_component_fn = Ident::new(&init_fn, struct_name.span());

    let expanded = quote! {
        ::inventory::submit! {
            InitComponentFn(#init_component_fn)
        }

        pub fn #init_component_fn() {
            log!("Init component", stringify!(#struct_name));
        }
    };

    expanded
}

fn changed_quote(changed_state_js_fn: &str, struct_name: &Ident) -> proc_macro2::TokenStream {
    let js_callback_ident = Ident::new(changed_state_js_fn, Span::call_site());
    let rs_name_str = from_pascal_to_snake(changed_state_js_fn);
    let rs_name_ident = Ident::new(&rs_name_str, Span::call_site());
    let component_name = Ident::new(&format!("{}Component", struct_name), Span::call_site());

    let expanded = quote! {
        #[callback(#js_callback_ident)]
        fn #rs_name_ident(element_id: String, key: String, from: String, to: String) {
            if let Some(component) = #component_name::get_component(&element_id) {
                let name = key.as_str();
                component.internal_attributes_update(name, &from, &to);
                component.attribute_changed(name, &from, &to);
            } else {
                error!("Component id not found in changed attribute.", element_id.clone());
            }
        }
    };

    expanded
}

fn observed_quote(observed_js_fn: &str, struct_name: &Ident) -> proc_macro2::TokenStream {
    let js_callback_ident = Ident::new(observed_js_fn, Span::call_site());
    let rs_name_str = from_pascal_to_snake(observed_js_fn);
    let rs_name_ident = Ident::new(&rs_name_str, Span::call_site());
    let component_name = Ident::new(&format!("{}Component", struct_name), Span::call_site());

    let expanded = quote! {
        #[callback(#js_callback_ident)]
        fn #rs_name_ident() -> &'static [&'static str] {
            #component_name::internal_observed_attributes()
        }
    };

    expanded
}

fn store_name(struct_name: &Ident) -> String {
    let store_name = from_pascal_to_snake(&struct_name.to_string()).to_uppercase() + "_COMPONENTS";
    store_name
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

    let struct_name_str = struct_ident.to_string();
    let var_name_str = from_pascal_to_snake(struct_name_str.as_str());
    let var_ident = Ident::new(&var_name_str, Span::call_site());

    let rust_name = format!(
        "init_component_self_{}",
        from_pascal_to_snake(struct_name_str.as_str())
    );

    let mut kebab_case_name = from_pascal_to_snake(struct_name_str.as_str());
    kebab_case_name = kebab_case_name.replace("_", "-");

    let rust_ident = Ident::new(&rust_name, Span::call_site());
    let struct_ref_container =
        Ident::new(&format!("{}RefContainer", struct_ident), Span::call_site());

    let expanded = quote! {
        #[callback(#js_name)]
        #[allow(non_snake_case)]
        pub fn #rust_ident(current_id: String) -> String {  // init_component_state_struct_name, returns id for the instance of the element
            log!("Compoent generation previous id:", current_id.clone());

            let #var_ident = #struct_ident::default();

            let key = if current_id.is_empty() {
                let component_counter = ::kaja_web::NEXT_COMPONENT_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                format!("{}-{}", #kebab_case_name, component_counter)
            } else {
                current_id
            };

            let component = std::rc::Rc::new(std::cell::RefCell::new(#struct_ref_container {
                id: key.clone(),
                attribute_values: std::rc::Rc::new(std::cell::RefCell::new(#var_ident)),
            }));

            #store_ident.with(|store| {
                store.borrow_mut().insert(key.clone(), component);
            });

            log!("created key:", key.clone());

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
    _struct_name: &Ident,
) -> proc_macro2::TokenStream {
    let name_str_lit = LitStr::new(&class_creator.to_string(), Span::call_site());

    let expanded = quote! {
        ::inventory::submit! {
            InitComponentFn(#class_creator)
        }

        #[allow(non_snake_case)]
        pub fn #class_creator() { // createComponentTheStructName
        let component_name = #name_str_lit;
            let tag_name = #tag;

            let js_init = String::from(
                r#"
                        return class extends HTMLElement {
                            constructor() {
                                super();
                                let currentId = this.getAttribute("id");

                                if (currentId === null) {
                                    currentId = "";
                                }

                                let resolvedId = __INIT_SELF_CALLBACK(currentId);

                                if (!currentId) {
                                    this.setAttribute("id", resolvedId);
                                }
                            }

                            connectedCallback() {
                                let componentid = this.getAttribute("id");
                                __ATTACH_CALLBACK(componentid);
                            }

                            disconnectedCallback() {
                                let componentid = this.getAttribute("id");
                                __DETACH_CALLBACK(componentid);
                            }

                            attributeChangedCallback(name, oldValue, newValue) {
                                let componentid = this.getAttribute("id");
                                __ATTRIBUTE_CHANGED_CALLBACK(componentid, name, oldValue === null ? "" : String(oldValue), newValue === null ? "" : String(newValue));
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
                error!("Failed to register component", #tag);
            }
        }
    };

    expanded
}

fn create_component(struct_item: &ItemStruct) -> proc_macro2::TokenStream {
    let struct_ident = &struct_item.ident;
    let struct_ref_container =
        Ident::new(&format!("{}RefContainer", struct_ident), Span::call_site());
    let component_ident = Ident::new(&format!("{}Component", struct_ident), Span::call_site());

    let store_name_str = store_name(struct_ident);
    let store_ident = Ident::new(&store_name_str, Span::call_site());

    let (setter_arms, getter_arms, observed_attributes) = match &struct_item.fields {
        Fields::Named(fields) => {
            let setter_arms = fields.named.iter().filter_map(|field| {
                let field_ident = field.ident.as_ref()?;
                let field_name = field_ident.to_string();
                let field_ty = &field.ty;
                Some(quote! {
                    #field_name => {
                        self.#field_ident = value.parse::<#field_ty>().map_err(|_| ::kaja_web::Error)?;
                    }
                })
            });

            let getter_arms = fields.named.iter().filter_map(|field| {
                let field_ident = field.ident.as_ref()?;
                let field_name = field_ident.to_string();
                Some(quote! {
                    #field_name => Some(self.#field_ident.to_string())
                })
            });

            let observed_attributes = fields.named.iter().filter_map(|field| {
                let field_ident = field.ident.as_ref()?;
                let field_name = LitStr::new(&field_ident.to_string(), Span::call_site());
                Some(quote! { #field_name })
            });

            (
                quote! { #(#setter_arms,)* },
                quote! { #(#getter_arms,)* },
                quote! { #(#observed_attributes,)* },
            )
        }
        _ => (quote! {}, quote! {}, quote! {}),
    };

    let struct_name_lit = LitStr::new(&struct_ident.to_string(), Span::call_site());

    let expanded = quote! {
        thread_local! {
            static #store_ident: std::cell::RefCell<std::collections::HashMap<String, std::rc::Rc<std::cell::RefCell<#struct_ref_container>>>> =
                std::cell::RefCell::new(std::collections::HashMap::new());
        }

        impl #struct_ident {
            fn set_test_attribute(&mut self, name: &str, value: &str) {
                let _ = self.try_set_test_attribute(name, value);
            }

            fn try_set_test_attribute(&mut self, name: &str, value: &str) -> ::kaja_web::Result<()> {
                match name {
                    #setter_arms
                    _ => {
                        error!("No field", name, "in", #struct_name_lit, "can't set it to", value);
                        return Err(::kaja_web::Error);
                    }
                }

                Ok(())
            }

            fn get_attribute(&self, name: &str) -> Option<String> {
                match name {
                    #getter_arms
                    _ => {
                        error!("No field", name, "in", #struct_name_lit, "can't get it");
                        None
                    }
                }
            }
        }

        #[derive(Default)]
        struct #struct_ref_container {
            id: String,
            attribute_values: std::rc::Rc<std::cell::RefCell<#struct_ident>>,
        }

        impl #struct_ref_container {
            fn create_component(id: &str) {
                let handle = std::rc::Rc::new(std::cell::RefCell::new(#struct_ref_container {
                    id: id.to_string(),
                    attribute_values: std::rc::Rc::new(std::cell::RefCell::new(#struct_ident::default())),
                }));

                Self::register_component(handle.clone());
                let component = #component_ident::get_component(&id);

                if component.is_none() {
                    error!("Failed to register component.");
                    return;
                }

                let component_impl = component.unwrap();
                component_impl.connected();
            }

            fn register_component(component: std::rc::Rc<std::cell::RefCell<#struct_ref_container>>) {
                let id = component.borrow().id.clone();

                #store_ident.with(|components| {
                    components.borrow_mut().insert(id, component);
                });
            }
        }

        #[derive(Default)]
        struct #component_ident {
            handle: std::rc::Rc<std::cell::RefCell<#struct_ref_container>>,
        }

        impl #component_ident {
            fn get_component(id: &str) -> Option<#component_ident> {
                let handle = #store_ident.with(|components| components.borrow().get(id).cloned());

                if !handle.is_some() {
                    return None;
                }

                Some(#component_ident {
                    handle: handle.unwrap(),
                })
            }

            fn internal_observed_attributes() -> &'static [&'static str] {
                &[#observed_attributes]
            }

            fn internal_attributes_update(&self, name: &str, _old_value: &str, new_value: &str) {
                let handle = self.handle.borrow();
                let mut attr = handle.attribute_values.borrow_mut();
                attr.set_test_attribute(name, new_value);
            }

            fn set_attribute(&self, name: &str, value: &str) {
                let id = self.get_id();
                let element = get_element_by_id(&id);

                if element.is_none() {
                    error!("Element not found for id:", id, "in set_attribute");
                    return;
                }

                if !Self::internal_observed_attributes().contains(&name) {
                    error!("Attribute not observed:", name, "in set_attribute");
                    return;
                }

                let res = element.unwrap().set_attribute(name, value);

                if res.is_err() {
                    error!("Failed to set attribute:", name, "in set_attribute");
                }
            }

            fn get_attribute(&self, name: &str) -> Option<String> {
                let handle = self.handle.borrow();
                let attr = handle.attribute_values.borrow();
                attr.get_attribute(name)
            }

            fn get_attributes(&self) -> std::rc::Rc<std::cell::RefCell<#struct_ident>> {
                self.handle.borrow().attribute_values.clone()
            }

            fn get_id(&self) -> String {
                self.handle.borrow().id.clone()
            }
        }
    };

    expanded
}
