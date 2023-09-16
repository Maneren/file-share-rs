#[derive(Debug)]
pub enum UploadError {
  IO(std::io::Error),
  Http(reqwest::Error),
}

impl From<std::io::Error> for UploadError {
  fn from(error: std::io::Error) -> Self {
    UploadError::IO(error)
  }
}

impl From<reqwest::Error> for UploadError {
  fn from(error: reqwest::Error) -> Self {
    UploadError::Http(error)
  }
}

pub struct ReadProgress<R, F> {
  pub inner: R,
  pub callback: F,
}

impl<R: std::io::Read, F> std::io::Read for ReadProgress<R, F>
where
  F: FnMut(usize),
{
  fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
    self.inner.read(buf).map(|n| {
      (self.callback)(n);
      n
    })
  }
}
