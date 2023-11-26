#![warn(clippy::pedantic)]
#![allow(clippy::wildcard_imports)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::unsafe_derive_deserialize)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::must_use_candidate)]
#![feature(generic_arg_infer)]

use cfg_if::cfg_if;
pub mod app;
mod error_template;
mod pages;
pub mod utils;

cfg_if! {
    if #[cfg(feature = "ssr")] {

pub mod config;
pub mod fileserv;

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
