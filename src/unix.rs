use libc::{fcntl, off_t, F_RDLCK, F_SETLK, F_SETLKW, F_UNLCK, F_WRLCK, SEEK_SET};

use std::fs::File;
use std::io::{self, Error, ErrorKind};
use std::os::raw::{c_int, c_short};
use std::os::unix::io::AsRawFd;

use super::Lock;

fn flck(fd: c_int, op: c_int, typ: c_short, off: usize, len: usize) -> io::Result<()> {
    if len == 0 {
        Err(ErrorKind::InvalidInput.into())
    } else {
        let lock = libc::flock {
            l_start: off as off_t,
            l_len: len as off_t,
            l_pid: 0,
            l_type: typ,
            l_whence: SEEK_SET as c_short,
        };
        loop {
            let rc = unsafe { fcntl(fd, op, &lock) };
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
}

pub fn raw_file_lock(f: &File, lock: Lock, off: usize, len: usize, wait: bool) -> io::Result<()> {
    let typ = match lock {
        Lock::Shared => F_RDLCK,
        Lock::Exclusive => F_WRLCK,
    };
    let op = match wait {
        true => F_SETLKW,
        false => F_SETLK,
    };
    flck(f.as_raw_fd(), op, typ, off, len)
}

pub fn raw_file_unlock(f: &File, off: usize, len: usize) -> io::Result<()> {
    flck(f.as_raw_fd(), F_SETLK, F_UNLCK, off, len)
}

pub fn raw_file_lock_any(f: &File, off: usize, len: usize) -> io::Result<Lock> {
    let fd = f.as_raw_fd();
    flck(fd, F_SETLK, F_WRLCK, off, len)
        .and(Ok(Lock::Exclusive))
        .or_else(|e| {
            if e.kind() == ErrorKind::WouldBlock {
                flck(fd, F_SETLKW, F_RDLCK, off, len).and(Ok(Lock::Shared))
            } else {
                Err(e)
            }
        })
}
