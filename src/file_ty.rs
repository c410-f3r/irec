/// File type
#[derive(Clone, Copy, Debug)]
pub enum FileTy {
  /// Audio
  Audio,
  /// Video
  Video,
}

impl From<FileTy> for &str {
  #[inline]
  fn from(from: FileTy) -> Self {
    match from {
      FileTy::Audio => "audio",
      FileTy::Video => "video",
    }
  }
}

impl TryFrom<&str> for FileTy {
  type Error = crate::Error;

  #[inline]
  fn try_from(from: &str) -> Result<Self, Self::Error> {
    Ok(match from {
      "audio" => Self::Audio,
      "video" => Self::Video,
      _ => return Err(crate::Error::UnknownFileTyStr),
    })
  }
}
