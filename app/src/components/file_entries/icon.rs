use std::{collections::HashMap, sync::LazyLock};

use include_flate::flate;
use leptos::{IntoView, prelude::*};
use rust_embed::RustEmbed;
use serde::Deserialize;

use crate::components::file_entries::EntryType;

flate!(static ICONS_JSON: str from "assets/icons.json");
flate!(static FILE_ICON: str from "assets/icons/file.svg");
flate!(static FOLDER_ICON: str from "assets/icons/folder.svg");

#[derive(RustEmbed)]
#[folder = "assets/icons"]
struct Icons;

#[derive(Deserialize)]
struct IconMaps {
    extensions: HashMap<String, String>,
    languages: HashMap<String, String>,
    filenames: HashMap<String, String>,
    folders: HashMap<String, String>,
}

static DECODED_ICONS: LazyLock<HashMap<String, String>> = LazyLock::new(|| {
    Icons::iter()
        .filter_map(|key| {
            Icons::get(&key).map(|file| {
                (
                    key.into_owned(),
                    String::from_utf8_lossy(file.data.as_ref()).into_owned(),
                )
            })
        })
        .collect()
});

fn get_icon(name: &str) -> Option<String> {
    let key = format!("{name}.svg");
    DECODED_ICONS.get(&key).cloned()
}

static ICON_MAPS: LazyLock<IconMaps> =
    LazyLock::new(|| serde_json::from_str(&ICONS_JSON).expect("Icon maps are valid"));

static FILENAMES_MAP: LazyLock<HashMap<String, String>> = LazyLock::new(|| {
    ICON_MAPS
        .extensions
        .iter()
        .chain(ICON_MAPS.languages.iter())
        .map(|(k, v)| (format!(".{k}"), v.clone()))
        .chain(
            ICON_MAPS
                .filenames
                .iter()
                .map(|(k, v)| (k.clone(), v.clone())),
        )
        .collect()
});

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
    let lowercase = folder_name.to_ascii_lowercase();
    let trimmed = lowercase.trim_matches(['_', ' ', '.']);

    if trimmed.is_empty() {
        return FOLDER_ICON.clone();
    }

    ICON_MAPS
        .folders
        .get(trimmed)
        .or_else(|| longest_matching_suffix(trimmed, &ICON_MAPS.folders))
        .and_then(|name| get_icon(name))
        .unwrap_or_else(|| FOLDER_ICON.clone())
}

fn get_file_icon(file_name: &str) -> String {
    longest_matching_suffix(&file_name.to_ascii_lowercase(), &*FILENAMES_MAP)
        .and_then(|name| get_icon(name))
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
