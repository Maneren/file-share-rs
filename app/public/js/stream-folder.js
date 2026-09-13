// Streaming folder extraction prototype.
//
// Pipeline: fetch(`/archive/<path>?method=tar.zst|tar.gz|tar`)
//   -> DecompressionStream (zstd/gzip/none)
//   -> minimal USTAR parser (regular files + dirs, GNU longnames)
//   -> File System Access `showDirectoryPicker`, one WritableStream per file.
//
// Nothing is buffered: peak RAM is a few hundred KiB regardless of folder
// size, so 100 GiB transfers only touch disk. Requires Chromium
// (`showDirectoryPicker`, secure context); other browsers fall back to a
// plain archive download. `zstd` in `DecompressionStream` is still rolling
// out — on TypeError we abort with a hint to retry `tar.gz`.
//
// Prototype limits (documented, not bugs): no PAX/symlink/hardlink restore
// (skipped + counted), no uid/gid/mtime restore, no tar checksum validation,
// no resume. Server writes plain USTAR via tokio-tar, so this is sufficient.
"use strict";

const TAR_BLOCK = 512;

const dec = new TextDecoder();

function decodeName(bytes, start, len) {
  const slice = bytes.subarray(start, start + len);
  const nul = slice.indexOf(0);
  return dec.decode(nul < 0 ? slice : slice.subarray(0, nul));
}

function parseOctal(bytes, start, len) {
  const text = decodeName(bytes, start, len).trim();
  if (text === "") return 0;
  const n = parseInt(text, 8);
  return Number.isSafeInteger(n) && n >= 0 ? n : NaN;
}

function isZeroBlock(bytes) {
  for (let i = 0; i < TAR_BLOCK; i++) {
    if (bytes[i] !== 0) return false;
  }
  return true;
}

// Sanitizes a tar member name to a relative path inside the target dir.
// Returns an array of path components, or null when the entry must be skipped.
function sanitizeTarPath(name) {
  const parts = name.split("/").filter((p) => p !== "" && p !== ".");
  if (parts.length === 0) return null;
  if (parts.some((p) => p === "..")) return null;
  return parts;
}

// Buffered pull reader over a WHATWG ReadableStream<Uint8Array>.
class StreamBuffer {
  constructor(reader) {
    this.reader = reader;
    this.chunks = [];
    this.head = 0; // consumed offset into chunks[0]
    this.total = 0; // unconsumed bytes across chunks
    this.done = false;
  }

  async fill(minBytes) {
    while (this.total < minBytes && !this.done) {
      const { done, value } = await this.reader.read();
      if (done) {
        this.done = true;
        break;
      }
      if (value && value.length > 0) {
        this.chunks.push(value);
        this.total += value.length;
      }
    }
    if (this.total < minBytes) {
      throw new Error("Unexpected end of archive stream");
    }
  }

  take(n) {
    const out = new Uint8Array(n);
    let off = 0;
    while (off < n) {
      const head = this.chunks[0];
      const avail = head.length - this.head;
      const want = Math.min(avail, n - off);
      out.set(head.subarray(this.head, this.head + want), off);
      this.head += want;
      off += want;
      this.total -= want;
      if (this.head >= head.length) {
        this.chunks.shift();
        this.head = 0;
      }
    }
    return out;
  }

  async takeExactly(n) {
    await this.fill(n);
    return this.take(n);
  }

  // Up to n bytes without failing at EOF; returns null when fully drained.
  async takeUpTo(n) {
    if (this.total === 0 && !this.done) {
      const { done, value } = await this.reader.read();
      if (done) {
        this.done = true;
      } else if (value && value.length > 0) {
        this.chunks.push(value);
        this.total += value.length;
      }
    }
    if (this.total === 0) return null;
    return this.take(Math.min(n, this.total));
  }

  async skip(n) {
    while (n > 0) {
      if (this.total === 0) {
        if (this.done) throw new Error("Unexpected end of archive stream");
        await this.fill(1);
      }
      const head = this.chunks[0];
      const avail = head.length - this.head;
      const want = Math.min(avail, n);
      this.head += want;
      this.total -= want;
      n -= want;
      if (this.head >= head.length) {
        this.chunks.shift();
        this.head = 0;
      }
    }
  }
}

async function mkdirp(root, parts, cache) {
  let key = "";
  let dir = root;
  for (const part of parts) {
    key += `/${part}`;
    let next = cache.get(key);
    if (!next) {
      next = await dir.getDirectoryHandle(part, { create: true });
      cache.set(key, next);
    }
    dir = next;
  }
  return dir;
}

