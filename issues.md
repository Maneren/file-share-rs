## file-share-rs — improvement opportunities

Verified against `server/src/**/*`, `app/src/**/*`.

### 1. Performance

- `server/src/fileserv.rs:56-63,64-73`: SHA-256 + hex per static request; ETag unquoted so `If-None-Match` never matches → no `304`. Precompute `OnceLock`, quote ETags, `immutable,max-age=31536000` for `/pkg/*`, `no-cache` for HTML.

### 2. Features / UI-UX gaps

- No delete/rename/move, no file-size/date columns on mobile.
- Upload: posts to `/upload` with native XHR progress (server-fn upload and
  SSE progress deleted); no drag-drop/progress-persist.
- `new_folder.rs:20,42,45`: string `onclick="showModal()"` breaks under CSP, optimistic close with no error display, no validation (`required/pattern/maxlength`), `unwrap/expect` on refs.
- `folder_download.rs`: `navigator.clipboard` string onclick fails on `http://LAN-IP` (needs secure context) with no fallback, `curl '...'` unescaped injection, hover-only dropdown inaccessible on touch/keyboard.
- `breadcrumbs.rs`, `file_entries.rs`: no `<nav aria-label>`, no `aria-current`, no `overflow-x-auto` (deep paths clip mobile), no `title`/ellipsis in breadcrumbs, no `role=progressbar`, `Loading...` bare `<p>` no spinner/`aria-live`.
- `app/src/lib.rs`, `server/src/main.rs`: `/` → `/index` but route is `/index/*path`, 404 renders with `200`.
- Missing flags: `--tls`, `--read-only`, `--hidden`.
