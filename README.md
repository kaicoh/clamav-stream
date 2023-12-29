# clamav-stream

[![Version](https://img.shields.io/crates/v/clamav-stream)](https://crates.io/crates/clamav-stream)
[![License](https://img.shields.io/crates/l/clamav-stream)](LICENSE)
[![Test](https://img.shields.io/github/actions/workflow/status/kaicoh/clamav-stream/test.yml)](https://github.com/kaicoh/clamav-stream/actions/workflows/test.yml)

A `ScannedStream` is a wrapper stream holding byte stream. It sends the inner stream to [clamav](https://www.clamav.net/) to scan it while passes it through to the consumer.

This library is inspired by the [toblux/rust-clamav-client](https://github.com/toblux/rust-clamav-client).

## Getting Started

Add dependency to your `Cargo.toml`.

```toml
[dependencies]
clamav_stream = "0.1.0"
```

Wrap byte stream with ScannedStream and consume it.

### When the byte stream is clean

There are no deferences between consuming `ScannedStream` and its inner stream.

```rust,no_run
use clamav_stream::ScannedStream;

use bytes::Bytes;
use std::net::TcpStream;
use tokio::fs::File;
use tokio_stream::StreamExt;
use tokio_util::io::ReaderStream;

#[tokio::main]
async fn main() {
    let file = File::open("tests/clean.txt").await.unwrap();
    let mut input = ReaderStream::new(file);

    let addr = "localhost:3310"; // tcp address to clamav server.
    let mut stream = ScannedStream::<_, TcpStream>::tcp(&mut input, addr).unwrap();

    // The result of consuming ScannedStream is equal to consuming the input stream.
    assert_eq!(stream.next().await, Some(Ok(Bytes::from("file contents 1st"))));
    assert_eq!(stream.next().await, Some(Ok(Bytes::from("file contents 2nd"))));
    // ... continue until all contents are consumed ...
    assert_eq!(stream.next().await, Some(Ok(Bytes::from("file contents last"))));
    assert_eq!(stream.next().await, None);
}
```

### When the byte stream is infected

An Err is returned after all contents are consumed.

```rust,no_run
use clamav_stream::{Error, ScannedStream};

use bytes::Bytes;
use std::net::TcpStream;
use tokio::fs::File;
use tokio_stream::StreamExt;
use tokio_util::io::ReaderStream;

#[tokio::main]
async fn main() {
    let file = File::open("tests/eicar.txt").await.unwrap();
    let mut input = ReaderStream::new(file);

    let addr = "localhost:3310"; // tcp address to clamav server.
    let mut stream = ScannedStream::<_, TcpStream>::tcp(&mut input, addr).unwrap();

    // An Err is returned after all contents are consumed.
    assert_eq!(stream.next().await, Some(Ok(Bytes::from("file contents 1st"))));
    assert_eq!(stream.next().await, Some(Ok(Bytes::from("file contents 2nd"))));
    // ... continue until all contents are consumed ...
    assert_eq!(stream.next().await, Some(Ok(Bytes::from("file contents last"))));
    assert_eq!(stream.next().await, Some(Err(Error::Scan("message from clamav".into()))));
    assert_eq!(stream.next().await, None);
}
```

## License

This software is released under the [MIT License](LICENSE).
