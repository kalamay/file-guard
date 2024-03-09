//! A cross-platform library for simple advisory file locking.
//!
//! The lock supports both exclusive and shared locking modes for a byte range
//! of an opened `File` object. Exclusively locking a portion of a file denies
//! all other processes both shared and exclusive access to the specified
//! region of the file. Shared locking a portion of a file denies all processes
//! exclusive access to the specified region of the file. The locked range does
//! not need to exist within the file, and the ranges may be used for any
//! arbitrary advisory locking protocol between processes.
//!
//! The result of a [`lock()`], [`try_lock()`], or [`lock_any()`] is a
//! [`FileGuard`]. When dropped, this [`FileGuard`] will unlock the region of
//! the file currently held. Exclusive locks may be [`.downgrade()`]'ed to
//! either a shared lock cross platform.
//!
//! On Unix systems `fcntl` is used to perform the locking, and on Windows, `LockFileEx`.
//! All generally available behavior is consistent across platforms. For platform-
//! specific behavior, traits may be used for the respective platform. For example,
//! on Windows, locks cannot be safely upgraded, whereas on Unix systems, this can
//! be done safely and atomically. To use this feature, the
//! [`file_guard::os::unix::FileGuardExt`] may `use`ed, enabling the [`.upgrade()`]
//! and [`.try_upgrade()`] methods.
//!
//! Note that on Windows, the file must be open with write permissions to lock it.
//!
//! # Examples
//!
//! ```
//! use file_guard::Lock;
//! use std::fs::OpenOptions;
//!
//! # fn main() -> std::io::Result<()> {
//! let mut file = OpenOptions::new()
//!     .read(true)
//!     .write(true)
//!     .create(true)
//!     .open("example-lock")?;
//!
//! let mut lock = file_guard::lock(&mut file, Lock::Exclusive, 0, 1)?;
//! write_to_file(&mut lock)?;
//! # fn write_to_file(f: &mut std::fs::File) -> std::io::Result<()> { Ok(()) }
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
//!     b: file_guard::lock(&file, Lock::Shared, 1, 2)?,
//! };
//! // both locks will be unlocked when t goes out of scope
//! # Ok(())
//! # }
//! ```
//!
//! Anything that can `Deref` or `DerefMut` to a `File` can be used with the [`FileGuard`]
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
//!     b: file_guard::lock(file, Lock::Shared, 1, 2)?,
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
//! [`.downgrade()`]: struct.FileGuard.html#method.downgrade
//! [`file_guard::os::unix::FileGuardExt`]: os/unix/trait.FileGuardExt.html
//! [`.upgrade()`]: os/unix/trait.FileGuardExt.html#tymethod.upgrade
//! [`.try_upgrade()`]: os/unix/trait.FileGuardExt.html#tymethod.try_upgrade

#![deny(missing_docs)]

use std::fs::File;
use std::io::ErrorKind;
use std::ops::{Deref, DerefMut, Range};
use std::{fmt, io};

pub mod os;
use self::os::{raw_file_downgrade, raw_file_lock};

/// The type of a lock operation.
///
/// This is used to specify the desired lock type when used with [`lock()`]
/// and [`try_lock()`], and it is the successful result type returned by
/// [`lock_any()`].
///
/// [`lock()`]: fn.lock.html
/// [`try_lock()`]: fn.try_lock.html
/// [`lock_any()`]: fn.lock_any.html
#[derive(Copy, Clone, PartialEq, Debug)]
pub enum Lock {
    /// A shared lock may be concurrently held by multiple processes while
    /// preventing future exclusive locks its lifetime.
    ///
    /// The shared lock type cannot be obtained while an exclusive lock is held
    /// by another process. When successful, a shared lock guarantees that only
    /// one or more shared locks are concurrently held, and that no exclusive
    /// locks are held.
    ///
    /// This lock type--often referred to as a read lock--may be used as a
    /// means of signaling read integrity. When used cooperatively, they ensure
    /// no exclusive lock is held, and thus, no other process may be writing to
    /// a shared resource.
    Shared,
    /// An exclusive lock may only be held by a single process.
    ///
    /// The exclusive lock type can neither be obtained while any shared locks
    /// are held or while any other exclusive locks are held. This linearizes
    /// the sequence of processes attempting to acquire an exclusive lock.
    ///
    /// This lock type--also known as a write lock--may be used as a means of
    /// ensuring write exclusivity. In a cooperative locking environment, all
    /// access to a shared resource is halted until the exlusive lock is
    /// released.
    Exclusive,
}

/// Wait and claim the desired [`Lock`] type using a byte range of a file.
///
/// The byte range does not need to exist in the underlying file.
///
/// [`Lock`]: enum.Lock.html
pub fn lock<T: Deref<Target = File>>(
    file: T,
    lock: Lock,
    offset: usize,
    len: usize,
) -> io::Result<FileGuard<T>> {
    unsafe {
        raw_file_lock(&file, Some(lock), offset, len, true)?;
    }
    Ok(FileGuard {
        offset,
        len,
        file,
        lock,
    })
}

