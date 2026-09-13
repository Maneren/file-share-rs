// Shared browser-capability probing for the streaming downloads.
//
// Every automatic choice (native decode vs raw bytes, picker vs blob/anchor,
// streaming extract vs plain archive) goes through here so the *reason* is
// logged in one consistent `[file-share]` format. Open devtools console to
// see why a given method was picked on a given browser.
"use strict";

window.FileShareCaps = (() => {
  const dsCache = Object.create(null);

  function dsProbe(format) {
    if (format in dsCache) return dsCache[format];
    let result;
    if (typeof DecompressionStream === "undefined") {
      result = { supported: false, reason: "DecompressionStream API missing from this browser" };
    } else {
      try {
        // Construction throws TypeError for unknown/unsupported formats.
        new DecompressionStream(format);
        result = { supported: true, reason: `native DecompressionStream("${format}") available` };
      } catch (err) {
        result = {
          supported: false,
          reason: `DecompressionStream("${format}") threw (${err && err.name ? err.name : err}); wire bytes cannot be decoded natively here`,
        };
      }
    }
    dsCache[format] = result;
    return result;
  }

  function secureContextReason() {
    if (typeof window.isSecureContext === "undefined") return "isSecureContext unknown";
    return window.isSecureContext
      ? "secure context"
      : "NOT a secure context (pickers require HTTPS/localhost/secure context)";
  }

  // File-saving path: picker streams to disk with O(chunk) RAM, blob buffers.
  function pickSaveMethod(kind) {
    const api = kind === "dir" ? "showDirectoryPicker" : "showSaveFilePicker";
    if (!(api in window)) {
      return {
        method: kind === "dir" ? "anchor" : "blob",
        reason: `${api} missing from this browser (Chromium-only API) — falling back to buffered download`,
      };
    }
    if (window.isSecureContext === false) {
      return {
        method: kind === "dir" ? "anchor" : "blob",
        reason: `${api} present but unusable: ${secureContextReason()} — falling back to buffered download (serve over HTTPS or localhost to unlock streaming to disk)`,
      };
    }
    return {
      method: "picker",
      reason: `${api} present + ${secureContextReason()} — streaming straight to disk`,
    };
  }

  // Wire-format decoding path. Returns { stream, format, how, reason }.
  function pickDecode(stream, wire) {
    if (!wire || wire === "none" || wire === "identity") {
      return { stream, format: "identity", how: "passthrough", reason: "wire is uncompressed — no decoder needed" };
    }
    const format = wire === "gz" ? "gzip" : wire;
    const probe = dsProbe(format);
    if (probe.supported) {
      return {
        stream: stream.pipeThrough(new DecompressionStream(format)),
        format,
        how: "native",
        reason: probe.reason,
      };
    }
    return { stream, format, how: "raw-fallback", reason: probe.reason };
  }

  function log(tag, choice) {
    console.info(
      `[file-share:${tag}] method=${choice.method || choice.how || "?"}` +
        (choice.format ? ` format=${choice.format}` : "") +
        ` — ${choice.reason}`,
    );
  }

  function logEnv(tag) {
    const ua = (navigator.userAgentData && navigator.userAgentData.brands
      ? navigator.userAgentData.brands.map((b) => `${b.brand}/${b.version}`).join(" ")
      : navigator.userAgent);
    console.info(
      `[file-share:${tag}] browser="${ua}" secureContext=${window.isSecureContext} ` +
        `pickers(save=${"showSaveFilePicker" in window},dir=${"showDirectoryPicker" in window}) ` +
        `DS(gzip=${dsProbe("gzip").supported},zstd=${dsProbe("zstd").supported})`,
    );
  }

  return { dsProbe, pickSaveMethod, pickDecode, log, logEnv };
})();
