#![warn(clippy::pedantic)]
#![allow(clippy::wildcard_imports)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::must_use_candidate)]

use file_share_app::*;
use wasm_bindgen::prelude::wasm_bindgen;

#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[wasm_bindgen]
pub fn hydrate() {
  // initializes logging using the `log` crate
  _ = console_log::init_with_level(log::Level::Debug);
  console_error_panic_hook::set_once();

  leptos::mount::hydrate_body(App);
}