/// Attempt to claim the desired [`Lock`] type using a byte range of a file.
///
/// If the desired [`Lock`] type cannot be obtained without blocking, an
/// `Error` of kind `ErrorKind::WouldBlock` is returned. Otherwise if
/// successful, the lock is held.
///
/// The byte range does not need to exist in the underlying file.
///
/// [`Lock`]: enum.Lock.html
pub fn try_lock<T: Deref<Target = File>>(
    file: T,
    lock: Lock,
    offset: usize,
    len: usize,
) -> io::Result<FileGuard<T>> {
    unsafe {
        raw_file_lock(&file, Some(lock), offset, len, false)?;
    }
    Ok(FileGuard {
        offset,
        len,
        file,
        lock,
    })
}

/// First attempt to claim an [`Exclusive`] lock and then fallback to a
/// [`Shared`] lock for a byte range of a file. This is not currently an
/// atomic operation.
///
/// When successful, the [`FileGuard`] may be inspected for the lock type
/// obtained using [`.lock_type()`], [`.is_shared()`], or [`.is_exclusive()`].
///
/// The byte range does not need to exist in the underlying file.
///
/// [`Exclusive`]: enum.Lock.html#variant.Exclusive
/// [`Shared`]: enum.Lock.html#variant.Shared
/// [`FileGuard`]: struct.FileGuard.html
/// [`.lock_type()`]: struct.FileGuard.html#method.lock_type
/// [`.is_shared()`]: struct.FileGuard.html#method.is_shared
/// [`.is_exclusive()`]: struct.FileGuard.html#method.is_exclusive
pub fn lock_any<T: Deref<Target = File>>(
    file: T,
    offset: usize,
    len: usize,
) -> io::Result<FileGuard<T>> {
    let lock = match unsafe { raw_file_lock(&file, Some(Lock::Exclusive), offset, len, false) } {
        Ok(_) => Lock::Exclusive,
        Err(e) => {
            if e.kind() == ErrorKind::WouldBlock {
                unsafe {
                    raw_file_lock(&file, Some(Lock::Shared), offset, len, true)?;
                }
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

/// An RAII implementation of a "scoped lock" of a file. When this structure
/// is dropped (falls out of scope), the lock will be unlocked.
///
/// This structure is created by the [`lock()`], [`try_lock()`], and
/// [`lock_any()`] functions.
///
/// [`lock()`]: fn.lock.html
/// [`try_lock()`]: fn.try_lock.html
/// [`lock_any()`]: fn.lock_any.html
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
    /// Gets the [`Lock`] type currently held.
    ///
    /// [`Lock`]: enum.Lock.html
    #[inline]
    pub fn lock_type(&self) -> Lock {
        self.lock
    }

    /// Test if the currently held [`Lock`] type is [`Shared`].
    ///
    /// [`Lock`]: enum.Lock.html
    /// [`Shared`]: enum.Lock.html#variant.Shared
    #[inline]
    pub fn is_shared(&self) -> bool {
        self.lock == Lock::Shared
    }

    /// Test if the currently held [`Lock`] type is [`Exclusive`].
    ///
    /// [`Lock`]: enum.Lock.html
    /// [`Exclusive`]: enum.Lock.html#variant.Exclusive
    #[inline]
    pub fn is_exclusive(&self) -> bool {
        self.lock == Lock::Exclusive
    }

    /// Gets the byte range of the held lock.
    #[inline]
    pub fn range(&self) -> Range<usize> {
        self.offset..(self.offset + self.len)
    }

    /// Gets the byte offset of the held lock.
    #[inline]
    pub fn offset(&self) -> usize {
        self.offset
    }

    /// Gets the byte length of the held lock.
    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }

    /// Tests if the byte range of the lock has a length of zero.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Safely exchanges an [`Exclusive`] [`Lock`] for a [`Shared`] one.
    ///
    /// If the currently held lock is already [`Shared`], no change is made and
    /// the method succeeds. This exchange safely ensures no lock is released
    /// during operation. That is, no waiting [`Exclusive`] lock attempts may
    /// obtain the lock during the downgrade. Other [`Shared`] locks waiting
    /// will be granted a lock as a result, however.
    ///
    /// [`Lock`]: enum.Lock.html
    /// [`Exclusive`]: enum.Lock.html#variant.Exclusive
    /// [`Shared`]: enum.Lock.html#variant.Shared
    pub fn downgrade(&mut self) -> io::Result<()> {
        if self.is_exclusive() {
            unsafe {
                raw_file_downgrade(&self.file, self.offset, self.len)?;
            }
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

impl<T> DerefMut for FileGuard<T>
where
    T: DerefMut<Target = File>,
{
    fn deref_mut(&mut self) -> &mut T {
        &mut self.file
    }
}

impl<T> Drop for FileGuard<T>
where
    T: Deref<Target = File>,
{
    #[inline]
    fn drop(&mut self) {
        let _ = unsafe { raw_file_lock(&self.file, None, self.offset, self.len, false) };
    }
}
