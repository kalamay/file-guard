//! A cross-platform library for safe advisory file locking.
//!
//! The lock supports both exclusive and shared locking modes for a byte range
//! of an opened `File` object. Exclusively locking a portion of a file denies
//! all other processes both shared and exclusive access to the specified
//! region of the file. Shared locking a portion of a file denies all processes
//! exclusive access to the specified region of the file. The locked range does
//! not need to exist within the file, and the ranges may be used for any
//! arbitrary advisory locking protocol between processes.
//!
//! This result of a [`lock()`], [`try_lock()`], or [`lock_any()`] is a
//! [`FileGuard`]. When dropped, this [`FileGuard`] will unlock the region of
//! the file currently held. This value may also be [`.upgrade()`]'ed to
//! either a shared or exlusive lock.
//!
//! On Unix systems `fcntl` is used to perform the locking, and on Windows, `LockFileEx`.
//!
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
//! Anything that can `Deref` to a `File` can be used with the [`FileGuard`]
//! (i.e. `Rc<File>`):
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
//!
//! [`FileGuard`]: struct.FileGuard.html
//! [`lock()`]: fn.lock.html
//! [`try_lock()`]: fn.try_lock.html
//! [`lock_any()`]: fn.lock_any.html
//! [`.upgrade()`]: struct.FileGuard.html#method.upgrade

//#![deny(missing_docs)]

use std::fs::File;
use std::io::ErrorKind;
use std::ops::{Deref, Range};
use std::{fmt, io};

pub mod os;
use self::os::{raw_file_downgrade, raw_file_lock};

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
    raw_file_lock(&file, Some(lock), offset, len, true)?;
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
    raw_file_lock(&file, Some(lock), offset, len, false)?;
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
    let lock = match raw_file_lock(&file, Some(Lock::Exclusive), offset, len, false) {
        Ok(_) => Lock::Exclusive,
        Err(e) => {
            if e.kind() == ErrorKind::WouldBlock {
                raw_file_lock(&file, Some(Lock::Shared), offset, len, true)?;
                Lock::Shared
            } else {
                return Err(e);
            }
        }
    };
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

    pub fn downgrade(&mut self) -> io::Result<()> {
        if self.is_exclusive() {
            raw_file_downgrade(&self.file, self.offset, self.len)?;
            self.lock = Lock::Shared;
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
        let _ = raw_file_lock(&self.file, None, self.offset, self.len, false);
    }
}
