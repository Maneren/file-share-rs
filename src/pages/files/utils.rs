use std::{
  ffi::OsStr,
  // io, mem,
  path::Path,
  // pin::Pin,
  // task::{Context, Poll},
};

// use futures::{executor::block_on, Future, FutureExt};
// use tokio::io::AsyncRead;
// use wasm_bindgen_futures::JsFuture;
// use web_sys::ReadableStreamByobReader;

pub fn os_to_string(str: impl AsRef<OsStr>) -> String {
  str.as_ref().to_string_lossy().to_string()
}

pub fn get_file_extension(path: impl AsRef<OsStr>) -> String {
  Path::new(path.as_ref())
    .extension()
    .map(os_to_string)
    .unwrap_or_default()
}

#[allow(clippy::cast_possible_truncation)]
#[allow(clippy::cast_possible_wrap)]
#[allow(clippy::cast_sign_loss)]
#[allow(clippy::cast_precision_loss)]
pub fn format_bytes(bytes: u64) -> String {
  let prefixes = ["", "K", "M", "G", "T", "P", "E", "Z", "Y"];

  if bytes == 0 {
    return "0 B".into();
  }

  let bytes_f64 = bytes as f64;

  // calculate log1024(bytes) and round down
  let power_of_1024 = (bytes_f64.log2() / 10.0).floor() as i32;

  let number = bytes_f64 / 1024f64.powi(power_of_1024);
  let formatted = format!("{number:0.2}");
  let formatted = formatted.trim_end_matches('0').trim_end_matches('.'); // Remove trailing zeros

  let prefix = prefixes[power_of_1024 as usize];

  format!("{formatted} {prefix}B")
}

// struct Sendable(JsFuture);
// // Safety: WebAssembly will only ever run in a single-threaded context.
// unsafe impl Send for Sendable {}
// // impl Deref for Sendable {
// //   type Target = JsFuture;
// //   fn deref(&self) -> &JsFuture {
// //     &self.0
// //   }
// // }
// impl Future for Sendable {
//   type Output = <JsFuture as Future>::Output;
//   fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
//     self.get_mut().0.poll_unpin(cx)
//   }
// }
//
// pub struct ReadableStreamAdapter {
//   reader: ReadableStreamByobReader,
//   future: Option<JsFuture>,
//   buffer: [u8; 1024],
// }
//
// impl ReadableStreamAdapter {
//   pub fn new(reader: ReadableStreamByobReader) -> Self {
//     Self {
//       reader,
//       future: None,
//       buffer: [0; 1024],
//     }
//   }
// }
//
// unsafe impl Send for ReadableStreamAdapter {}
// unsafe impl Sync for ReadableStreamAdapter {}
//
// impl io::Read for ReadableStreamAdapter {
//   fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
//     let promise = self.reader.read_with_u8_array(buf);
//
//     #[allow(clippy::cast_sign_loss)]
//     #[allow(clippy::cast_possible_truncation)]
//     block_on(JsFuture::from(promise))
//       .map(|n| n.as_f64().unwrap() as usize)
//       .map_err(|e| {
//         io::Error::new(
//           io::ErrorKind::Other,
//           format!("ReadableStreamByobReader::read failed: {:?}", e),
//         )
//       })
//   }
// }
//
// impl AsyncRead for ReadableStreamAdapter {
//   fn poll_read(
//     mut self: Pin<&mut Self>,
//     cx: &mut Context<'_>,
//     read_buf: &mut tokio::io::ReadBuf<'_>,
//   ) -> Poll<io::Result<()>> {
//     match self.future.take() {
//       None => {
//         // let mut buffer = mem::take(&mut self.buffer);
//         let buffer = &mut self.buffer;
//         // let reader = &self.reader;
//         let promise = self.reader.read_with_u8_array(buffer);
//         self.future = Some(JsFuture::from(promise));
//         Poll::Pending
//       }
//       Some(mut future) => future
//         .poll_unpin(cx)
//         .map_ok(|_| {
//           read_buf.put_slice(&self.buffer[..]);
//         })
//         .map_err(|e| {
//           io::Error::new(
//             io::ErrorKind::Other,
//             format!("ReadableStreamByobReader::read failed: {:?}", e),
//           )
//         }),
//     }
//   }
// }

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  pub fn test_format_bytes() {
    assert_eq!(format_bytes(0), "0 B");
    assert_eq!(format_bytes(1), "1 B");
    assert_eq!(format_bytes(1024), "1 KB");
    assert_eq!(format_bytes(1024 * 1024), "1 MB");
    assert_eq!(format_bytes(1024 * 1024 * 1024), "1 GB");
    assert_eq!(format_bytes(1024 * 1024 * 1024 * 1024), "1 TB");

    assert_eq!(format_bytes(5 * 1024 * 1024), "5 MB");

    assert_eq!(format_bytes(1024 + 256), "1.25 KB");
    assert_eq!(format_bytes(1024 + 100), "1.1 KB");
    assert_eq!(format_bytes(1024 + 1000), "1.98 KB");

    assert_eq!(format_bytes(u64::MAX), "16 EB");
  }
}
