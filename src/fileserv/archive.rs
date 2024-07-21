use std::{fmt, path::Path};

use async_compression::tokio::write::{GzipEncoder, ZstdEncoder};
use async_walkdir::WalkDir;
use async_zip::{
  tokio::write::ZipFileWriter, Compression, StringEncoding, ZipEntryBuilder, ZipString,
};
use thiserror::Error as ThisError;
use tokio::{
  fs,
  io::{self, AsyncWrite, AsyncWriteExt},
};
use tokio_stream::StreamExt as _;
use tokio_tar::Builder;
use tokio_util::compat::FuturesAsyncWriteCompatExt;

#[derive(Debug, ThisError)]
pub enum Error {
  /// Any kind of IO errors
  #[error("{0}\ncaused by: {1}")]
  Io(String, std::io::Error),

  /// Any error related to an invalid path (failed to retrieve entry name, unexpected entry type, etc)
  #[error("Invalid path\ncaused by: {0}")]
  InvalidPath(String),

  /// Any other kind of error
  #[error("Other error\ncaused by: {0}")]
  Other(String),

  /// Might occur when the creation of an archive fails
  #[error("An error occurred while creating the {0}\ncaused by: {1}")]
  ArchiveCreation(String, Box<Error>),
}

#[derive(Debug, Clone, Copy)]
pub enum ArchiveMethod {
  Tar,
  TarGz,
  TarZstd,
  Zip,
}

impl ArchiveMethod {
  #[must_use]
  pub fn mimetype(&self) -> &'static str {
    match self {
      ArchiveMethod::Tar => "application/x-tar",
      ArchiveMethod::TarGz => "application/gzip",
      ArchiveMethod::TarZstd => "application/zstd",
      ArchiveMethod::Zip => "application/zip",
    }
  }

  pub async fn create_archive<P, W>(self, dir: P, out: W) -> Result<(), Error>
  where
    P: AsRef<Path>,
    W: AsyncWrite + Unpin + Send + Sync,
  {
    let dir = dir.as_ref();
    match self {
      ArchiveMethod::Tar => tar_dir(dir, out).await,
      ArchiveMethod::TarGz => tar_gz(dir, out).await,
      ArchiveMethod::TarZstd => tar_zstd(dir, out).await,
      ArchiveMethod::Zip => zip_dir(dir, out).await,
    }
  }
}

impl TryFrom<&str> for ArchiveMethod {
  type Error = ();

  fn try_from(value: &str) -> Result<Self, Self::Error> {
    match value {
      "tar" => Ok(ArchiveMethod::Tar),
      "tar.gz" => Ok(ArchiveMethod::TarGz),
      "tar.zst" => Ok(ArchiveMethod::TarZstd),
      "zip" => Ok(ArchiveMethod::Zip),
      _ => Err(()),
    }
  }
}

impl fmt::Display for ArchiveMethod {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let extension = match self {
      ArchiveMethod::Tar => "tar",
      ArchiveMethod::TarGz => "tar.gz",
      ArchiveMethod::TarZstd => "tar.zst",
      ArchiveMethod::Zip => "zip",
    };
    write!(f, "{extension}")
  }
}

/// Write a gzipped tarball of `dir` in `out`.
async fn tar_gz<W>(dir: &Path, out: W) -> Result<(), Error>
where
  W: AsyncWrite + Unpin + Send + Sync,
{
  let mut encoder = GzipEncoder::new(out);

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
  let mut encoder = ZstdEncoder::new(out);

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
  let mut zip = ZipFileWriter::with_tokio(out);

  zip.comment(format!(
    "This archive was created by the file-share-rs server at {}",
    chrono::Local::now().to_rfc2822()
  ));

  let mut walker = WalkDir::new(dir);

  while let Some(entry) = walker.next().await {
    let Ok(entry) = entry else {
      continue;
    };

    if entry.file_type().await.is_ok_and(|t| !t.is_file()) {
      continue;
    }

    add_file_to_zip(&entry.path(), dir, &mut zip).await?;
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
  zip: &mut ZipFileWriter<W>,
) -> Result<(), Error>
where
  W: AsyncWrite + Unpin,
{
  let name = path.strip_prefix(base_dir).map_err(|_| {
    Error::InvalidPath(format!(
      "Failed to strip {} from {}",
      base_dir.display(),
      path.display()
    ))
  })?;

  let zip_name = ZipString::new(
    name.as_os_str().as_encoded_bytes().to_vec(),
    StringEncoding::Raw,
  );

  let mut file = fs::File::open(path)
    .await
    .map_err(|e| Error::Io(format!("Failed to open {} for reading", path.display()), e))?;

  let entry = ZipEntryBuilder::new(zip_name, Compression::Deflate).build();

  let mut sink = zip
    .write_entry_stream(entry)
    .await
    .map_err(|e| {
      Error::ArchiveCreation(
        format!("Failed to write {} to the ZIP archive", name.display()),
        Error::Other(e.to_string()).into(),
      )
    })?
    .compat_write();

  io::copy(&mut file, &mut sink).await.map_err(|e| {
    Error::Io(
      format!("Failed to write {} to the ZIP archive", name.display()),
      e,
    )
  })?;

  sink.shutdown().await.map_err(|e| {
    Error::ArchiveCreation(
      format!("Failed to write {} to the ZIP archive", name.display()),
      Error::Other(e.to_string()).into(),
    )
  })?;

  Ok(())
}
