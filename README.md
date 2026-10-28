# Kaja Web Component Example

A simple example of a custom HTML component using Rust and Kaja Web. Similar in functionality to 
other web component libraries but small and fast.

## Component

```rust
use kaja_web::prelude::*;

#[component("main-component")]
struct Main {
    #[attribute]
    test: bool,
}
```

The `#[component]` macro defines a custom HTML element, named `main-component` in this example. 
The `#[attribute]` macro associates the `test` field with an HTML attribute. Attributes can be
updated with main-component.set_attribute("something", "somethingelse");. Or used in html like this:
`<main-component test="true"><main-component>`

## Implementation

The macro generates a new empty impl called MainComponent, different from Main but with access to 
the same functionality.

```rust
use kaja_web::prelude::*;

impl Component for MainComponent {
    fn connected(&self) {
        log!("Connected:", self.get_id());
    }

    fn disconnected(&self) {
        log!("Disconnected:", self.get_id());
    }

    fn attribute_changed(&self, name: &str, _old_value: &str, _new_value: &str) {
        if name == "test" {
            self.render();
        }
    }
}
```

## Rendering

Rendering can be done in `attribute_changed` but it is a separate function in thie example.

```rust
use kaja_web::prelude::*;

impl MainComponent {
    fn render(&self) {
        let test: bool = self.get_attribute("test").unwrap_or_default();

        let content = html! {{
            <p>Test: $test</p>
        }};

        let id = self.get_id();

        if let Some(element) = get_element_by_id(&id) {
            element.set_inner_html(content.as_str());
        } else {
            error!("Element not found for component id:", id);
        }
    }
}
```

## Startup

```rust
use kaja_web::prelude::*;

#[wasm_bindgen(start)]
pub fn init() {
    let elements = get_elements("main-component");
    
    for element in elements {
        let id = element.id();
    
        if id.is_empty() {
            continue;
        }
    
        if let Some(element) = get_element_by_id(&id) {
            let test = true;
            let _ = element.set_attribute("test", test.to_string().as_str());
        }
    }
}
```

## Usage

```html
<main-component test="false"></main-component>
```
