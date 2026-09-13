// Streaming download with on-the-fly client decompression.
//
// Server streams `/download/<path>?compress=zstd|gzip|none` and marks the
// wire format in `x-compression`. The browser decompresses via the native
// `DecompressionStream` and pipes straight to disk via the File System Access
// API, so multi-GiB files never sit in RAM. Falls back to a blob download
// (fine for small files / old browsers).
//
// `zstd` in `DecompressionStream` is still rolling out; if the browser throws
// we download the raw bytes with the `.zst` suffix so nothing is lost.
async function streamDownload(url, filename, compression) {
  const caps = window.FileShareCaps;
  if (caps) caps.logEnv("stream-download");

  const res = await fetch(url);
  if (!res.ok || !res.body) {
    throw new Error(`Download failed: ${res.status}`);
  }

  let stream = res.body;
  let saveName = filename;
  const wire = res.headers.get("x-compression") || compression || "identity";
  if (caps) {
    console.info(
      `[file-share:stream-download] requested=${compression || "?"} wire=${wire} — ` +
        (res.headers.get("x-compression")
          ? "server-advertised wire format wins over the button default"
          : "no x-compression header, using the button default"),
    );
  }

  const decode = caps
    ? caps.pickDecode(stream, wire)
    : { stream, format: wire, how: "native", reason: "capability helper missing, trying native decode" };
  stream = decode.stream;
  if (caps) caps.log("stream-download:decode", { how: `decode-${decode.how}`, format: decode.format, reason: decode.reason });
  if (decode.how === "raw-fallback") {
    // zstd still rolling out: keep the bytes, just fix the suffix.
    saveName = `${filename}.${decode.format === "gzip" ? "gz" : "zst"}`;
  }

  const save = caps
    ? caps.pickSaveMethod("file")
    : {
        method: "showSaveFilePicker" in window ? "picker" : "blob",
        reason: "capability helper missing, feature-detecting inline",
      };
  if (caps) caps.log("stream-download:save", save);

  if (save.method === "picker") {
    const handle = await window.showSaveFilePicker({ suggestedName: saveName });
    const writable = await handle.createWritable();
    try {
      await stream.pipeTo(writable);
    } catch (err) {
      try {
        await writable.abort();
      } catch (_) {
        // ignore abort errors, the original error matters
      }
      throw err;
    }
  } else {
    const blob = await new Response(stream).blob();
    const a = document.createElement("a");
    a.href = URL.createObjectURL(blob);
    a.download = saveName;
    document.body.appendChild(a);
    a.click();
    setTimeout(() => {
      URL.revokeObjectURL(a.href);
      a.remove();
    }, 1000);
  }
}

window.streamDownload = streamDownload;
