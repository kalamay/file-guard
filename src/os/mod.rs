//! Provides low-level support operations for file locking.
#[cfg(windows)]
pub mod windows;

#[cfg(windows)]
pub use self::windows::{raw_file_lock, raw_file_downgrade};

#[cfg(unix)]
#[macro_use]
pub mod unix;

#[cfg(unix)]
pub use self::unix::{raw_file_lock, raw_file_downgrade};
