use cfg_if::cfg_if;

pub mod components;
pub mod configuration;
pub mod prometheus_client;
pub mod routes;
pub mod startup;
pub mod telemtry;

cfg_if! {
if #[cfg(feature = "hydrate")] {

  use wasm_bindgen::prelude::wasm_bindgen;
  use components::App;

    #[wasm_bindgen]
    pub fn hydrate() {
      use leptos::*;

      console_error_panic_hook::set_once();

      leptos::mount_to_body(move || {
          view! { <App/> }
      });
    }
}
}
