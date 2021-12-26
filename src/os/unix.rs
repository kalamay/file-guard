//! Provides low-level support operations for file locking on UNIX platforms.
use libc::{fcntl, off_t, F_RDLCK, F_SETLK, F_SETLKW, F_UNLCK, F_WRLCK, SEEK_SET};

use std::fs::File;
use std::io::{self, Error, ErrorKind};
use std::ops::Deref;
use std::os::raw::c_short;
use std::os::unix::io::AsRawFd;

use crate::{FileGuard, Lock};

/// Acquires and releases a file lock.
///
/// # Safety
///
/// When used to unlock, this does not guarantee that an exclusive lock is
/// already held.
pub unsafe fn raw_file_lock(
    f: &File,
    lock: Option<Lock>,
    off: usize,
    len: usize,
    wait: bool,
) -> io::Result<()> {
    if len == 0 {
        return Err(ErrorKind::InvalidInput.into());
    }

    let op = match wait {
        true => F_SETLKW,
        false => F_SETLK,
    };

    let lock = libc::flock {
        l_start: off as off_t,
        l_len: len as off_t,
        l_pid: 0,
        l_type: match lock {
            Some(Lock::Shared) => F_RDLCK as c_short,
            Some(Lock::Exclusive) => F_WRLCK as c_short,
            None => F_UNLCK as c_short,
        },
        l_whence: SEEK_SET as c_short,
        #[cfg(any(target_os = "freebsd", target_os = "solaris", target_os = "illumos"))]
        l_sysid: 0,
        #[cfg(any(target_os = "solaris", target_os = "illumos"))]
        l_pad: [0; 4],
    };

    loop {
        let rc = fcntl(f.as_raw_fd(), op, &lock);
        if rc == -1 {
            let err = Error::last_os_error();
            if err.kind() != ErrorKind::Interrupted {
                break Err(err);
            }
        } else {
            break Ok(());
        }
    }
}

/// Downgrades a file lock from exclusive to shared.
///
/// # Safety
///
/// This does not guarantee that an exclusive lock is already held.
pub unsafe fn raw_file_downgrade(f: &File, off: usize, len: usize) -> io::Result<()> {
    raw_file_lock(f, Some(Lock::Shared), off, len, false)
}

/// UNIX-specific extensions to [`FileGuard`].
///
/// [`FileGuard`]: ../../struct.FileGuard.html
pub trait FileGuardExt {
    /// Upgrades a lock from [`Shared`] to [`Exclusive`].
    ///
    /// If the currently held lock is already [`Exclusive`], no change is made
    /// and the method succeeds.
    ///
    /// [`Shared`]: ../../enum.Lock.html#variant.Shared
    /// [`Exclusive`]: ../../enum.Lock.html#variant.Exclusive
    fn upgrade(&mut self) -> io::Result<()>;

    /// Attempts to upgrade a lock from [`Shared`] to [`Exclusive`].
    ///
    /// If the currently held lock is already [`Exclusive`], no change is made
    /// and the method succeeds. If the upgrade cannot be obtained without
    /// blocking, an `Error` of kind `ErrorKind::WouldBlock` is returned.
    ///
    /// [`Shared`]: ../../enum.Lock.html#variant.Shared
    /// [`Exclusive`]: ../../enum.Lock.html#variant.Exclusive
    fn try_upgrade(&mut self) -> io::Result<()>;
}

impl<T> FileGuardExt for FileGuard<T>
where
    T: Deref<Target = File>,
{
    fn upgrade(&mut self) -> io::Result<()> {
        if self.is_shared() {
            unsafe {
                raw_file_lock(
                    &self.file,
                    Some(Lock::Exclusive),
                    self.offset,
                    self.len,
                    true,
                )?;
            }
            self.lock = Lock::Exclusive;
        }
        Ok(())
    }

    fn try_upgrade(&mut self) -> io::Result<()> {
        if self.is_shared() {
            unsafe {
                raw_file_lock(
                    &self.file,
                    Some(Lock::Exclusive),
                    self.offset,
                    self.len,
                    false,
                )?;
            }
            self.lock = Lock::Exclusive;
        }
        Ok(())
    }
}
