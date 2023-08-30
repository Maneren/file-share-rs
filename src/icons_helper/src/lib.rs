#![warn(clippy::pedantic)]
#![allow(clippy::missing_panics_doc)]

extern crate proc_macro;
use std::path::Path;

use proc_macro::TokenStream;
use quote::quote;

#[proc_macro]
pub fn get_known_icon_extensions(_tokens: TokenStream) -> TokenStream {
  let path = Path::new("public/icons");

  let extensions = std::fs::read_dir(path)
    .unwrap()
    .map(|entry| entry.unwrap().path())
    .filter(|path| path.is_file())
    .map(|path| path.file_stem().unwrap().to_string_lossy().to_string())
    .collect::<Vec<_>>();

  quote! { [#(#extensions),*] }.into()
}
