use bytes::Bytes;
use clamav_stream::{Error, ScannedStream};
use std::net::TcpStream;
use tokio::fs::File;
use tokio_stream::{Stream, StreamExt};
use tokio_util::io::ReaderStream;

const HOST_ADDRESS: &str = "localhost:3310";

const EICAR_FILE_PATH: &str = "tests/eicar.txt";
const CLEAN_FILE_PATH: &str = "tests/clean.txt";

const EICAR_FILE_SIGNATURE_FOUND_RESPONSE: &str = "stream: Eicar-Signature FOUND\0";
const CLEAN_FILE_CONTENTS: &str = "Hello World!\n";

#[tokio::test]
async fn scan_clean_file() {
    let mut input = read_file(CLEAN_FILE_PATH).await;
    let stream = scanned_stream(&mut input);

    let result = consume(stream).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), CLEAN_FILE_CONTENTS);
}

#[tokio::test]
async fn scan_infected_file() {
    let mut input = read_file(EICAR_FILE_PATH).await;
    let stream = scanned_stream(&mut input);

    let result = consume(stream).await;
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().to_string(),
        EICAR_FILE_SIGNATURE_FOUND_RESPONSE
    );
}

async fn read_file(path: &str) -> ReaderStream<File> {
    let err_msg = format!("Could not read test file {}", EICAR_FILE_PATH);
    let file = File::open(path).await.expect(&err_msg);
    ReaderStream::new(file)
}

fn scanned_stream(
    input: &mut ReaderStream<File>,
) -> ScannedStream<'_, ReaderStream<File>, TcpStream> {
    let err_msg = format!("Could not connect tcp address {}", HOST_ADDRESS);
    ScannedStream::<_, TcpStream>::tcp(input, HOST_ADDRESS).expect(&err_msg)
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
