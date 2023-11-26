use std::{
  fmt, fs,
  io::{self, Cursor},
  path::Path,
};

use libflate::gzip;
use tar::Builder;
use thiserror::Error as ThisError;
use zip::{write::FileOptions, ZipWriter};

#[derive(Debug, ThisError)]
pub enum Error {
  /// Any kind of IO errors
  #[error("{0}\ncaused by: {1}")]
  Io(String, std::io::Error),

  /// Any error related to an invalid path (failed to retrieve entry name, unexpected entry type, etc)
  #[error("Invalid path\ncaused by: {0}")]
  InvalidPath(String),

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

  pub fn create_archive<P, W>(self, dir: P, out: W) -> Result<(), Error>
  where
    P: AsRef<Path>,
    W: io::Write,
  {
    let dir = dir.as_ref();
    match self {
      ArchiveMethod::Tar => tar_dir(dir, out),
      ArchiveMethod::TarGz => tar_gz(dir, out),
      ArchiveMethod::TarZstd => tar_zstd(dir, out),
      ArchiveMethod::Zip => zip_dir(dir, out),
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
fn tar_gz<W>(dir: &Path, out: W) -> Result<(), Error>
where
  W: std::io::Write,
{
  let mut out = gzip::Encoder::new(out).map_err(|e| Error::Io("GZIP".to_string(), e))?;

  tar_dir(dir, &mut out)?;

  out
    .finish()
    .into_result()
    .map_err(|e| Error::Io("GZIP finish".to_string(), e))?;

  Ok(())
}

/// Write a zstd-compressed tarball of `dir` in `out`.
fn tar_zstd<W>(dir: &Path, out: W) -> Result<(), Error>
where
  W: std::io::Write,
{
  let mut out = zstd::Encoder::new(out, 0).map_err(|e| Error::Io("ZSTD".to_string(), e))?;

  tar_dir(dir, &mut out)?;

  out
    .finish()
    .map_err(|e| Error::Io("ZSTD finish".to_string(), e))?;

  Ok(())
}

/// Write a tarball of `dir` in `out`.
fn tar_dir<W>(dir: &Path, out: W) -> Result<(), Error>
where
  W: std::io::Write,
{
  let inner_folder = dir
    .file_name()
    .ok_or_else(|| Error::InvalidPath("Directory name terminates in \"..\"".to_string()))?;

  let directory = inner_folder.to_str().ok_or_else(|| {
    Error::InvalidPath("Directory name contains invalid UTF-8 characters".to_string())
  })?;

  tar(dir, directory, out).map_err(|e| Error::ArchiveCreation("tarball".to_string(), Box::new(e)))
}

/// Writes a tarball of `dir` in `out`.
///
/// The content of `src_dir` will be saved in the archive as a folder named `inner_folder`.
fn tar<W>(src_dir: &Path, inner_folder: &str, out: W) -> Result<(), Error>
where
  W: std::io::Write,
{
  let mut tar_builder = Builder::new(out);

  tar_builder.follow_symlinks(true);

  // Recursively adds the content of src_dir into the archive stream
  tar_builder
    .append_dir_all(inner_folder, src_dir)
    .map_err(|e| {
      Error::Io(
        format!(
          "Failed to append the content of {} to the TAR archive",
          src_dir.to_string_lossy()
        ),
        e,
      )
    })?;

  // Finish the archive
  tar_builder
    .into_inner()
    .map_err(|e| Error::Io("Failed to finish writing the TAR archive".to_string(), e))?;

  Ok(())
}

/// Write a zip archive of `dir` in `out`.
/// The content of `dir` will be saved in the archive as a folder named `dir`.
fn zip_dir<W>(dir: &Path, mut out: W) -> Result<(), Error>
where
  W: io::Write,
{
  let mut data = Vec::new();
  {
    let mut zip = ZipWriter::new(Cursor::new(&mut data));

    zip.set_comment(format!(
      "This archive was created by the fileserv server at {}",
      chrono::Local::now().to_rfc2822()
    ));

    add_dir_to_zip(dir, dir, &mut zip)?;

    zip.finish().map_err(|e| {
      Error::Io(
        "Failed to finish writing the ZIP archive".to_string(),
        e.into(),
      )
    })?;
  }

  out
    .write_all(&data)
    .map_err(|e| Error::Io("Failed to write the ZIP archive".to_string(), e))?;

  Ok(())
}

fn basename(base_path: &Path, path: &Path) -> Result<String, Error> {
  path
    .strip_prefix(base_path)
    .map_err(|_| {
      Error::InvalidPath(format!(
        "Failed to strip {} from {}",
        base_path.to_string_lossy(),
        path.to_string_lossy()
      ))
    })
    .map(|basename| basename.to_string_lossy().into_owned())
}

fn add_file_to_zip<W>(base_path: &Path, path: &Path, zip: &mut ZipWriter<W>) -> Result<(), Error>
where
  W: io::Write + io::Seek,
{
  let name = basename(base_path, path)?;

  zip
    .start_file(&name, FileOptions::default())
    .map_err(|e| Error::Io(format!("Failed to add {name} to the ZIP archive"), e.into()))?;

  let mut file = fs::File::open(path).map_err(|e| {
    Error::Io(
      format!("Failed to open {} for reading", path.to_string_lossy()),
      e,
    )
  })?;

  io::copy(&mut file, zip)
    .map_err(|e| Error::Io(format!("Failed to write {name} to the ZIP archive"), e))?;

  Ok(())
}

/// Recursively add the content of `src_dir` to the zip archive `zip`.
fn add_dir_to_zip<W>(base_path: &Path, dir: &Path, zip: &mut ZipWriter<W>) -> Result<(), Error>
where
  W: io::Write + io::Seek,
{
  for entry in fs::read_dir(dir).map_err(|e| {
    Error::Io(
      format!("Failed to read the content of {}", dir.to_string_lossy()),
      e,
    )
  })? {
    let entry = entry.map_err(|e| {
      Error::Io(
        format!("Failed to read the content of {}", dir.to_string_lossy()),
        e,
      )
    })?;

    let path = entry.path();

    let name = basename(base_path, &path)?;

    if path.is_dir() {
      zip
        .add_directory(name, FileOptions::default())
        .map_err(|e| {
          Error::Io(
            format!(
              "Failed to add {} to the ZIP archive",
              path.to_string_lossy()
            ),
            e.into(),
          )
        })?;

      add_dir_to_zip(base_path, &path, zip)?;
    } else {
      add_file_to_zip(base_path, &path, zip)?;
    }
  }

  Ok(())
}
