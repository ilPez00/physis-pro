//! A terminal pane: run a command in the document's directory and show what it
//! said.
//!
//! # This executes arbitrary commands
//!
//! `sh -c <the line you typed>`, as you, with your environment. That is what a
//! terminal is, and it is the one part of this program that can destroy
//! anything you can destroy. Nothing runs automatically: no file, no
//! transition, no observation and no model output is ever fed to it. The only
//! input is a line a person typed, in the pane, with the pane in front of them.
//!
//! The run happens on its own thread so a slow command cannot freeze the
//! editor, and output is capped so a runaway one cannot exhaust memory. A
//! command that is still running when the editor exits keeps running — there is
//! no process group to signal here, and pretending otherwise would be worse
//! than saying so.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{channel, Receiver};

/// Cap on captured output per stream. Beyond this the tail is kept, because the
/// end of a build log is the part that matters.
const MAX_OUTPUT_BYTES: usize = 256 * 1024;

pub struct Outcome {
    pub command: String,
    pub cwd: PathBuf,
    pub stdout: String,
    pub stderr: String,
    pub code: Option<i32>,
    pub truncated: bool,
}

impl Outcome {
    pub fn ok(&self) -> bool {
        self.code == Some(0)
    }
}

pub struct Runner {
    rx: Option<Receiver<Outcome>>,
    pub running: Option<String>,
}

impl Runner {
    pub fn new() -> Runner {
        Runner {
            rx: None,
            running: None,
        }
    }

    pub fn busy(&self) -> bool {
        self.rx.is_some()
    }

    /// Start `line` with `sh -c` in `cwd`. Replaces any previous run's channel;
    /// the previous child is not killed, it is simply no longer listened to.
    pub fn run(&mut self, line: &str, cwd: &Path) {
        let command = line.trim().to_string();
        if command.is_empty() {
            return;
        }
        let (tx, rx) = channel();
        let dir = cwd.to_path_buf();
        let cmd = command.clone();
        std::thread::spawn(move || {
            let _ = tx.send(execute(&cmd, &dir));
        });
        self.rx = Some(rx);
        self.running = Some(command);
    }

    /// Non-blocking poll. `None` means still running.
    pub fn poll(&mut self) -> Option<Outcome> {
        let rx = self.rx.as_ref()?;
        match rx.try_recv() {
            Ok(outcome) => {
                self.rx = None;
                self.running = None;
                Some(outcome)
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => None,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                self.rx = None;
                self.running = None;
                Some(Outcome {
                    command: String::new(),
                    cwd: PathBuf::new(),
                    stdout: String::new(),
                    stderr: "the command thread stopped without a result".into(),
                    code: None,
                    truncated: false,
                })
            }
        }
    }
}

fn execute(command: &str, cwd: &Path) -> Outcome {
    let spawned = Command::new("sh")
        .arg("-c")
        .arg(command)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();
    let mut child = match spawned {
        Ok(c) => c,
        Err(e) => {
            return Outcome {
                command: command.to_string(),
                cwd: cwd.to_path_buf(),
                stdout: String::new(),
                stderr: format!("cannot start sh: {e}"),
                code: None,
                truncated: false,
            }
        }
    };
    // Read both pipes before waiting, or a child that fills the other pipe
    // deadlocks against us.
    let mut out = Vec::new();
    let mut err = Vec::new();
    if let Some(mut s) = child.stdout.take() {
        let _ = s.read_to_end(&mut out);
    }
    if let Some(mut s) = child.stderr.take() {
        let _ = s.read_to_end(&mut err);
    }
    let code = match child.wait() {
        Ok(s) => s.code(),
        Err(e) => {
            err.extend_from_slice(format!("\nwait failed: {e}").as_bytes());
            None
        }
    };
    let (out, t1) = cap(out);
    let (err, t2) = cap(err);
    Outcome {
        command: command.to_string(),
        cwd: cwd.to_path_buf(),
        stdout: out,
        stderr: err,
        code,
        truncated: t1 || t2,
    }
}

fn cap(bytes: Vec<u8>) -> (String, bool) {
    let truncated = bytes.len() > MAX_OUTPUT_BYTES;
    let slice = if truncated {
        &bytes[bytes.len() - MAX_OUTPUT_BYTES..]
    } else {
        &bytes[..]
    };
    (
        String::from_utf8_lossy(slice).into_owned(),
        truncated,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_to_completion(line: &str) -> Outcome {
        let mut r = Runner::new();
        r.run(line, Path::new("/tmp"));
        for _ in 0..2000 {
            if let Some(o) = r.poll() {
                return o;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        panic!("command did not finish");
    }

    #[test]
    fn captures_stdout_and_the_exit_code() {
        let o = run_to_completion("printf hello");
        assert_eq!(o.stdout, "hello");
        assert!(o.ok());
        assert_eq!(o.code, Some(0));
    }

    #[test]
    fn captures_a_failing_exit_code_and_stderr() {
        let o = run_to_completion("printf oops >&2; exit 3");
        assert_eq!(o.code, Some(3));
        assert!(!o.ok());
        assert!(o.stderr.contains("oops"));
    }

    #[test]
    fn runs_in_the_given_directory() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("marker.txt"), "x").unwrap();
        let mut r = Runner::new();
        r.run("ls", dir.path());
        for _ in 0..2000 {
            if let Some(o) = r.poll() {
                assert!(o.stdout.contains("marker.txt"), "ran in the wrong place: {o:?}", o = o.stdout);
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        panic!("no result");
    }

    #[test]
    fn a_runaway_command_is_capped_not_swallowed() {
        let o = run_to_completion("yes | head -c 400000");
        assert!(o.truncated, "output beyond the cap is reported");
        assert!(o.stdout.len() <= MAX_OUTPUT_BYTES);
    }

    #[test]
    fn an_empty_line_runs_nothing() {
        let mut r = Runner::new();
        r.run("   ", Path::new("/tmp"));
        assert!(!r.busy(), "no child for a blank line");
    }
}
