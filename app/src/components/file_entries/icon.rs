use include_flate::flate;
use leptos::{prelude::*, IntoView};

flate!(static LANGUAGES_MAP_JSON: str from "assets/associations/language.json");
flate!(static EXTENSIONS_MAP_JSON: str from "assets/associations/extension.json");
flate!(static FOLDER_MAP_JSON: str from "assets/associations/folder.json");
flate!(static FILENAMES_MAP_JSON: str from "assets/associations/file.json");

use std::{collections::HashMap, sync::LazyLock};

use rust_embed::RustEmbed;

use crate::components::file_entries::EntryType;

flate!(static FILE_ICON: str from "assets/icons/file.svg");
flate!(static FOLDER_ICON: str from "assets/icons/folder.svg");

#[derive(RustEmbed)]
#[folder = "assets/icons"]
struct Icons;

fn get_icon(name: &str) -> Option<String> {
    let name = format!("{name}.svg");
    Icons::get(&name).map(|icon| String::from_utf8_lossy(icon.data.as_ref()).into_owned())
}

static LANGUAGES_MAP: LazyLock<HashMap<String, String>> =
    LazyLock::new(|| serde_json::from_str(&LANGUAGES_MAP_JSON).expect("The language map is valid"));

static FILENAMES_MAP: LazyLock<HashMap<String, String>> = LazyLock::new(|| {
    let file_map = serde_json::from_str::<HashMap<String, String>>(&FILENAMES_MAP_JSON)
        .expect("The filename map is valid");

    let extensions_name = serde_json::from_str::<HashMap<String, String>>(&EXTENSIONS_MAP_JSON)
        .expect("The extension map is valid");

    file_map.into_iter().chain(extensions_name).collect()
});

static FOLDER_MAP: LazyLock<HashMap<String, String>> =
    LazyLock::new(|| serde_json::from_str(&FOLDER_MAP_JSON).expect("The folder map is valid"));

fn longest_matching_suffix<'a, 'b>(
    target: &str,
    options: impl IntoIterator<Item = (&'b String, &'a String)>,
) -> Option<&'a String> {
    options
        .into_iter()
        .filter_map(|(ext, name)| target.ends_with(ext).then_some((ext, name)))
        .max_by_key(|(ext, _)| ext.len())
        .map(|(_, name)| name)
}

fn get_folder_icon(folder_name: &str) -> String {
    FOLDER_MAP
        .get(folder_name)
        .or_else(|| longest_matching_suffix(folder_name, FOLDER_MAP.iter()))
        .map(String::as_str)
        .and_then(get_icon)
        .unwrap_or_else(|| FOLDER_ICON.clone())
}

fn get_file_icon(file_name: &str) -> String {
    let file_type =
        longest_matching_suffix(file_name, FILENAMES_MAP.iter()).map_or("file", String::as_str);

    get_icon(file_type)
        .or_else(|| LANGUAGES_MAP.get(file_type).and_then(|lang| get_icon(lang)))
        .unwrap_or_else(|| FILE_ICON.clone())
}

#[allow(clippy::needless_pass_by_value)]
#[component]
pub fn Icon(type_: EntryType, name: String) -> impl IntoView {
    let icon = match type_ {
        EntryType::File => get_file_icon(&name),
        EntryType::Folder => get_folder_icon(&name),
    };
    view! { <div class="icon" inner_html=icon /> }
}
