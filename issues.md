## file-share-rs — improvement opportunities

Verified against `server/src/*`, `app/src/**/*`.

### 1. Correctness / security (do first)

- `server/src/fileserv.rs:168-174,185` + `app/src/server.rs:91-96`: `safe_join_*` is lexical only (`is_safe_relative_path` + `join`). Symlink inside share (`link -> /etc`) escapes via `upload/link`, `new_folder`, `archive/link`. `list_dir` does `canonicalize()+starts_with()` — unify all paths on that. Canonicalize `target_dir` once at startup.
- `server/src/fileserv.rs:153-161`: `Content-Disposition: filename="{file_name}"` from on-disk name + `.expect()`. Name with `"`/newline/non-ASCII panics (DoS) / header injection. Sanitize + use `filename*` RFC5987, return `400` not `expect`.
- `server/src/fileserv.rs:121-151`: archive sends `200` + headers before validating path exists/is-dir. Failure only closes pipe → truncated archive with `200`. Validate synchronously first.
- `server/src/fileserv.rs:206,221-254`: `while let Ok(Some(...))` swallows multipart `Err` → `200 OK` on corrupt upload; mid-write failure leaves partial file; `create_new` exists → `500` should be `409`, perm → `403`. Delete partial on error, `match`/`?` instead.
- `server/src/main.rs:131`: `DefaultBodyLimit::disable()` globally. Scope limit to `/upload` only (`RequestBodyLimitLayer` + `--max-upload-size` flag), otherwise server-fns unbounded.
- `app/src/utils.rs:61-80`: `encode_path` encodes `/` → `%2F` (`/files/foo%2Fbar` 404s on many static servers). Encode per-component, join with `/`. `try_decode_path:66-71` silently falls back on error + double-decode vs Axum `Path` (already decoded): `%252e` bypass risk. Decode exactly once.
- `server/src/main.rs:130`: bare `ServeDir::new(&target_dir)`. Confirm symlink containment, add `handle_error`, security headers (CSP, `X-Content-Type-Options`), `Trace/Timeout/CatchPanic` layers.
- No auth/rate-limit: LAN-exposed + `--upload` = arbitrary write. Add `--auth-token`, `RateLimit`, archive max-size/depth/timeout, disconnect cancellation (`server/src/fileserv.rs:146-151` spawned task keeps compressing after client leaves).

### 2. Performance

- `server/src/fileserv.rs:56-63,64-73`: SHA-256 + hex per static request; ETag unquoted so `If-None-Match` never matches → no `304`. Precompute `OnceLock`, quote ETags, `immutable,max-age=31536000` for `/pkg/*`, `no-cache` for HTML.
- `server/src/fileserv.rs:232-254`: per-chunk (~8KB) `write_all`, no `BufWriter`. Wrap in `BufWriter` / `io::copy`.
- `server/src/fileserv/archive.rs`: zip always `Deflate` (recompresses jpg/mp4), `comment(Local::now())` non-deterministic + TZ leak. Use `Stored` for incompressible exts.
- `app/src/server.rs:49-74`: full dir load+sort+serialize, no pagination. 10k+ entries = huge payload + WASM freeze. Add `limit/offset` + server sort.
- `app/src/components/file_entries/icon.rs:27-43`: `LazyLock<HashMap>` decodes all SVGs at startup, `.cloned()` per row. Return `&'static str`, memoize, resolve icon server-side.
- `server/src/main.rs:100-125`: `compression` layer placed before `archive/upload/nest_service` comment says intentional, but `/files/*` also uncompressed. Verify `NotForContentType("video/")` prefix-matches, else dead code.
- `app/src/utils.rs:38`: `timestamp_opt().unwrap()` panics on out-of-range mtime. Fallback to epoch. `format_bytes:100-118` float `log2/powi` → integer loop.

### 3. Features / UI-UX gaps

