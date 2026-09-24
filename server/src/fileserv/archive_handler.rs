//! `GET /archive` handlers: stream a directory as tar/zip.
//!
//! Archive *bytes* are produced by [`super::archive_io::create_archive`].

use std::{
    fmt::Write as _,
    path::{Path as StdPath, PathBuf},
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use async_walkdir::WalkDir;
use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{HeaderValue, Response, StatusCode, header},
    response::IntoResponse,
};
pub use file_share_app::archive::Method;
use file_share_app::fs_guard::resolve_contained_path;
use leptos::logging;
use serde::Deserialize;
use tokio::{fs, io, io::AsyncWriteExt as _, spawn, task::JoinHandle, time};
use tokio_stream::{Stream, StreamExt as _};
use tokio_util::io::ReaderStream;

use super::{archive_io, responses::PATH_NOT_FOUND};
use crate::{security::SecurityConfig, state::AppState};

/// Size of the in-memory pipe between archive creation and the HTTP body.
/// Large enough to keep a 1 Gbps link fed while the compressor runs ahead.
const DUPLEX_BUF_SIZE: usize = 1024 * 1024;

/// Chunk size used when polling the pipe into the HTTP body stream.
const READER_STREAM_CAPACITY: usize = 64 * 1024;

#[derive(Debug, Deserialize)]
pub struct ArchiveQuery {
    method: Option<Method>,
}

pub async fn handle_archive_with_path<'a>(
    State(app_state): State<AppState>,
    Path(path): Path<String>,
    Query(params): Query<ArchiveQuery>,
) -> impl IntoResponse + use<'a> {
    let target_dir = &app_state.app_config.target_dir;
    logging::log!("Handling archive with path '{path:?}' and params '{params:?}'");

    // Axum already percent-decodes `Path` params exactly once; decoding
    // again here would turn `%252e` into `.` and reopen traversal.
    let Some(path) = resolve_contained_path(target_dir, StdPath::new(&path)).await else {
        return PATH_NOT_FOUND.into_response();
    };

    if let Err(response) = check_archive_dir(&path).await {
        return *response;
    }

    if let Err(response) = check_archive_limits(&path, &app_state.security).await {
        return *response;
    }

    handle_archive(
        path,
        params.method.unwrap_or_default(),
        ArchiveLimits::new(&app_state.security),
    )
    .into_response()
}

pub async fn handle_archive_without_path(
    State(app_state): State<AppState>,
    Query(params): Query<ArchiveQuery>,
) -> impl IntoResponse + use<> {
    logging::log!("Handling archive without path and with params '{params:?}'");
    let path = app_state.app_config.target_dir.clone();
    if let Err(response) = check_archive_dir(&path).await {
        return *response;
    }
    if let Err(response) = check_archive_limits(&path, &app_state.security).await {
        return *response;
    }
    handle_archive(
        path,
        params.method.unwrap_or_default(),
        ArchiveLimits::new(&app_state.security),
    )
    .into_response()
}

/// Reject missing paths and non-directories before archive headers are sent.
/// Otherwise a bad target would produce a `200 OK` with a truncated body.
async fn check_archive_dir(path: &StdPath) -> Result<(), Box<Response<Body>>> {
    match fs::metadata(path).await {
        Ok(metadata) if metadata.is_dir() => Ok(()),
        Ok(_) => Err(Box::new(
            (
                StatusCode::BAD_REQUEST,
                "Archives can only be created from directories",
            )
                .into_response(),
        )),
        Err(_) => Err(Box::new(PATH_NOT_FOUND.into_response())),
    }
}

/// Opt-in (`--max-archive-size`, `--archive-timeout`) bounds for one archive.
#[derive(Clone, Copy)]
struct ArchiveLimits {
    max_size: Option<u64>,
    timeout: Option<Duration>,
}

impl ArchiveLimits {
    fn new(security: &SecurityConfig) -> Self {
        Self {
            max_size: security.max_archive_size,
            timeout: security.archive_timeout,
        }
    }
}

fn handle_archive(
    path: PathBuf,
    archive_method: Method,
    limits: ArchiveLimits,
) -> impl IntoResponse + use<> {
    let Some(name) = path.file_name() else {
        return (
            StatusCode::BAD_REQUEST,
            format!("Invalid path (missing folder name): '{}'", path.display()),
        )
            .into_response();
    };
    let file_name = format!("{}.{}", name.display(), archive_method);

    logging::log!("Creating: {file_name}");

    let Some(disposition) = content_disposition(&file_name) else {
        logging::error!("Failed to build Content-Disposition for {file_name:?}");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to prepare download",
        )
            .into_response();
    };

    let (writer, reader) = io::duplex(DUPLEX_BUF_SIZE);
    let stream = ReaderStream::with_capacity(reader, READER_STREAM_CAPACITY);

    // Abort the compressor when the client goes away: dropping the response
    // body drops the stream, which aborts the task instead of letting it
    // compress a tree nobody reads anymore.
    let task = spawn(async move {
        let mut out = CountingWriter::new(writer, limits.max_size);
        let create = archive_io::create_archive(archive_method, path, &mut out);
        let result = match limits.timeout {
            Some(duration) => match time::timeout(duration, create).await {
                Ok(inner) => inner,
                Err(_) => Err(archive_io::Error::Other(format!(
                    "Archive creation timed out after {}s",
                    duration.as_secs()
                ))),
            },
            None => create.await,
        };
        if let Err(err) = result {
            logging::error!("Error during archive creation: {err:?}");
            if let Err(err) = out.shutdown().await {
                logging::error!("Failed to shut down archive stream: {err}");
            }
        }
    });
    let stream = AbortOnDrop::new(task, stream);

    let headers = [
        (header::CONTENT_DISPOSITION, disposition),
        (
            header::CONTENT_TYPE,
            archive_method
                .mimetype()
                .parse()
                .expect("Static mimetypes are valid"),
        ),
        (header::CACHE_CONTROL, HeaderValue::from_static("no-cache")),
    ];

    (headers, Body::from_stream(stream)).into_response()
}

