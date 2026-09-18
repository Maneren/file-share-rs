## file-share-rs — improvement opportunities

Verified against `server/src/*`, `app/src/**/*`.

### 1. Correctness / security (do first)

- `server/src/fileserv.rs:206,221-254`: `while let Ok(Some(...))` swallows multipart `Err` → `200 OK` on corrupt upload; mid-write failure leaves partial file; `create_new` exists → `500` should be `409`, perm → `403`. Delete partial on error, `match`/`?` instead.
- `server/src/main.rs:131`: `DefaultBodyLimit::disable()` globally. Scope limit to `/upload` only (`RequestBodyLimitLayer` + `--max-upload-size` flag), otherwise server-fns unbounded.
- `server/src/main.rs:130`: `ServeDir` has no error mapping and no `Trace`/`Timeout`/`CatchPanic` layers. (Symlink containment is now enforced by a gate in front of `ServeDir`; `nosniff`/`no-referrer`/`SAMEORIGIN` headers are sent. CSP intentionally skipped — it would block Leptos hydration inline scripts.)
- No auth/rate-limit: LAN-exposed + `--upload` = arbitrary write. Add `--auth-token`, `RateLimit`, archive max-size/depth/timeout, disconnect cancellation (`server/src/fileserv.rs:146-151` spawned task keeps compressing after client leaves).

### 2. Performance

- `server/src/fileserv.rs:56-63,64-73`: SHA-256 + hex per static request; ETag unquoted so `If-None-Match` never matches → no `304`. Precompute `OnceLock`, quote ETags, `immutable,max-age=31536000` for `/pkg/*`, `no-cache` for HTML.
- `app/src/server.rs:49-74`: full dir load+sort+serialize, no pagination. 10k+ entries = huge payload + WASM freeze. Add `limit/offset` + server sort.

### 3. Features / UI-UX gaps

- No delete/rename/move, no search/filter/sort toggle, no pagination, no hidden-file toggle, no empty-state CTA (`app/src/lib.rs:77-86` renders nothing), no file-size/date columns on mobile.
- Upload (`app/src/components/upload.rs:126,153-207`): stale `PathBuf` prop (navigating changes folder, upload goes to old), `listing` in `app/src/lib.rs:54-57` never invalidated on upload, ID = hash(filenames) collides, `.expect()` panics with no error UI, silent abort on concurrent upload, `create(true).truncate(true):94-99` overwrites silently, no drag-drop/cancel/progress-persist/input-reset.
- `upload/progress.rs:40-56`: `broadcast().expect()` panics if receiver left; fast-upload race (`finish` before subscribe → stuck at 0%); no TTL → leak. `use_upload_progress.rs:13-17` splits SSE on `\n` assuming chunk alignment → mis-parse.
- `new_folder.rs:20,42,45`: string `onclick="showModal()"` breaks under CSP, optimistic close with no error display, no validation (`required/pattern/maxlength`), `unwrap/expect` on refs.
- `folder_download.rs:13-51`: stale `base_path`, `navigator.clipboard` string onclick fails on `http://LAN-IP` (needs secure context) with no fallback, `curl '...'` unescaped injection, hover-only dropdown inaccessible on touch/keyboard.
- `breadcrumbs.rs:9-39`, `file_entries.rs:31-83`: `<a>` vs `<A>` inconsistency (full reload vs SPA), no `<nav aria-label>`, no `aria-current`, no `overflow-x-auto` (deep paths clip mobile), no `title`/ellipsis, folder/file links hijacked by router (`<A>` for download), no `role=progressbar`, `Loading...` bare `<p>` no spinner/`aria-live`.
- `app/src/lib.rs:61,116-125`: `expect_context<AppConfig>` no CSR provider (panics on client nav), `/` → `/index` but route is `/index/*path`, 404 renders with `200`.
- `server/src/main.rs:199,205-232`: QR `break` on first error hides rest; QR/display includes docker/link-local IPs. Filter to usable LAN, dedupe.
- Missing flags: `--max-upload-size`, `--tls`, `--auth`, `--read-only`, `--hidden`; `--port 3000` busy silently picks random port (TOCTOU) — only auto-pick on `--port 0`.
