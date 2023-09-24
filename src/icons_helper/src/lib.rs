#![warn(clippy::pedantic)]
#![allow(clippy::missing_panics_doc)]

extern crate proc_macro;
use std::path::Path;

use proc_macro::{TokenStream, TokenTree};
use quote::quote;

#[proc_macro]
pub fn get_known_icon_extensions(tokens: TokenStream) -> TokenStream {
  let path_token = tokens.into_iter().next().expect("expected a path token");

  let TokenTree::Literal(literal) = path_token else {
    panic!("expected a literal");
  };

  let literal = literal.to_string();
  let path = Path::new(literal.trim_matches('"'));

  let extensions = std::fs::read_dir(path)
    .unwrap()
    .map(|entry| entry.unwrap().path())
    .filter(|path| path.is_file())
    .map(|path| path.file_stem().unwrap().to_string_lossy().to_string())
    .collect::<Vec<_>>();

  quote! { [#(#extensions),*] }.into()
}
