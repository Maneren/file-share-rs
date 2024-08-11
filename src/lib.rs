#![warn(clippy::pedantic)]
#![allow(clippy::wildcard_imports)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::must_use_candidate)]

use cfg_if::cfg_if;
pub mod app;
mod error_template;
pub mod utils;

cfg_if! {
if #[cfg(feature = "ssr")] {
  pub mod config;
} else if #[cfg(feature = "hydrate")] {
  use wasm_bindgen::prelude::wasm_bindgen;
  use crate::app::*;

  #[global_allocator]
  static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

  #[wasm_bindgen]
  pub fn hydrate() {
    // initializes logging using the `log` crate
    _ = console_log::init_with_level(log::Level::Debug);
    console_error_panic_hook::set_once();

    leptos::mount_to_body(App);
  }
}}
