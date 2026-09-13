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
  const res = await fetch(url);
  if (!res.ok || !res.body) {
    throw new Error(`Download failed: ${res.status}`);
  }

  let stream = res.body;
  let saveName = filename;
  const wire = res.headers.get("x-compression") || compression || "identity";

  if (wire && wire !== "none" && wire !== "identity") {
    const format = wire === "gz" ? "gzip" : wire;
    try {
      stream = stream.pipeThrough(new DecompressionStream(format));
    } catch (err) {
      console.warn(`DecompressionStream(${format}) unsupported, saving raw`, err);
      saveName = `${filename}.${format === "gzip" ? "gz" : "zst"}`;
    }
  }

  if ("showSaveFilePicker" in window) {
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
