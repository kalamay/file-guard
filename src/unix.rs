use libc::{fcntl, off_t, F_RDLCK, F_SETLK, F_SETLKW, F_UNLCK, F_WRLCK, SEEK_SET};

use std::fs::File;
use std::io::{self, Error, ErrorKind};
use std::ops::Deref;
use std::os::raw::c_short;
use std::os::unix::io::AsRawFd;

use super::{FileGuard, Lock};

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
        };
        loop {
            let rc = unsafe { fcntl(f.as_raw_fd(), op, &lock) };
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

pub fn raw_file_downgrade(f: &File, off: usize, len: usize) -> io::Result<()> {
    raw_file_lock(f, Some(Lock::Shared), off, len, false)
}

pub trait Upgrade {
    fn upgrade(&mut self) -> io::Result<()>;
    fn try_upgrade(&mut self) -> io::Result<()>;
}

impl<T> Upgrade for FileGuard<T>
where
    T: Deref<Target = File>,
{
    fn upgrade(&mut self) -> io::Result<()> {
        if self.is_shared() {
            raw_file_lock(
                &self.file,
                Some(Lock::Exclusive),
                self.offset,
                self.len,
                true,
            )?;
            self.lock = Lock::Exclusive;
        }
        Ok(())
    }

    fn try_upgrade(&mut self) -> io::Result<()> {
        if self.is_shared() {
            raw_file_lock(
                &self.file,
                Some(Lock::Exclusive),
                self.offset,
                self.len,
                false,
            )?;
            self.lock = Lock::Exclusive;
        }
        Ok(())
    }
}
