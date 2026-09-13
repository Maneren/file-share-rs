use leptos::prelude::*;

/// Button that streams a single file through `window.streamDownload`.
///
/// The server compresses on the fly (`/download/..?compress=zstd`), the
/// browser decompresses via native `DecompressionStream` and writes straight
/// to disk with `showSaveFilePicker`. Plain component on purpose: the
/// download is triggered by an inline `onclick`, no hydration needed.
#[component]
pub fn StreamDownloadButton(
    download_url: String,
    filename: String,
    #[prop(default = "zstd")] compression: &'static str,
) -> impl IntoView {
    view! {
      <button
        class="btn btn-ghost btn-xs shrink-0"
        title="Fast streaming download with on-the-fly decompression"
        data-url=download_url
        data-filename=filename
        data-compression=compression
        onclick="window.streamDownload(this.getAttribute('data-url'), this.getAttribute('data-filename'), this.getAttribute('data-compression'))"
      >
        "Stream"
      </button>
    }
}
