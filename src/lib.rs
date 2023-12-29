//! A [`ScannedStream`] sends the inner stream to [clamav](https://www.clamav.net/) to scan its
//! contents while passes it through to the stream consumer.
//!
//! If a virus is detected by the clamav, it returns Err as stream chunk otherwise it just passes the
//! inner stream through to the consumer.
//!
//! ## When the byte stream is clean
//!
//! There are no deferences between consuming [`ScannedStream`] and its inner stream.
//! ```rust,no_run
//! use clamav_stream::ScannedStream;
//!
//! use bytes::Bytes;
//! use std::net::TcpStream;
//! use tokio::fs::File;
//! use tokio_stream::StreamExt;
//! use tokio_util::io::ReaderStream;
//!
//! #[tokio::main]
//! async fn main() {
//!     let file = File::open("tests/clean.txt").await.unwrap();
//!     let mut input = ReaderStream::new(file);
//!
//!     let addr = "localhost:3310"; // tcp address to clamav server.
//!     let mut stream = ScannedStream::<_, TcpStream>::tcp(&mut input, addr).unwrap();
//!
//!     // The result of consuming ScannedStream is equal to consuming the input stream.
//!     assert_eq!(stream.next().await, Some(Ok(Bytes::from("file contents 1st"))));
//!     assert_eq!(stream.next().await, Some(Ok(Bytes::from("file contents 2nd"))));
//!     // ... continue until all contents are consumed ...
//!     assert_eq!(stream.next().await, Some(Ok(Bytes::from("file contents last"))));
//!     assert_eq!(stream.next().await, None);
//! }
//! ```
//!
//! ## When the byte stream is infected
//!
//! An Err is returned after all contents are consumed.
//! ```rust,no_run
//! use clamav_stream::{Error, ScannedStream};
//!
//! use bytes::Bytes;
//! use std::net::TcpStream;
//! use tokio::fs::File;
//! use tokio_stream::StreamExt;
//! use tokio_util::io::ReaderStream;
//!
//! #[tokio::main]
//! async fn main() {
//!     let file = File::open("tests/eicar.txt").await.unwrap();
//!     let mut input = ReaderStream::new(file);
//!
//!     let addr = "localhost:3310"; // tcp address to clamav server.
//!     let mut stream = ScannedStream::<_, TcpStream>::tcp(&mut input, addr).unwrap();
//!
//!     // An Err is returned after all contents are consumed.
//!     assert_eq!(stream.next().await, Some(Ok(Bytes::from("file contents 1st"))));
//!     assert_eq!(stream.next().await, Some(Ok(Bytes::from("file contents 2nd"))));
//!     // ... continue until all contents are consumed ...
//!     assert_eq!(stream.next().await, Some(Ok(Bytes::from("file contents last"))));
//!     assert_eq!(stream.next().await, Some(Err(Error::Scan("message from clamav".into()))));
//!     assert_eq!(stream.next().await, None);
//! }
//! ```

mod error;
pub use error::Error;

use pin_project::pin_project;
use std::{
    error::Error as StdError,
    io::{Read, Write},
    net::{TcpStream, ToSocketAddrs},
    path::Path,
    pin::{pin, Pin},
    task::{Context, Poll},
};
use tokio_stream::Stream;

#[cfg(unix)]
use std::os::unix::net::UnixStream;

const START: &[u8; 10] = b"zINSTREAM\0";
const FINISH: &[u8; 4] = &[0, 0, 0, 0];
const CHUNK_SIZE: usize = 4096;

/// A wrapper stream holding byte stream. This sends the inner stream to [clamav](https://www.clamav.net/) to scan it while passes it through to the consumer.
#[pin_project]
pub struct ScannedStream<'a, St: ?Sized, RW: Read + Write> {
    #[pin]
    input: &'a mut St,
    inner: RW,
    started: bool,
    finished: bool,
}

macro_rules! write_clamav {
    ($stream:expr, $bytes:expr) => {
        if let Err(err) = write_stream($stream, $bytes) {
            return Poll::Ready(Some(Err(err)));
        }
    };
}

macro_rules! read_clamav {
    ($stream:expr) => {
        if let Err(err) = read_stream_response($stream) {
            return Poll::Ready(Some(Err(err)));
        }
    };
}

