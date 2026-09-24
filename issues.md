## file-share-rs — improvement opportunities

Verified against `server/src/**/*`, `app/src/**/*`.

### 1. Correctness / security (do first)

- `server/src/fileserv/upload.rs:58`: `while let Ok(Some(...))` swallows multipart `Err` → `200 OK` on corrupt upload; mid-write failure leaves partial file; `create_new` error always maps to `500` (exists → `409`, denied → `403`). Delete partial on error, `match` instead. (The browser server-fn in `app/src/api/upload.rs` propagates read errors correctly but also leaves partials — same cleanup needed.)
- `server/src/router.rs:92`: `DefaultBodyLimit::disable()` globally → server-fns unbounded. When `--max-upload-size` is passed, apply `RequestBodyLimitLayer` (bounds uploads and server-fns); otherwise keep back-compat behavior. All upload/auth/rate limiting is gated behind CLI flags (off by default).
- `server/src/router.rs`: no `Trace`/`CatchPanic` layers. (Symlink containment gate in `fileserv/gate.rs` + `nosniff`/`no-referrer`/`SAMEORIGIN` headers done; CSP intentionally skipped — it would block Leptos hydration inline scripts. No global `Timeout` — it would kill legit long uploads/archives; archive streaming gets a per-request `--archive-timeout` instead.)
- No auth/rate-limit: LAN-exposed + `--upload` = arbitrary write. Add `--auth-token` (global middleware, `Bearer` or cookie + `/login` form so the browser UI keeps working), per-IP `--rate-limit` (token bucket, `429` + `Retry-After`), archive `--max-archive-size`/`--max-archive-depth`/`--archive-timeout`, abort the compressor task on client disconnect (`archive_handler.rs:111-118` spawned task keeps compressing after the client leaves).
- `server/src/cli.rs:90-93`: busy `--port` silently picks a random port (TOCTOU) — only auto-pick on `--port 0`, otherwise fail.

### 2. Performance

- `server/src/fileserv.rs:56-63,64-73`: SHA-256 + hex per static request; ETag unquoted so `If-None-Match` never matches → no `304`. Precompute `OnceLock`, quote ETags, `immutable,max-age=31536000` for `/pkg/*`, `no-cache` for HTML.

### 3. Features / UI-UX gaps

- No delete/rename/move, no file-size/date columns on mobile.
- Upload (`app/src/components/upload.rs`): `listing` never invalidated on upload, ID = hash(filenames) collides, `.expect()` panics with no error UI, silent abort on concurrent upload, `create(true).truncate(true)` overwrites silently, no drag-drop/cancel/progress-persist/input-reset.
- `upload/progress.rs:40-56`: `broadcast().expect()` panics if receiver left; fast-upload race (`finish` before subscribe → stuck at 0%); no TTL → leak. `use_upload_progress.rs:13-17` splits SSE on `\n` assuming chunk alignment → mis-parse.
- `new_folder.rs:20,42,45`: string `onclick="showModal()"` breaks under CSP, optimistic close with no error display, no validation (`required/pattern/maxlength`), `unwrap/expect` on refs.
- `folder_download.rs`: `navigator.clipboard` string onclick fails on `http://LAN-IP` (needs secure context) with no fallback, `curl '...'` unescaped injection, hover-only dropdown inaccessible on touch/keyboard.
- `breadcrumbs.rs`, `file_entries.rs`: no `<nav aria-label>`, no `aria-current`, no `overflow-x-auto` (deep paths clip mobile), no `title`/ellipsis in breadcrumbs, no `role=progressbar`, `Loading...` bare `<p>` no spinner/`aria-live`.
- `app/src/lib.rs`, `server/src/main.rs`: `/` → `/index` but route is `/index/*path`, 404 renders with `200`.
- Missing flags: `--tls`, `--read-only`, `--hidden` (`--max-upload-size`, `--auth-token`, `--rate-limit`, archive caps, and `--port 0` auto-pick done — see §1).