async function extractTarStream(tarStream, rootHandle, onProgress) {
  const buf = new StreamBuffer(tarStream.getReader());
  const dirCache = new Map();
  let pendingLongName = null;
  let files = 0;
  let bytes = 0;
  let skipped = 0;

  for (;;) {
    const header = await buf.takeExactly(TAR_BLOCK);
    if (isZeroBlock(header)) break; // end-of-archive (prototype: first zero block)

    let name = decodeName(header, 0, 100);
    const prefix = decodeName(header, 345, 155);
    if (prefix !== "") name = `${prefix}/${name}`;
    const size = parseOctal(header, 124, 12);
    if (Number.isNaN(size)) throw new Error("Corrupt tar header: bad size field");
    const typeflag = String.fromCharCode(header[156]);
    const dataBlocks = Math.ceil(size / TAR_BLOCK);
    const padding = dataBlocks * TAR_BLOCK - size;

    // GNU longname: data block holds the real name for the *next* entry.
    if (typeflag === "L" || typeflag === "K") {
      const raw = await buf.takeExactly(dataBlocks * TAR_BLOCK);
      pendingLongName = dec.decode(raw.subarray(0, size)).replace(/\0.*$/, "");
      continue;
    }
    if (pendingLongName !== null) {
      name = pendingLongName;
      pendingLongName = null;
    }
    // PAX extended headers carry metadata we don't restore; skip payload.
    if (typeflag === "x" || typeflag === "g") {
      await buf.skip(dataBlocks * TAR_BLOCK);
      continue;
    }

    const parts = sanitizeTarPath(name);
    if (parts === null) {
      await buf.skip(dataBlocks * TAR_BLOCK);
      skipped++;
      continue;
    }

    if (typeflag === "5") {
      await mkdirp(rootHandle, parts, dirCache);
      if (size > 0) await buf.skip(dataBlocks * TAR_BLOCK);
      continue;
    }

    if (typeflag === "0" || typeflag === "\0" || typeflag === "7" || typeflag === "") {
      const parent = parts.slice(0, -1);
      const dir = parent.length > 0 ? await mkdirp(rootHandle, parent, dirCache) : rootHandle;
      const fileHandle = await dir.getFileHandle(parts[parts.length - 1], { create: true });
      const writable = await fileHandle.createWritable();
      try {
        let remaining = size;
        while (remaining > 0) {
          const chunk = await buf.takeUpTo(Math.min(remaining, 256 * 1024));
          if (chunk === null) throw new Error("Unexpected end of archive stream");
          await writable.write(chunk);
          remaining -= chunk.length;
          bytes += chunk.length;
        }
        if (padding > 0) await buf.skip(padding);
        await writable.close();
      } catch (err) {
        try {
          await writable.abort();
        } catch (_) {
          // ignore abort errors, the original error matters
        }
        throw err;
      }
      files++;
      if (onProgress) onProgress({ files, bytes, skipped });
      continue;
    }

    // Symlinks/hardlinks/dev nodes: not restored by this prototype.
    await buf.skip(dataBlocks * TAR_BLOCK);
    skipped++;
  }

  return { files, bytes, skipped };
}

async function streamFolder(url, compression, button) {
  const absUrl = new URL(url, window.location.origin).toString();
  const setLabel = (text) => {
    if (button) button.textContent = text;
  };
  const original = button ? button.textContent : "";

  if (!("showDirectoryPicker" in window)) {
    // Fallback for old browsers: plain archive download, server compresses.
    const a = document.createElement("a");
    a.href = absUrl;
    a.download = "";
    document.body.appendChild(a);
    a.click();
    a.remove();
    return;
  }

  let rootHandle;
  try {
    rootHandle = await window.showDirectoryPicker({ mode: "readwrite" });
  } catch (err) {
    if (err && err.name === "AbortError") return; // user cancelled
    throw err;
  }

  try {
    if (button) button.disabled = true;
    setLabel("Starting…");

    const res = await fetch(absUrl);
    if (!res.ok || !res.body) throw new Error(`Download failed: ${res.status}`);

    let stream = res.body;
    if (compression && compression !== "none") {
      const format = compression === "gz" ? "gzip" : compression;
      try {
        stream = stream.pipeThrough(new DecompressionStream(format));
      } catch (err) {
        throw new Error(
          `This browser cannot decode ${format} here — retry with tar.gz. (${err})`,
        );
      }
    }

    const result = await extractTarStream(stream, rootHandle, ({ files, bytes }) => {
      const mib = (bytes / (1024 * 1024)).toFixed(1);
      setLabel(`${files} files · ${mib} MiB`);
    });
    setLabel(`Done: ${result.files} files${result.skipped > 0 ? ` (${result.skipped} skipped)` : ""}`);
  } catch (err) {
    console.error(err);
    setLabel("Failed — see console");
    throw err;
  } finally {
    if (button) {
      button.disabled = false;
      setTimeout(() => {
        if (button.textContent.startsWith("Done:") || button.textContent.startsWith("Failed")) {
          button.textContent = original;
        }
      }, 6000);
    }
  }
}

window.streamFolder = streamFolder;
