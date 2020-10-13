![Verify](https://github.com/kalamay/file-guard/workflows/Verify/badge.svg?branch=main)

# file-guard

A cross-platform library for safe file locking in Rust.

Take a look at the [Documentation](https://docs.rs/file-guard/) for details!

The lock supports both exclusive and shared locking modes for a byte range
of an opened `File` object. Exclusively locking a portion of a file denies
all other processes both shared and exclusive access to the specified
region of the file. Shared locking a portion of a file denies all processes
exclusive access to the specified region of the file. The locked range does
not need to exist within the file, and the ranges may be used for any
arbitrary advisory locking protocol between processes.

This result of a [`lock()`], [`try_lock()`], or [`lock_any()`] is a
[`FileGuard`]. When dropped, this [`FileGuard`] will unlock the region of
the file currently held. This value may also be [`.upgrade()`]'ed to
either a shared or exlusive lock.

On Unix systems `fcntl` is used to perform the locking, and on Windows, `LockFileEx`.

# Examples

```rust
use file_guard::Lock;
use std::fs::OpenOptions;

let file = OpenOptions::new()
    .read(true)
    .write(true)
    .create(true)
    .open("example-lock")?;

let lock = file_guard::lock(&file, Lock::Exclusive, 0, 1)?;
// the lock will be unlocked when it goes out of scope
```

You can store one or more locks in a struct:

```rust
use file_guard::{FileGuard, Lock};
use std::fs::{File, OpenOptions};

let file = OpenOptions::new()
    .read(true)
    .write(true)
    .create(true)
    .open("example-lock")?;

struct Thing<'file> {
    a: FileGuard<&'file File>,
    b: FileGuard<&'file File>,
}

let t = Thing {
    a: file_guard::lock(&file, Lock::Exclusive, 0, 1)?,
    b: file_guard::lock(&file, Lock::Shared, 1, 1)?,
};
// both locks will be unlocked when t goes out of scope
```

Anything that can `Deref` to a `File` can be used with the [`FileGuard`].
This works with `Rc<File>`:

```rust
use file_guard::{FileGuard, Lock};
use std::fs::{File, OpenOptions};
use std::rc::Rc;

let file = Rc::new(
    OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open("example-lock")?
);

struct Thing {
    a: FileGuard<Rc<File>>,
    b: FileGuard<Rc<File>>,
}

let t = Thing {
    a: file_guard::lock(file.clone(), Lock::Exclusive, 0, 1)?,
    b: file_guard::lock(file, Lock::Shared, 1, 1)?,
};
// both locks will be unlocked and the file will be closed when t goes out of scope
```

[`FileGuard`]: https://docs.rs/file-guard/0.1.0/file_guard/struct.FileGuard.html
[`lock()`]: https://docs.rs/file-guard/0.1.0/file_guard/fn.lock.html
[`try_lock()`]: https://docs.rs/file-guard/0.1.0/file_guard/fn.try_lock.html
[`lock_any()`]: https://docs.rs/file-guard/0.1.0/file_guard/fn.lock_any.html
[`.upgrade()`]: https://docs.rs/file-guard/0.1.0/file_guard/struct.FileGuard.html#method.upgrade
