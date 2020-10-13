use std::fs::File;
use std::io::{self, Error, ErrorKind};
use std::mem::MaybeUninit;
use std::os::windows::io::AsRawHandle;
use std::os::windows::raw::HANDLE;

use winapi::shared::minwindef::DWORD;
use winapi::shared::winerror::ERROR_LOCK_VIOLATION;
use winapi::um::fileapi::{LockFileEx, UnlockFileEx};
use winapi::um::minwinbase::{LOCKFILE_EXCLUSIVE_LOCK, LOCKFILE_FAIL_IMMEDIATELY, OVERLAPPED};

use super::Lock;

fn overlapped(off: usize) -> OVERLAPPED {
    let mut ov: OVERLAPPED = unsafe { MaybeUninit::zeroed().assume_init() };
    let mut s = unsafe { ov.u.s_mut() };
    s.Offset = (off & 0xffffffff) as DWORD;
    s.OffsetHigh = (off >> 32) as DWORD;
    ov
}

pub fn raw_file_lock(
    f: &File,
    lock: Option<Lock>,
    off: usize,
    len: usize,
    wait: bool,
) -> io::Result<()> {
    if len == 0 {
        Err(ErrorKind::InvalidInput.into())
    } else {
        let mut ov = overlapped(off);

        let rc = if let Some(lock) = lock {
            let mut flags = if wait { 0 } else { LOCKFILE_FAIL_IMMEDIATELY };
            if lock == Lock::Exclusive {
                flags = flags | LOCKFILE_EXCLUSIVE_LOCK;
            }
            unsafe {
                LockFileEx(
                    f.as_raw_handle(),
                    flags,
                    0,
                    (len & 0xffffffff) as DWORD,
                    (len >> 32) as DWORD,
                    &mut ov,
                )
            }
        } else {
            unsafe {
                UnlockFileEx(
                    f.as_raw_handle(),
                    0,
                    (len & 0xffffffff) as DWORD,
                    (len >> 32) as DWORD,
                    &mut ov,
                )
            }
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
}

pub fn raw_file_downgrade(f: &File, off: usize, len: usize) -> io::Result<()> {
    // Add a shared lock.
    raw_file_lock(f, Some(Lock::Shared), off, len, false)?;
    // Removed the exclusive lock.
    raw_file_lock(f, None, off, len, false)?;
}