/// Build a `Content-Disposition` value safe for on-disk file names.
///
/// The quoted `filename` carries an ASCII-only fallback (`"` and other
/// problematic characters replaced), while `filename*` (RFC 5987) carries
/// the exact UTF-8 name. Returns `None` when no valid header can be built
/// instead of panicking on attacker-influenced input.
fn content_disposition(file_name: &str) -> Option<HeaderValue> {
    fn is_attr_char(b: u8) -> bool {
        b.is_ascii_alphanumeric()
            || matches!(
                b,
                b'!' | b'#' | b'$' | b'&' | b'+' | b'-' | b'.' | b'^' | b'_' | b'`' | b'|' | b'~'
            )
    }

    let fallback: String = file_name
        .chars()
        .map(|c| {
            if c.is_ascii_graphic() && !matches!(c, '"' | '\\') {
                c
            } else {
                '_'
            }
        })
        .collect();

    let mut encoded = String::with_capacity(file_name.len());
    for b in file_name.bytes() {
        if is_attr_char(b) {
            encoded.push(b as char);
        } else {
            let _ = write!(encoded, "%{b:02X}");
        }
    }

    format!(r#"attachment; filename="{fallback}"; filename*=UTF-8''{encoded}"#)
        .parse()
        .ok()
}

/// Reject trees that exceed `--max-archive-depth`/`--max-archive-size`
/// before streaming starts, so the client gets a clean error instead of a
/// truncated archive. No-op when neither flag is set.
///
/// The mid-stream [`CountingWriter`] backstop and `--archive-timeout` still
/// bound races where the tree grows between this scan and the stream.
async fn check_archive_limits(
    root: &StdPath,
    security: &SecurityConfig,
) -> Result<(), Box<Response<Body>>> {
    let max_size = security.max_archive_size;
    let max_depth = security.max_archive_depth;
    if max_size.is_none() && max_depth.is_none() {
        return Ok(());
    }

    let mut total: u64 = 0;
    let mut walker = WalkDir::new(root);
    while let Some(entry) = walker.next().await {
        let Ok(entry) = entry else { continue };
        let entry_path = entry.path();
        let Ok(relative) = entry_path.strip_prefix(root) else {
            continue;
        };

        if max_depth.is_some_and(|max| relative.components().count() > max) {
            return Err(Box::new(
                (
                    StatusCode::BAD_REQUEST,
                    "Directory tree is too deep to archive",
                )
                    .into_response(),
            ));
        }

        if let Some(max) = max_size {
            let Ok(file_type) = entry.file_type().await else {
                continue;
            };
            // Mirror what lands in the archive: entries the archivers skip
            // (symlinks, dirs, specials) cost ~nothing.
            if file_type.is_file()
                && let Ok(metadata) = entry.metadata().await
            {
                total = total.saturating_add(metadata.len());
                if total > max {
                    return Err(Box::new(
                        (
                            StatusCode::PAYLOAD_TOO_LARGE,
                            "Directory is too large to archive",
                        )
                            .into_response(),
                    ));
                }
            }
        }
    }
    Ok(())
}

/// [`AsyncWrite`](io::AsyncWrite) wrapper that fails once `max` total bytes
/// were written — backstop for `--max-archive-size` when the tree grows
/// between the pre-scan and the stream.
struct CountingWriter<W> {
    inner: W,
    written: u64,
    max: Option<u64>,
}

impl<W> CountingWriter<W> {
    fn new(inner: W, max: Option<u64>) -> Self {
        Self {
            inner,
            written: 0,
            max,
        }
    }
}

impl<W: io::AsyncWrite + Unpin> io::AsyncWrite for CountingWriter<W> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        let this = self.get_mut();
        if let Some(max) = this.max {
            let incoming = u64::try_from(buf.len()).unwrap_or(u64::MAX);
            if this.written.saturating_add(incoming) > max {
                return Poll::Ready(Err(std::io::Error::other("archive size limit exceeded")));
            }
        }
        let result = Pin::new(&mut this.inner).poll_write(cx, buf);
        if let Poll::Ready(Ok(len)) = &result {
            this.written += u64::try_from(*len).unwrap_or(u64::MAX);
        }
        result
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.get_mut().inner).poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.get_mut().inner).poll_shutdown(cx)
    }
}

/// Stream wrapper that aborts the archive task when the response body is
/// dropped (client disconnect), instead of compressing unread data.
struct AbortOnDrop<S> {
    handle: Option<JoinHandle<()>>,
    stream: S,
}

impl<S> AbortOnDrop<S> {
    fn new(handle: JoinHandle<()>, stream: S) -> Self {
        Self {
            handle: Some(handle),
            stream,
        }
    }
}

impl<S: Stream + Unpin> Stream for AbortOnDrop<S> {
    type Item = S::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.get_mut().stream).poll_next(cx)
    }
}

impl<S> Drop for AbortOnDrop<S> {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            handle.abort();
        }
    }
}
