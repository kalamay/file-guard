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

Anything that can `Deref` to a file can be used with the [`FileGuard`].
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
