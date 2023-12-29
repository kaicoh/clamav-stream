use bytes::Bytes;
use clamav_stream::{BoxError, ScannedStream};
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
    let err_msg = format!("Could not read test file {}", CLEAN_FILE_PATH);
    let file = File::open(CLEAN_FILE_PATH).await.expect(&err_msg);
    let mut input = ReaderStream::new(file).map(boxed);

    let err_msg = format!("Could not connect tcp address {}", HOST_ADDRESS);
    let stream = ScannedStream::<_, TcpStream>::tcp(&mut input, HOST_ADDRESS).expect(&err_msg);

    let result = consume(stream).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), CLEAN_FILE_CONTENTS);
}

#[tokio::test]
async fn scan_infected_file() {
    let err_msg = format!("Could not read test file {}", EICAR_FILE_PATH);
    let file = File::open(EICAR_FILE_PATH).await.expect(&err_msg);
    let mut input = ReaderStream::new(file).map(boxed);

    let err_msg = format!("Could not connect tcp address {}", HOST_ADDRESS);
    let stream = ScannedStream::<_, TcpStream>::tcp(&mut input, HOST_ADDRESS).expect(&err_msg);

    let result = consume(stream).await;
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().to_string(),
        EICAR_FILE_SIGNATURE_FOUND_RESPONSE
    );
}

fn boxed(result: Result<Bytes, std::io::Error>) -> Result<Bytes, BoxError> {
    result.map_err(|err| err.into())
}

async fn consume<S>(mut stream: S) -> Result<String, BoxError>
where
    S: Stream<Item = Result<Bytes, BoxError>> + Unpin,
{
    let mut bytes: Vec<u8> = vec![];

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        bytes.append(&mut chunk.into());
    }

    let res = String::from_utf8(bytes)?;
    Ok(res)
}
