use std::fs::File;
use std::io::ErrorKind;
use std::{env, thread, time};

use vmap::{MapMut, Span, SpanMut};

use file_guard::Lock;

macro_rules! error {
    ($($arg:tt)*) => ({
        println!("error: {}", format_args!($($arg)*));
        std::process::exit(1);
    })
}

fn main() {
    let mut args = env::args();

    args.next();

    let path = next(&mut args, "path");
    let (mut map, file) = MapMut::with_options()
        .open(&path)
        .unwrap_or_else(|e| error!("cannot open {:?}: {}", path, e));

    run(&mut args, &mut map, &file);
}

fn run(args: &mut env::Args, map: &mut MapMut, file: &File) {
    let mut guard = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "+lock" => {
                let (lock, off, len) = lock_values(args, map.len());
                if let Some(_) = guard {
                    error!("lock is already held");
                } else {
                    let g = file_guard::lock(file, lock, off, len)
                        .unwrap_or_else(|e| error!("{:?} lock failed: {}", lock, e));
                    println!("{} {:?}", arg, lock);
                    guard = Some(g);
                }
            }
            "+trylock" => {
                let (lock, off, len) = lock_values(args, map.len());
                if let Some(_) = guard {
                    error!("lock is already held");
                } else {
                    match file_guard::try_lock(file, lock, off, len) {
                        Err(e) => {
                            if e.kind() == ErrorKind::WouldBlock {
                                println!("{} None", arg)
                            } else {
                                error!("{:?} try lock failed: {}", lock, e)
                            }
                        }
                        Ok(g) => {
                            println!("{} {:?}", arg, lock);
                            guard = Some(g);
                        }
                    }
                }
            }
            "+lockany" => {
                let (off, len) = lock_size(args, map.len());
                if let Some(_) = guard {
                    error!("lock is already held");
                } else {
                    let g = file_guard::lock_any(file, off, len)
                        .unwrap_or_else(|e| error!("any lock failed: {}", e));
                    println!("{} {:?}", arg, g.lock_type());
                    guard = Some(g);
                }
            }
            "+downgrade" => {
                if let Some(ref mut g) = guard {
                    g.downgrade()
                        .unwrap_or_else(|e| error!("downgrade failed: {}", e));
                    println!("{} {:?}", arg, Lock::Shared);
                } else {
                    error!("lock is not held");
                }
            }
            "+unlock" => {
                if let Some(_) = guard {
                    guard = None;
                    println!("{}", arg);
                } else {
                    error!("lock is not held");
                }
            }
            "+write" => {
                let off = int(args, 0, map.len());
                let val = int(args, 0, usize::MAX);
                map.write_volatile(off, val);
                println!("{} {} {}", arg, off, val);
            }
            "+wait" => {
                let off = int(args, 0, map.len());
                let val = int(args, 0, usize::MAX);
                let mut total = 0;
                while map.read_volatile::<usize>(off) != val {
                    if total == 500 {
                        error!("timed out");
                    }
                    thread::sleep(time::Duration::from_millis(10));
                    total += 1;
                }
                println!("{} {} {}", arg, off, val);
            }
            arg => error!("unknown argument {}", arg),
        }
    }
}

fn next(args: &mut env::Args, what: &'static str) -> String {
    match args.next() {
        Some(v) => v,
        None => error!("{} expected", what),
    }
}

fn int(args: &mut env::Args, min: usize, max: usize) -> usize {
    let val = args.next().unwrap_or_else(|| error!("integer expected"));
    let s = val
        .parse::<usize>()
        .unwrap_or_else(|_| error!("must be a valid integer"));
    if s < min {
        error!("value must be at least {}", min)
    }
    if s > max {
        error!("value must be at most {}", max)
    }
    s
}

fn lock_values(args: &mut env::Args, len: usize) -> (Lock, usize, usize) {
    let (off, len) = lock_size(args, len);
    (lock_type(args), off, len)
}

fn lock_type(args: &mut env::Args) -> Lock {
    match next(args, "lock type").as_str() {
        "sh" | "shared" => Lock::Shared,
        "ex" | "exclusive" => Lock::Exclusive,
        other => error!("unknown lock type {}", other),
    }
}

fn lock_size(args: &mut env::Args, len: usize) -> (usize, usize) {
    let off = int(args, 0, len - 1);
    let len = int(args, 1, len - off);
    (off, len)
}