impl<'a, St, RW, E> Stream for ScannedStream<'a, St, RW>
where
    St: Stream<Item = Result<bytes::Bytes, E>> + Unpin + ?Sized,
    RW: Read + Write,
    E: StdError + Send + Sync + 'static,
{
    type Item = Result<bytes::Bytes, Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let me = self.project();
        match me.input.poll_next(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Some(Ok(bytes))) => {
                if !*me.started {
                    *me.started = true;
                    write_clamav!(me.inner, START);
                }

                for chunk in bytes.as_ref().chunks(CHUNK_SIZE) {
                    let len = chunk.len() as u32;
                    write_clamav!(me.inner, &len.to_be_bytes());
                    write_clamav!(me.inner, chunk);
                }

                Poll::Ready(Some(Ok(bytes)))
            }
            Poll::Ready(Some(Err(err))) => Poll::Ready(Some(Err(Error::Stream(Box::new(err))))),
            Poll::Ready(None) => {
                if *me.finished {
                    return Poll::Ready(None);
                }

                *me.finished = true;
                write_clamav!(me.inner, FINISH);
                read_clamav!(me.inner);

                Poll::Ready(None)
            }
        }
    }
}

impl<'a, St, RW, E> ScannedStream<'a, St, RW>
where
    St: Stream<Item = Result<bytes::Bytes, E>> + Unpin + ?Sized,
    RW: Read + Write,
    E: StdError,
{
    /// Create a new [`ScannedStream`]
    pub fn new(input: &'a mut St, inner: RW) -> Self {
        Self {
            input,
            inner,
            started: false,
            finished: false,
        }
    }

    /// Create a new [`ScannedStream`] connecting to clamav server with tcp socket.
    pub fn tcp(
        input: &'a mut St,
        addr: impl ToSocketAddrs,
    ) -> Result<ScannedStream<'a, St, TcpStream>, Error> {
        let inner = TcpStream::connect(addr)?;
        Ok(ScannedStream::new(input, inner))
    }

    /// Create a new [`ScannedStream`] connecting to clamav server with unix socket.
    #[cfg(unix)]
    pub fn socket(
        input: &'a mut St,
        path: impl AsRef<Path>,
    ) -> Result<ScannedStream<'a, St, UnixStream>, Error> {
        let inner = UnixStream::connect(path)?;
        Ok(ScannedStream::new(input, inner))
    }
}

fn write_stream(stream: &mut impl Write, buf: &[u8]) -> Result<(), Error> {
    stream.write_all(buf)?;
    Ok(())
}

fn read_stream_response(stream: &mut impl Read) -> Result<(), Error> {
    let mut body: Vec<u8> = vec![];
    stream.read_to_end(&mut body)?;

    let res = std::str::from_utf8(&body)?;

    if res.contains("OK") && !res.contains("FOUND") {
        Ok(())
    } else {
        Err(Error::Scan(res.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use std::io::{self, Cursor};
    use tokio_stream::StreamExt;

    #[tokio::test]
    async fn it_returns_original_inputs_when_success() {
        let mut input = tokio_stream::iter(stream_from_str("Hello World"));
        let mut inner = MockStream::new("OK");

        let stream = ScannedStream::new(&mut input, &mut inner);
        let result = consume(stream).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Hello World");

        assert_eq!(inner.written.len(), 4);
        assert_eq!(inner.written.get(0).unwrap(), "zINSTREAM\0");
        assert_eq!(
            inner.written.get(1).unwrap(),
            &String::from_utf8(("Hello World".len() as u32).to_be_bytes().to_vec()).unwrap(),
        );
        assert_eq!(inner.written.get(2).unwrap(), "Hello World");
        assert_eq!(
            inner.written.get(3).unwrap(),
            &String::from_utf8(vec![0, 0, 0, 0]).unwrap(),
        );
    }

    #[tokio::test]
    async fn it_returns_an_error_when_found_any_virus() {
        let mut input = tokio_stream::iter(stream_from_str("Hello World"));
        let mut inner = MockStream::new("FOUND test virus");

        let stream = ScannedStream::new(&mut input, &mut inner);
        let result = consume(stream).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "FOUND test virus");
    }

    struct MockStream {
        written: Vec<String>,
        output: Cursor<Vec<u8>>,
    }

    impl MockStream {
        fn new(value: &str) -> Self {
            Self {
                written: vec![],
                output: Cursor::new(value.as_bytes().to_vec()),
            }
        }
    }

    impl Read for MockStream {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            self.output.read(buf)
        }
    }

    impl Write for MockStream {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.written.push(String::from_utf8(buf.to_vec()).unwrap());
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    fn stream_from_str(value: &'static str) -> impl Iterator<Item = Result<Bytes, Error>> {
        [Ok(Bytes::from(value))].into_iter()
    }

    async fn consume<S>(mut stream: S) -> Result<String, Error>
    where
        S: Stream<Item = Result<Bytes, Error>> + Unpin,
    {
        let mut bytes: Vec<u8> = vec![];

        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            bytes.append(&mut chunk.into());
        }

        let res = std::str::from_utf8(&bytes)?;
        Ok(res.to_string())
    }
}
