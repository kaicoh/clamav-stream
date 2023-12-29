use std::{error::Error as StdError, io, str::Utf8Error};

/// The error type returned by [`ScannedStream`](crate::ScannedStream).
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Equivalent to the [`std::io::Error`](std::io::Error).
    #[error("io error: {0}")]
    Io(io::Error),

    /// Equivalent to the [`std::str::Utf8Error`](std::str::Utf8Error).
    #[error("utf8 error: {0}")]
    Utf8(Utf8Error),

    /// An error returned while consuming the inner stream.
    #[error("stream error: {0}")]
    Stream(Box<dyn StdError + Send + Sync>),

    /// Infected stream error with message from the clamav.
    #[error("{0}")]
    Scan(String),
}

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<Utf8Error> for Error {
    fn from(error: Utf8Error) -> Self {
        Self::Utf8(error)
    }
}

impl PartialEq for Error {
    fn eq(&self, other: &Self) -> bool {
        format!("{self}").as_str() == format!("{other}").as_str()
    }
}
