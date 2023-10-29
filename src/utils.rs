use std::path::PathBuf;

use crate::FileTy;

/// Base directory where all generated files are stored.
#[inline]
pub fn irec_dir(ft: FileTy) -> crate::Result<PathBuf> {
  let mut base = home::home_dir().ok_or(crate::Error::UnknownHomeDirectory)?;
  base.push(".irec");
  match ft {
    FileTy::Audio => {
      base.push("audio");
    }
    FileTy::Video => {
      base.push("video");
    }
  }
  Ok(base)
}
