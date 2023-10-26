#![doc = include_str!("../README.md")]

pub mod api;
mod error;
mod file_ty;
mod utils;

pub use error::Error;
pub use file_ty::FileTy;
pub use utils::*;

/// Shortcut of `std::result::Result<T, irec::Error>`
pub type Result<T> = std::result::Result<T, Error>;
