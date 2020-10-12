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

fn flock(file: HANDLE, flags: DWORD, off: usize, len: usize) -> io::Result<()> {
    if len == 0 {
        Err(ErrorKind::InvalidInput.into())
    } else {
        let mut ov: OVERLAPPED = unsafe { MaybeUninit::zeroed().assume_init() };
        let mut s = unsafe { ov.u.s_mut() };
        s.Offset = (off & 0xffffffff) as DWORD;
        s.OffsetHigh = (off >> 32) as DWORD;

        let rc = unsafe {
            LockFileEx(
                file,
                flags,
                0,
                (len & 0xffffffff) as DWORD,
                (len >> 32) as DWORD,
                &mut ov,
            )
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

pub fn raw_file_lock(f: &File, lock: Lock, off: usize, len: usize, wait: bool) -> io::Result<()> {
    let mut flags = if wait { 0 } else { LOCKFILE_FAIL_IMMEDIATELY };
    if lock == Lock::Exclusive {
        flags = flags | LOCKFILE_EXCLUSIVE_LOCK;
    }
    flock(f.as_raw_handle(), flags, off, len)
}

pub fn raw_file_unlock(f: &File, off: usize, len: usize) -> io::Result<()> {
    if len == 0 {
        Err(ErrorKind::InvalidInput.into())
    } else {
        let mut ov: OVERLAPPED = unsafe { MaybeUninit::zeroed().assume_init() };
        let mut s = unsafe { ov.u.s_mut() };
        s.Offset = (off & 0xffffffff) as DWORD;
        s.OffsetHigh = (off >> 32) as DWORD;

        let rc = unsafe {
            UnlockFileEx(
                f.as_raw_handle(),
                0,
                (len & 0xffffffff) as DWORD,
                (len >> 32) as DWORD,
                &mut ov,
            )
        };

        if rc == 0 {
            Err(Error::last_os_error())
        } else {
            Ok(())
        }
    }
}

pub fn raw_file_lock_any(f: &File, off: usize, len: usize) -> io::Result<Lock> {
    let fd = f.as_raw_handle();
    flock(
        fd,
        LOCKFILE_EXCLUSIVE_LOCK | LOCKFILE_FAIL_IMMEDIATELY,
        off,
        len,
    )
    .and(Ok(Lock::Exclusive))
    .or_else(|e| {
        if e.kind() == ErrorKind::WouldBlock {
            flock(fd, 0, off, len).and(Ok(Lock::Shared))
        } else {
            Err(e)
        }
    })
}
