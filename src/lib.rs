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
        match raw_file_unlock(&self.file, self.offset, self.len) {
            _ => {}
        }
    }
}

/*
#[cfg(test)]
mod tests {
    #[macro_use]
    extern crate tokio;

    use std::fs::OpenOptions;
    use std::io;

    use file_guard::Lock;

    #[path = "../../tests/pipeline.rs"]
    mod pipeline;

    #[tokio::test]
    async fn test_lock_any() -> io::Result<((), ())> {
        let path = "test-lock-any";
        let f = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&path)?;
        f.set_len(1024)?;

        let mut a = pipeline::Pipeline::new(&path);
        a.lock_any(Lock::Exclusive, 0, 1)
            .write(0, 1)
            .wait(0, 2)
            .unlock()
            .write(0, 3);

        let mut b = pipeline::Pipeline::new(&path);
        b.wait(0, 1)
            .lock_any(Lock::Shared, 0, 1)
            .write(0, 2)
            .unlock()
            .wait(0, 3)
            .lock_any(Lock::Exclusive, 0, 1)
            .unlock();

        try_join!(a.run("a"), b.run("b"))
    }
}
*/

/*
#[cfg(all(windows, test))]
mod tests {
    use std::fs::OpenOptions;
    use std::io::{Error, ErrorKind};
    use std::{ptr, thread, time};

    use super::*;

    #[test]
    fn test_lock() -> io::Result<()> {
        let mut map = vmap::MapMut::new(16).expect("failed to allocate shared memory");
        map[0] = 0;

        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open("test-lock")
            .expect("failed to open file");

        match unsafe { libc::fork() } {
            -1 => Err(Error::last_os_error()),
            0 => {
                while map[0] == 0 {
                    thread::sleep(time::Duration::from_millis(1));
                }
                let lock = lock_any(&file, 0, 1)?;
                assert_eq!(lock.lock_type(), Lock::Shared);
                Ok(())
            }
            pid => {
                let mut lock = lock_any(&file, 0, 1)?;
                assert_eq!(lock.lock_type(), Lock::Exclusive);
                thread::sleep(time::Duration::from_millis(20));
                lock.upgrade(Lock::Shared)?;
                map[0] = 1;
                unsafe {
                    libc::waitpid(pid, ptr::null_mut(), 0);
                }
                Ok(())
            }
        }
    }

    #[test]
    fn test_lock_fail() -> io::Result<()> {
        let mut map = vmap::MapMut::new(16).expect("failed to allocate shared memory");
        map[0] = 0;

        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open("test-lock-fail")
            .expect("failed to open file");

        match unsafe { libc::fork() } {
            -1 => Err(Error::last_os_error()),
            0 => {
                while map[0] == 0 {
                    thread::sleep(time::Duration::from_millis(1));
                }
                let lock = try_lock(&file, Lock::Shared, 0, 1);
                assert!(lock.is_err());
                assert_eq!(lock.unwrap_err().kind(), ErrorKind::WouldBlock);
                Ok(())
            }
            pid => {
                let lock = try_lock(&file, Lock::Exclusive, 0, 1);
                map[0] = 1;
                assert!(lock.is_ok());
                assert!(lock.unwrap().is_exclusive());
                unsafe {
                    libc::waitpid(pid, ptr::null_mut(), 0);
                }
                Ok(())
            }
        }
    }

    #[test]
    fn test_locks() -> io::Result<()> {
        let mut map = vmap::MapMut::new(16).expect("failed to allocate shared memory");
        map[0] = 0;

        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open("test-locks")
            .expect("failed to open file");

        match unsafe { libc::fork() } {
            -1 => Err(Error::last_os_error()),
            0 => {
                while map[0] == 0 {
                    thread::sleep(time::Duration::from_millis(1));
                }
                let lock1 = try_lock(&file, Lock::Shared, 0, 1);
                assert!(lock1.is_err());
                assert_eq!(lock1.unwrap_err().kind(), ErrorKind::WouldBlock);

                let lock2 = try_lock(&file, Lock::Shared, 1, 1);
                assert!(lock2.is_ok());
                assert_eq!(lock2.unwrap().lock_type(), Lock::Shared);
                Ok(())
            }
            pid => {
                let lock1 = try_lock(&file, Lock::Exclusive, 0, 1);
                assert!(lock1.is_ok());
                assert_eq!(lock1.unwrap().lock_type(), Lock::Exclusive);

                let lock2 = try_lock(&file, Lock::Exclusive, 1, 1);
                assert!(lock2.is_ok());
                assert_eq!(lock2.unwrap().lock_type(), Lock::Exclusive);

                thread::sleep(time::Duration::from_millis(20));
                //lock2.upgrade(Lock::Shared)?;
                map[0] = 1;
                unsafe {
                    libc::waitpid(pid, ptr::null_mut(), 0);
                }
                Ok(())
            }
        }
    }
}
*/
