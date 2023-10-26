use std::path::PathBuf;

/// Base directory where all generated files are stored.
#[inline]
pub fn irec_dir() -> crate::Result<PathBuf> {
  Ok(home::home_dir().ok_or(crate::Error::UnknownHomeDirectory)?.join(".irec"))
}
