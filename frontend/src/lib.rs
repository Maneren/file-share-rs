#![warn(clippy::pedantic)]
#![allow(clippy::wildcard_imports)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::must_use_candidate)]

use console_error_panic_hook::set_once;
use console_log::init_with_level;
#[allow(unused_imports)]
use file_share_app::*;
use log::Level;
use wasm_bindgen::prelude::wasm_bindgen;

#[wasm_bindgen]
pub fn hydrate() {
    // initializes logging using the `log` crate
    let level = if cfg!(debug_assertions) {
        Level::Debug
    } else {
        Level::Warn
    };
    _ = init_with_level(level);
    set_once();
    leptos::mount::hydrate_islands();
}