- No delete/rename/move, no search/filter/sort toggle, no pagination, no hidden-file toggle, no empty-state CTA (`app/src/lib.rs:77-86` renders nothing), no file-size/date columns on mobile.
- Upload (`app/src/components/upload.rs:126,153-207`): stale `PathBuf` prop (navigating changes folder, upload goes to old), `listing` in `app/src/lib.rs:54-57` never invalidated on upload, ID = hash(filenames) collides, `.expect()` panics with no error UI, silent abort on concurrent upload, `create(true).truncate(true):94-99` overwrites silently, no drag-drop/cancel/progress-persist/input-reset.
- `upload/progress.rs:40-56`: `broadcast().expect()` panics if receiver left; fast-upload race (`finish` before subscribe → stuck at 0%); no TTL → leak. `use_upload_progress.rs:13-17` splits SSE on `\n` assuming chunk alignment → mis-parse.
- `new_folder.rs:20,42,45`: string `onclick="showModal()"` breaks under CSP, optimistic close with no error display, no validation (`required/pattern/maxlength`), `unwrap/expect` on refs.
- `folder_download.rs:13-51`: stale `base_path`, `navigator.clipboard` string onclick fails on `http://LAN-IP` (needs secure context) with no fallback, `curl '...'` unescaped injection, hover-only dropdown inaccessible on touch/keyboard.
- `breadcrumbs.rs:9-39`, `file_entries.rs:31-83`: `<a>` vs `<A>` inconsistency (full reload vs SPA), no `<nav aria-label>`, no `aria-current`, no `overflow-x-auto` (deep paths clip mobile), no `title`/ellipsis, folder/file links hijacked by router (`<A>` for download), no `role=progressbar`, `Loading...` bare `<p>` no spinner/`aria-live`.
- `app/src/lib.rs:61,116-125`: `expect_context<AppConfig>` no CSR provider (panics on client nav), `/` → `/index` but route is `/index/*path`, 404 renders with `200`.
- `server/src/main.rs:199,205-232`: QR `break` on first error hides rest; QR/display includes docker/link-local IPs. Filter to usable LAN, dedupe.
- Missing flags: `--max-upload-size`, `--tls`, `--auth`, `--read-only`, `--hidden`; `server/src/config.rs:30,34` clap `bool` uses `default_value="false"` not `ArgAction::SetTrue`; `--port 3000` busy silently picks random port (TOCTOU) — only auto-pick on `--port 0`.

### 4. Idiomatic style / code health

- `server/src/main.rs:79-87` + `app/src/state.rs:6-10`: `AppConfig/LeptosOptions/PathBuf` cloned per request. Wrap in `Arc` (`AppState { config: Arc<AppConfig> }`).
- `server/src/fileserv.rs:96-119`: `Query<HashMap<String,String>>` + `#[allow(clippy::implicit_hasher)]` + `map_or_default(String::as_str)`. Use `Query<ArchiveQuery{method:Option<Method>}>` with `FromStr` for `Method` (replacing `TryFrom<&str,Error=()>` in `app/src/archive.rs:44-73`).
- `thiserror` in `fileserv/archive.rs:33-50`: `Io(String, io::Error)` without `#[source]`, breaks chain. Use `#[from]`/`#[source]`.
- `server/src/main.rs:62,167-181`: `get_configuration(None).unwrap()`, `try_join_all(spawn(...))` no graceful shutdown. Use `axum_server::Handle` + `tokio::signal`, `JoinSet`.
- Frontend: `collect_view()` unkeyed → `<For key=|e| name>`, `overflow-x-hidden` without ellipsis, invalid Tailwind `grow-2/grow-3` (`form.rs:17,31`), `Signal<u64>/Signal<Instant>` for constants (`progress_bar.rs:16-19`), double `percent()` call → `Memo`, `errors[0]` panic (`error_template.rs:40`), hardcoded English strings (no i18n), `console_log Debug` in release (`frontend/src/lib.rs:13`).
