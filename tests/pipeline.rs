use std::io::{self, BufRead, BufReader};
use std::process::{Child, ChildStdout, Command, Stdio};
use std::{env, mem};

use file_guard::Lock;

pub fn interleave(a: &mut PipelineSpawn, b: &mut PipelineSpawn) -> io::Result<()> {
    while a.step()? || b.step()? {}

    Ok(())
}

pub type Try = std::result::Result<Lock, Lock>;

pub struct Pipeline {
    cargo: String,
    dir: String,
    lines: Vec<String>,
    args: Vec<String>,
}

pub struct PipelineSpawn {
    name: &'static str,
    stdout: BufReader<ChildStdout>,
    linebuf: String,
    line: usize,
    lines: Vec<String>,
    child: Child,
}

impl Pipeline {
    pub fn new(path: &str) -> Self {
        let cargo = env::var("CARGO").unwrap();
        let dir = env::var("CARGO_MANIFEST_DIR").unwrap();
        Pipeline {
            cargo,
            dir,
            lines: Vec::new(),
            args: vec![path.to_owned()],
        }
    }

    pub fn spawn(&mut self, name: &'static str) -> io::Result<PipelineSpawn> {
        print!("[{}]$ cargo run --example test-tool -q --", name);
        for v in &self.args {
            print!(" {}", v);
        }
        print!("\n");

        let mut child = Command::new(&self.cargo)
            .current_dir(&self.dir)
            .args(&["run", "--example", "test-tool", "-q", "--"])
            .args(&self.args)
            .stdout(Stdio::piped())
            .spawn()?;
        let stdout = BufReader::new(child.stdout.take().unwrap());
        Ok(PipelineSpawn {
            name,
            stdout,
            linebuf: String::new(),
            line: 0,
            lines: mem::take(&mut self.lines),
            child,
        })
    }

    #[allow(dead_code)]
    pub fn lock(&mut self, lock: Lock, off: usize, len: usize) -> &mut Self {
        self.add_lock("lock", Ok(lock), off, len)
    }

    #[allow(dead_code)]
    pub fn try_lock(&mut self, lock: Try, off: usize, len: usize) -> &mut Self {
        self.add_lock("trylock", lock, off, len)
    }

    #[allow(dead_code)]
    pub fn lock_any(&mut self, expect: Lock, off: usize, len: usize) -> &mut Self {
        self.add_lock_result("lockany", Ok(expect));
        self.add_arg_size2("lockany", off, len);
        self
    }

    #[allow(dead_code)]
    pub fn downgrade(&mut self) -> &mut Self {
        self.add_lock_result("downgrade", Ok(Lock::Shared));
        self.args.push(format!("+downgrade"));
        self
    }

    #[allow(dead_code)]
    pub fn unlock(&mut self) -> &mut Self {
        self.lines.push(format!("+unlock"));
        self.args.push(format!("+unlock"));
        self
    }

    #[allow(dead_code)]
    pub fn write(&mut self, off: usize, val: usize) -> &mut Self {
        self.add_size2("write", off, val)
    }

    #[allow(dead_code)]
    pub fn wait(&mut self, off: usize, val: usize) -> &mut Self {
        self.add_size2("wait", off, val)
    }

    fn add_lock(&mut self, arg: &'static str, lock: Try, off: usize, len: usize) -> &mut Self {
        self.add_lock_result(arg, lock);
        self.add_arg_size2(arg, off, len);
        self.add_arg_type(lock)
    }

    fn add_lock_result(&mut self, arg: &'static str, lock: Try) {
        if let Ok(lock) = lock {
            self.lines.push(format!("+{} {:?}", arg, lock));
        } else {
            self.lines.push(format!("+{} None", arg));
        }
    }

    fn add_size2(&mut self, arg: &'static str, a: usize, b: usize) -> &mut Self {
        self.lines.push(format!("+{} {} {}", arg, a, b));
        self.add_arg_size2(arg, a, b);
        self
    }

    fn add_arg_size2(&mut self, arg: &'static str, a: usize, b: usize) {
        self.args.push(format!("+{}", arg));
        self.args.push(format!("{}", a));
        self.args.push(format!("{}", b));
    }

    fn add_arg_type(&mut self, lock: Try) -> &mut Self {
        self.args.push(match lock.unwrap_or_else(|e| e) {
            Lock::Shared => "sh".to_owned(),
            Lock::Exclusive => "ex".to_owned(),
        });
        self
    }
}

impl PipelineSpawn {
    pub fn step(&mut self) -> io::Result<bool> {
        if self.line < self.lines.len() {
            self.linebuf.clear();
            self.stdout.read_line(&mut self.linebuf)?;
            assert_eq!(self.lines[self.line], self.linebuf.trim_end());
            println!("[{}]: {}", self.name, self.lines[self.line]);
            self.line += 1;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

impl Drop for PipelineSpawn {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}
