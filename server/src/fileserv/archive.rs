#![allow(clippy::items_after_statements)]
//! Utility functions for creating archive files
//!
//! The `create_archive` function is the main entrypoint for creating the
//! archive. It takes a path and an output stream, and writes the archive to the
//! output stream.
//!
//! NOTE: Uses the fastest compression level to prevent compression from being a
//! bottleneck.

use std::{ffi::OsStr, io, path::Path};

use async_compression::{
    Level,
    tokio::write::{GzipEncoder, ZstdEncoder},
};
use async_walkdir::WalkDir;
use async_zip::{
    Compression, StringEncoding, ZipEntryBuilder, ZipString, tokio::write::ZipFileWriter,
};
use cfg_if::cfg_if;
use chrono::Local;
use file_share_app::archive::Method;
use futures::io::copy;
use thiserror::Error as ThisError;
use tokio::{
    fs,
    io::{AsyncWrite, AsyncWriteExt as _},
};
use tokio_stream::StreamExt as _;
use tokio_tar::Builder;
use tokio_util::compat::TokioAsyncReadCompatExt as _;

#[derive(Debug, ThisError)]
pub enum Error {
    /// Any kind of IO errors
    #[error("{0}")]
    Io(String, #[source] io::Error),

    /// Any error related to an invalid path (failed to retrieve entry name,
    /// unexpected entry type, etc)
    #[error("Invalid path: {0}")]
    InvalidPath(String),

    /// Any other kind of error
    #[error("{0}")]
    Other(String),

    /// Might occur when the creation of an archive fails
    #[error("An error occurred while creating {0}")]
    ArchiveCreation(String, #[source] Box<Error>),
}

/// Create an archive from given dir using the given method.
///
/// Writes an output stream into a passed [`AsyncWrite`] sink.
///
/// # Errors
///
/// This function will return an error if there is any error during the
/// archive creation, usually due to IO or invalid input dir.
pub async fn create_archive<P, W>(method: Method, dir: P, out: W) -> Result<(), Error>
where
    P: AsRef<Path>,
    W: AsyncWrite + Unpin + Send + Sync,
{
    let dir = dir.as_ref();
    match method {
        Method::Tar => tar_dir(dir, out).await,
        Method::TarGz => tar_gz(dir, out).await,
        Method::TarZstd => tar_zstd(dir, out).await,
        Method::Zip => zip_dir(dir, out).await,
    }
}

/// Write a gzipped tarball of `dir` in `out`.
async fn tar_gz<W>(dir: &Path, out: W) -> Result<(), Error>
where
    W: AsyncWrite + Unpin + Send + Sync,
{
    let mut encoder = GzipEncoder::with_quality(out, Level::Fastest);

    tar_dir(dir, &mut encoder).await?;

    encoder.shutdown().await.map_err(|e| {
        Error::ArchiveCreation(
            "gzip".to_string(),
            Box::new(Error::Io(
                "Finishing GZIP compression failed".to_string(),
                e,
            )),
        )
    })?;

    Ok(())
}

/// Write a zstd-compressed tarball of `dir` in `out`.
async fn tar_zstd<W>(dir: &Path, out: W) -> Result<(), Error>
where
    W: AsyncWrite + Unpin + Send + Sync,
{
    let mut encoder = ZstdEncoder::with_quality(out, Level::Fastest);

    tar_dir(dir, &mut encoder).await?;

    encoder
        .shutdown()
        .await
        .map_err(|e| Error::Io("Finishing ZSTD compression failed".to_string(), e))?;

    Ok(())
}

/// Write a tarball of `dir` in `out`.
async fn tar_dir<W>(dir: &Path, out: W) -> Result<(), Error>
where
    W: AsyncWrite + Unpin + Send + Sync,
{
    let folder_name = dir
        .file_name()
        .ok_or_else(|| Error::InvalidPath("Directory name terminates in \"..\"".to_string()))?;

    let mut builder = Builder::new_non_terminated(out);

    builder.follow_symlinks(false);

    builder
        .append_dir_all(folder_name, dir)
        .await
        .map_err(|e| {
            Error::Io(
                format!(
                    "Failed to append the content of {} to the TAR archive",
                    dir.display()
                ),
                e,
            )
        })?;

    builder
        .finish()
        .await
        .map_err(|e| Error::Io("Failed to finish writing the TAR archive".to_string(), e))?;

    Ok(())
}

/// Write a zip archive of `dir` in `out`.
/// The content of `dir` will be saved in the archive as a folder named `dir`.
async fn zip_dir<W>(dir: &Path, out: W) -> Result<(), Error>
where
    W: AsyncWrite + Unpin,
{
    let folder_name = dir
        .file_name()
        .ok_or_else(|| Error::InvalidPath("Directory name terminates in \"..\"".to_string()))?;

    let mut zip = ZipFileWriter::with_tokio(out);

    zip.comment(format!(
        "This archive was created by the file-share-rs server at {}",
        Local::now().to_rfc2822()
    ));

    // NOTE: `WalkDir` never follows symlinks, and the `file_type` check below
    // (which does not resolve links either) additionally skips symlinks,
    // sockets, fifos and devices, so only regular files land in the archive.
    let mut walker = WalkDir::new(dir);

    while let Some(entry) = walker.next().await {
        let Ok(entry) = entry else {
            continue;
        };

        let Ok(file_type) = entry.file_type().await else {
            continue;
        };
        if file_type.is_symlink() || !file_type.is_file() {
            continue;
        }

        add_file_to_zip(&entry.path(), dir, folder_name, &mut zip).await?;
    }

    zip.close().await.map_err(|e| {
        Error::ArchiveCreation(
            "Failed to finish writing the ZIP archive".to_string(),
            Error::Other(e.to_string()).into(),
        )
    })?;

    Ok(())
}

async fn add_file_to_zip<W>(
    path: &Path,
    base_dir: &Path,
    folder_name: &OsStr,
    zip: &mut ZipFileWriter<W>,
) -> Result<(), Error>
where
    W: AsyncWrite + Unpin,
{
    let relative = path.strip_prefix(base_dir).map_err(|_| {
        Error::InvalidPath(format!(
            "Failed to strip {} from {}",
            base_dir.display(),
            path.display()
        ))
    })?;
    let name = Path::new(folder_name).join(relative);

    let zip_name = ZipString::new(
        name.to_string_lossy().as_bytes().to_owned(),
        StringEncoding::Utf8,
    );

    let file = fs::File::open(path)
        .await
        .map_err(|e| Error::Io(format!("Failed to open {} for reading", path.display()), e))?;

    let entry = ZipEntryBuilder::new(zip_name, Compression::Deflate);

    cfg_if! { if #[cfg(target_family = "unix")] {
      use std::os::unix::fs::PermissionsExt as _;
      #[allow(clippy::cast_possible_truncation)]
      let entry = entry.unix_permissions(
        file
          .metadata()
          .await
          .map_err(|e| Error::Io(format!("Failed to get metadata for {}", path.display()), e))?
          .permissions()
          .mode() as u16
      );
    }}

    let mut sink = zip.write_entry_stream(entry).await.map_err(|e| {
        Error::ArchiveCreation(
            format!("Failed to write {} to the ZIP archive", name.display()),
            Error::Other(e.to_string()).into(),
        )
    })?;

    // NOTE: zip is a fallback for very old devices; `async_zip`'s entry sink
    // is futures-based, so the tokio file needs the compat shim here.
    copy(&mut file.compat(), &mut sink).await.map_err(|e| {
        Error::Io(
            format!("Failed to write {} to the ZIP archive", name.display()),
            e,
        )
    })?;

    sink.close().await.map_err(|e| {
        Error::ArchiveCreation(
            format!("Failed to write {} to the ZIP archive", name.display()),
            Error::Other(e.to_string()).into(),
        )
    })?;

    Ok(())
}
