//! # Examples
//!
//! ```
//! use file_guard::Lock;
//! use std::fs::OpenOptions;
//!
//! # fn main() -> std::io::Result<()> {
//! let file = OpenOptions::new()
//!     .read(true)
//!     .write(true)
//!     .create(true)
//!     .open("example-lock")?;
//!
//! let lock = file_guard::lock(&file, Lock::Exclusive, 0, 1)?;
//! // the lock will be unlocked when it goes out of scope
//! # Ok(())
//! # }
//! ```
//!
//! You can store one or more locks in a struct:
//!
//! ```
//! use file_guard::{FileGuard, Lock};
//! use std::fs::{File, OpenOptions};
//!
//! # fn main() -> std::io::Result<()> {
//! let file = OpenOptions::new()
//!     .read(true)
//!     .write(true)
//!     .create(true)
//!     .open("example-lock")?;
//!
//! struct Thing<'file> {
//!     a: FileGuard<&'file File>,
//!     b: FileGuard<&'file File>,
//! }
//!
//! let t = Thing {
//!     a: file_guard::lock(&file, Lock::Exclusive, 0, 1)?,
//!     b: file_guard::lock(&file, Lock::Shared, 1, 1)?,
//! };
//! // both locks will be unlocked when t goes out of scope
//! # Ok(())
//! # }
//! ```
//!
//! Anything that can `Deref` to a file can be used with the [`FileGuard`].
//! This works with `Rc<File>`:
//!
//! ```
//! use file_guard::{FileGuard, Lock};
//! use std::fs::{File, OpenOptions};
//! use std::rc::Rc;
//!
//! # fn main() -> std::io::Result<()> {
//! let file = Rc::new(
//!     OpenOptions::new()
//!         .read(true)
//!         .write(true)
//!         .create(true)
//!         .open("example-lock")?
//! );
//!
//! struct Thing {
//!     a: FileGuard<Rc<File>>,
//!     b: FileGuard<Rc<File>>,
//! }
//!
//! let t = Thing {
//!     a: file_guard::lock(file.clone(), Lock::Exclusive, 0, 1)?,
//!     b: file_guard::lock(file, Lock::Shared, 1, 1)?,
//! };
//! // both locks will be unlocked and the file will be closed when t goes out of scope
//! # Ok(())
//! # }
//! ```

//#![warn(missing_docs)]

use std::fs::File;
use std::ops::{Deref, Range};
use std::{fmt, io};

cfg_if::cfg_if! {
    if #[cfg(windows)] {
        mod windows;
        pub use self::windows::{raw_file_lock, raw_file_unlock, raw_file_lock_any};
    } else if #[cfg(unix)] {
        #[macro_use]
        mod unix;
        pub use self::unix::{raw_file_lock, raw_file_unlock, raw_file_lock_any};
    } else {
        // Unknown target_family
    }
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum Lock {
    Shared,
    Exclusive,
}

pub fn lock<T: Deref<Target = File>>(
    file: T,
    lock: Lock,
    offset: usize,
    len: usize,
) -> io::Result<FileGuard<T>> {
    raw_file_lock(&file, lock, offset, len, true)?;
    Ok(FileGuard {
        offset,
        len,
        file,
        lock,
    })
}

pub fn try_lock<T: Deref<Target = File>>(
    file: T,
    lock: Lock,
    offset: usize,
    len: usize,
) -> io::Result<FileGuard<T>> {
    raw_file_lock(&file, lock, offset, len, false)?;
    Ok(FileGuard {
        offset,
        len,
        file,
        lock,
    })
}

pub fn lock_any<T: Deref<Target = File>>(
    file: T,
    offset: usize,
    len: usize,
) -> io::Result<FileGuard<T>> {
    let lock = raw_file_lock_any(&file, offset, len)?;
    Ok(FileGuard {
        offset,
        len,
        file,
        lock,
    })
}

#[must_use = "if unused the file lock will immediately unlock"]
pub struct FileGuard<T: Deref<Target = File>> {
    offset: usize,
    len: usize,
    file: T,
    lock: Lock,
}

impl<T> fmt::Debug for FileGuard<T>
where
    T: Deref<Target = File>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "FileGuard::{:?}({}, {})",
            self.lock, self.offset, self.len
        )
    }
}

impl<T> FileGuard<T>
where
    T: Deref<Target = File>,
{
    #[inline]
    pub fn lock_type(&self) -> Lock {
        self.lock
    }

    #[inline]
    pub fn is_shared(&self) -> bool {
        self.lock == Lock::Shared
    }

    #[inline]
    pub fn is_exclusive(&self) -> bool {
        self.lock == Lock::Exclusive
    }

    #[inline]
    pub fn range(&self) -> Range<usize> {
        self.offset..(self.offset + self.len)
    }

    #[inline]
    pub fn offset(&self) -> usize {
        self.offset
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[inline]
    pub fn upgrade(&mut self, lock: Lock) -> io::Result<()> {
        if self.lock != lock {
            raw_file_lock(&self.file, lock, self.offset, self.len, true)?;
            self.lock = lock;
        }
        Ok(())
    }

    #[inline]
    pub fn try_upgrade(&mut self, lock: Lock) -> io::Result<()> {
        if self.lock != lock {
            raw_file_lock(&self.file, lock, self.offset, self.len, false)?;
            self.lock = lock;
        }
        Ok(())
    }
}

impl<T> Deref for FileGuard<T>
where
    T: Deref<Target = File>,
{
    type Target = T;

    fn deref(&self) -> &T {
        &self.file
    }
}

impl<T> Drop for FileGuard<T>
where
    T: Deref<Target = File>,
{
    #[inline]
    fn drop(&mut self) {
        let _ = raw_file_unlock(&self.file, self.offset, self.len);
    }
}
