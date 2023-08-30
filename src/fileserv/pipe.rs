use std::io::{self, Error, ErrorKind};

use axum::body::Bytes;
use futures::{channel::mpsc::Sender, executor::block_on, SinkExt};

pub struct Pipe {
  target: Sender<io::Result<Bytes>>,
}

impl Pipe {
  pub fn new(target: Sender<io::Result<Bytes>>) -> Self {
    Pipe { target }
  }
}

impl Drop for Pipe {
  fn drop(&mut self) {
    self.target.disconnect();
  }
}

impl io::Write for Pipe {
  fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
    block_on(self.target.send(Ok(Bytes::copy_from_slice(buf))))
      .map_err(|e| Error::new(io::ErrorKind::Other, e))?;

    Ok(buf.len())
  }

  fn flush(&mut self) -> io::Result<()> {
    block_on(self.target.flush()).map_err(|e| Error::new(ErrorKind::UnexpectedEof, e))
  }
}
