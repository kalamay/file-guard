use std::fs::OpenOptions;
use std::io;

mod pipeline;

use file_guard::Lock;

#[test]
fn test_lock_any() -> io::Result<()> {
    let path = "test-lock";
    let f = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(&path)?;
    f.set_len(1024)?;

    let mut a = pipeline::Pipeline::new(&path)
        .lock(Lock::Exclusive, 0, 1)
        .write(0, 1)
        .wait(0, 2)
        .downgrade()
        .write(0, 3)
        .wait(0, 4)
        .unlock()
        .spawn("a")?;

    let mut b = pipeline::Pipeline::new(&path)
        .wait(0, 1)
        .try_lock(Err(Lock::Shared), 0, 1)
        .write(0, 2)
        .wait(0, 3)
        .lock(Lock::Shared, 0, 1)
        .write(0, 4)
        .unlock()
        .spawn("b")?;

    pipeline::interleave(&mut a, &mut b)
}
