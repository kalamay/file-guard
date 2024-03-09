//! Provides low-level support operations for file locking on Windows platforms.
use std::fs::File;
use std::io::{self, Error, ErrorKind};
use std::mem::MaybeUninit;
use std::ops::Deref;
use std::os::windows::io::AsRawHandle;

use winapi::shared::minwindef::DWORD;
use winapi::shared::winerror::ERROR_LOCK_VIOLATION;
use winapi::um::fileapi::{LockFileEx, UnlockFileEx};
use winapi::um::minwinbase::{LOCKFILE_EXCLUSIVE_LOCK, LOCKFILE_FAIL_IMMEDIATELY, OVERLAPPED};
use winapi::um::winnt::HANDLE;

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

    let mut ov: OVERLAPPED = MaybeUninit::zeroed().assume_init();
    let s = ov.u.s_mut();
    s.Offset = (off & 0xffffffff) as DWORD;
    s.OffsetHigh = (off >> 16 >> 16) as DWORD;

    let lenlow = (len & 0xffffffff) as DWORD;
    let lenhigh = (len >> 16 >> 16) as DWORD;

    let rc = if let Some(lock) = lock {
        let mut flags = if wait { 0 } else { LOCKFILE_FAIL_IMMEDIATELY };
        if lock == Lock::Exclusive {
            flags |= LOCKFILE_EXCLUSIVE_LOCK;
        }
        LockFileEx(f.as_raw_handle() as HANDLE, flags, 0, lenlow, lenhigh, &mut ov)
    } else {
        UnlockFileEx(f.as_raw_handle() as HANDLE, 0, lenlow, lenhigh, &mut ov)
    };

    if rc == 0 {
        let e = Error::last_os_error();
        if e.raw_os_error() == Some(ERROR_LOCK_VIOLATION as i32) {
            Err(ErrorKind::WouldBlock.into())
        } else {
            Err(e)
        }
    } else {
        Ok(())
    }
}

/// Downgrades a file lock from exclusive to shared.
///
/// # Safety
///
/// This does not guarantee that an exclusive lock is already held.
pub unsafe fn raw_file_downgrade(f: &File, off: usize, len: usize) -> io::Result<()> {
    // Add a shared lock.
    raw_file_lock(f, Some(Lock::Shared), off, len, false)?;
    // Removed the exclusive lock.
    raw_file_lock(f, None, off, len, false)
}

/// Windows-specific extensions to [`FileGuard`].
///
/// [`FileGuard`]: ../../struct.FileGuard.html
pub trait FileGuardExt {}

impl<T> FileGuardExt for FileGuard<T> where T: Deref<Target = File> {}
