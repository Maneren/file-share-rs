use std::{collections::HashMap, sync::LazyLock};

use include_flate::flate;
use rust_embed::RustEmbed;
use serde::Deserialize;

flate!(static ICONS_JSON: str from "assets/icons.json");
flate!(static FILE_ICON: str from "assets/icons/file.svg");
flate!(static FOLDER_ICON: str from "assets/icons/folder.svg");

#[derive(RustEmbed)]
#[compression]
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

/// Look up a decoded SVG by icon name. Returns a borrowed slice, so
/// listing hundreds of files doesn't clone kilobytes of SVG per row.
fn get_icon(name: &str) -> Option<&'static str> {
    let key = format!("{name}.svg");
    DECODED_ICONS.get(&key).map(String::as_str)
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

pub(crate) fn get_folder_icon(folder_name: &str) -> &'static str {
    let lowercase = folder_name.to_ascii_lowercase();
    let trimmed = lowercase.trim_matches(['_', ' ', '.']);

    if trimmed.is_empty() {
        return &FOLDER_ICON;
    }

    ICON_MAPS
        .folders
        .get(trimmed)
        .or_else(|| longest_matching_suffix(trimmed, &ICON_MAPS.folders))
        .and_then(|name| get_icon(name))
        .unwrap_or(&FOLDER_ICON)
}

pub(crate) fn get_file_icon(file_name: &str) -> &'static str {
    let lower = file_name.to_ascii_lowercase();

    // Exact filename match (`Dockerfile`, `Makefile`, …).
    if let Some(svg) = FILENAMES_MAP
        .get(lower.as_str())
        .and_then(|name| get_icon(name))
    {
        return svg;
    }

    // Longest dotted suffix, probed longest-first in O(dots) `HashMap`
    // lookups (covers `.tar.gz`, `.d.ts`); the full scan below remains
    // for exotic unanchored fragments, preserving old behavior exactly.
    for (dot, _) in lower.match_indices('.') {
        if let Some(svg) = FILENAMES_MAP
            .get(&lower[dot..])
            .and_then(|name| get_icon(name))
        {
            return svg;
        }
    }

    longest_matching_suffix(&lower, &*FILENAMES_MAP)
        .and_then(|name| get_icon(name))
        .unwrap_or(&FILE_ICON)
}
