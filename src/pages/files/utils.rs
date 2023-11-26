use std::{ffi::OsStr, path::Path};

pub fn os_to_string(str: impl AsRef<OsStr>) -> String {
  str.as_ref().to_string_lossy().to_string()
}

pub fn get_file_extension(path: impl AsRef<Path>) -> String {
  path
    .as_ref()
    .extension()
    .map(os_to_string)
    .unwrap_or_default()
}

pub fn get_file_icon(name: &str) -> String {
  let KNOWN_EXTENSIONS = icons_helper::get_known_icon_extensions!("public/icons");

  let extension = get_file_extension(name);
  let extension_str = extension.as_str();

  let extension = icons::ADDITIONAL_EXTENSIONS_MAPPING
    .get(&extension)
    .unwrap_or(&extension_str);

  if KNOWN_EXTENSIONS.contains(extension) {
    (*extension).to_string()
  } else {
    "file".into()
  }
}

mod icons {
  use phf::{phf_map, Map};

  pub(super) static ADDITIONAL_EXTENSIONS_MAPPING: Map<&'static str, &'static str> = phf_map! {
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
}
