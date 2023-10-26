use std::fmt::{Debug, Display, Formatter};

/// Generic error
#[derive(Debug)]
pub enum Error {
  // External
  //
  /// See [std::fmt::Error].
  Fmt(std::fmt::Error),
  /// See [std::io::Error].
  Io(std::io::Error),
  /// See [std::time::SystemTimeError];
  SystemTimeError(std::time::SystemTimeError),
  /// See [wtx::Error].
  Wtx(wtx::Error),

  // Internal
  //
  /// Received a number of frames larger than the maximum amount.
  LargeAmountOfFrames,
  /// Received an unexpected WebSocket code.
  UnexpectedFrameOpCode,
  /// Unknown file type string.
  UnknownFileTyStr,
  /// It wasn't possible to locate the home directory.
  UnknownHomeDirectory,
}

impl Display for Error {
  #[inline]
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    <Self as Debug>::fmt(self, f)
  }
}

impl From<std::fmt::Error> for Error {
  #[inline]
  fn from(from: std::fmt::Error) -> Self {
    Error::Fmt(from)
  }
}

impl From<std::io::Error> for Error {
  #[inline]
  fn from(from: std::io::Error) -> Self {
    Error::Io(from)
  }
}

impl From<std::time::SystemTimeError> for Error {
  #[inline]
  fn from(from: std::time::SystemTimeError) -> Self {
    Error::SystemTimeError(from)
  }
}

impl From<wtx::Error> for Error {
  #[inline]
  fn from(from: wtx::Error) -> Self {
    Error::Wtx(from)
  }
}

impl std::error::Error for Error {}
