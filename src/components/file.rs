use leptos::*;
use phf::{phf_map, Map};

use crate::{components::Entry, utils::SystemTime};

static ADDITIONAL_EXTENSIONS_MAPPING: Map<&'static str, &'static str> = phf_map! {
  "apk"=> "android",

  "mp3"=> "audio",
  "ogg" => "audio",
  "wav" => "audio",
  "flac" => "audio",

  "mp4" => "video",
  "mov" => "video",
  "mkv" => "video",
  "webm" => "video",
  "avi" => "video",
  "gif" => "video",

  "png" => "image",
  "jpg" => "image",
  "jpeg" => "image",
  "ico" => "image",
  "bmp" => "image",
  "webp" => "image",

  "doc" => "word",
  "docx" => "word",
  "odt" => "document",
  "rtf" => "document",

  "xls" => "table",
  "xlsx" => "table",

  "ppt" => "powerpoint",
  "pptx" => "powerpoint",

  "zip" => "archive",
  "rar" => "archive",
  "tar" => "archive",
  "7z" => "archive",
  "gz" => "archive",
  "bz2" => "archive",
  "xz" => "archive",
  "zst" => "archive",

  "conf" => "settings",
  "rc" => "settings",

  "bat" => "console",
  "sh" => "console",
  "zsh" => "console",

  "jsx" => "react",
  "tsx" => "react",

  "gitignore" => "git",

  "ttf" => "font",
  "otf" => "font",
  "woff" => "font",
  "woff2" => "font",
};

pub fn get_file_icon(extension: &str) -> &str {
  let KNOWN_EXTENSIONS = icons_helper::get_known_icon_extensions!();

  let extension = ADDITIONAL_EXTENSIONS_MAPPING
    .get(extension)
    .unwrap_or(&extension);

  if KNOWN_EXTENSIONS.contains(extension) {
    extension
  } else {
    "file"
  }
}

#[component]
pub fn File<'a>(
  path: &'a str,
  name: &'a str,
  extension: &'a str,
  size: String,
  last_modified: SystemTime,
) -> impl IntoView {
  view! {
    <a href=format!("/files/{path}") download>
      <Entry name=name icon=get_file_icon(extension) size=size last_modified=Some(last_modified)/>
    </a>
  }
}
