//! Run Cargo for the ownership check: forward its output as it arrives and read test results from the
//! machine-readable report of nextest, `--message-format libtest-json` at the fixed structure
//! `--message-format-version 0.1` of the pinned nextest, never from its human output, whose text differs between
//! versions.
//!
//! Every Cargo child runs in its own process group. An interrupted check (SIGINT or SIGTERM, see
//! [`install_signal_handler`]) or a run that ends early terminates that group and waits for the child.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fmt;
use std::io::{self, BufRead, BufReader, Write};
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::Path;
use std::process::{Child, ChildStdout, Command, ExitStatus};
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::thread;

use cargo_metadata::libtest::{TestEvent, TestMessage};
use rustix::process::{Pid, Signal, kill_process_group};
use signal_hook::consts::{SIGINT, SIGTERM};
use signal_hook::iterator::Signals;

use crate::OwnershipError;

/// The report options and the variable that enables this experimental format of nextest.
pub const REPORT: [&str; 4] = [
    "--message-format",
    "libtest-json",
    "--message-format-version",
    "0.1",
];
pub const ENVIRONMENT: [(&str, &str); 1] = [("NEXTEST_EXPERIMENTAL_LIBTEST_JSON", "1")];

/// The reason of the `failed` event that nextest reports for a case it ended at its slow-timeout limit.
const TIME_LIMIT_EXCEEDED: &str = "time limit exceeded";

/// The process group of the running Cargo child, if any, and the signal that interrupted the check.
static CHILD: Mutex<Option<Pid>> = Mutex::new(None);
static INTERRUPTED: AtomicI32 = AtomicI32::new(0);

/// The `cargo` program: the one on `PATH`, or the one on the given `PATH` value.
#[derive(Clone, Debug, Default)]
pub struct Cargo {
    path: Option<OsString>,
}

impl Cargo {
    /// Run `cargo` from the given `PATH` value instead of the inherited one.
    pub fn with_path(path: OsString) -> Self {
        Self { path: Some(path) }
    }

    pub fn command(&self) -> Command {
        let mut command = Command::new("cargo");
        if let Some(path) = &self.path {
            // On Unix a PATH set on the command is also the PATH that finds the program.
            command.env("PATH", path);
        }
        command
    }
}

/// How a child ended: its exit code, or the signal that ended it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Exit {
    Code(i32),
    Signal(i32),
}

impl Exit {
    pub fn success(self) -> bool {
        self == Self::Code(0)
    }

    fn from_status(status: ExitStatus) -> Self {
        match (status.code(), status.signal()) {
            (Some(code), _) => Self::Code(code),
            (None, Some(signal)) => Self::Signal(signal),
            (None, None) => unreachable!("a Unix exit status has a code or a signal"),
        }
    }
}

impl fmt::Display for Exit {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Code(code) => write!(formatter, "{code}"),
            Self::Signal(signal) => write!(formatter, "signal {signal}"),
        }
    }
}

/// The exit and the combined standard output and error of a Cargo run.
#[derive(Clone, Debug)]
pub struct Outcome {
    pub exit: Exit,
    pub output: String,
}

/// The signal that interrupted the check, if one did.
pub fn interrupted() -> Option<i32> {
    match INTERRUPTED.load(Ordering::SeqCst) {
        0 => None,
        signal => Some(signal),
    }
}

fn current_child() -> MutexGuard<'static, Option<Pid>> {
    CHILD.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Handle SIGINT and SIGTERM for the process: a Cargo child runs in its own process group, so a terminal interrupt
/// does not reach it. A signal terminates the running child's group; the check then fails as interrupted. Without a
/// running child the process exits at once with 128 plus the signal number.
pub fn install_signal_handler() -> io::Result<()> {
    let mut signals = Signals::new([SIGINT, SIGTERM])?;
    thread::spawn(move || {
        for signal in signals.forever() {
            INTERRUPTED.store(signal, Ordering::SeqCst);
            let child = current_child();
            match *child {
                Some(group) => terminate(group),
                None => std::process::exit(128 + signal),
            }
        }
    });
    Ok(())
}

fn terminate(group: Pid) {
    match kill_process_group(group, Signal::KILL) {
        // The group has already exited; waiting still reaps the child.
        Ok(()) | Err(rustix::io::Errno::SRCH) => {}
        Err(error) => eprintln!("cannot terminate Cargo process group {group:?}: {error}"),
    }
}

/// A Cargo child in its own process group. Dropping it before [`Supervised::wait`] terminates the group and waits.
pub struct Supervised {
    child: Option<Child>,
}

impl Supervised {
    /// Start the command in a new process group and register it for the signal handler. The registration lock is
    /// held while the child starts, so a signal either sees the child or arrives before it exists.
    pub fn spawn(command: &mut Command) -> Result<Self, OwnershipError> {
        command.process_group(0);
        let mut current = current_child();
        if let Some(signal) = interrupted() {
            return Err(OwnershipError::interrupted(signal));
        }
        let child = command
            .spawn()
            .map_err(|error| OwnershipError(format!("cannot start cargo: {error}")))?;
        *current = Some(Pid::from_child(&child));
        Ok(Self { child: Some(child) })
    }

    /// The piped standard output of the child.
    pub fn stdout(&mut self) -> Option<ChildStdout> {
        self.child.as_mut()?.stdout.take()
    }

    /// Wait for the child without a time limit; an interrupted check fails here.
    pub fn wait(mut self) -> Result<Exit, OwnershipError> {
        let Some(mut child) = self.child.take() else {
            unreachable!("a supervised child is waited for once");
        };
        let status = child.wait();
        *current_child() = None;
        let status =
            status.map_err(|error| OwnershipError(format!("cannot wait for cargo: {error}")))?;
        if let Some(signal) = interrupted() {
            return Err(OwnershipError::interrupted(signal));
        }
        Ok(Exit::from_status(status))
    }
}

impl Drop for Supervised {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            terminate(Pid::from_child(&child));
            // The child is reaped so that no zombie remains; its status no longer matters.
            let _ = child.wait();
            *current_child() = None;
        }
    }
}

/// Run `cargo <args>` in `root` with the report environment, forward its combined standard output and error to `out`
/// line by line as it arrives, and return the exit and the whole output.
pub fn run(
    cargo: &Cargo,
    root: &Path,
    args: &[&str],
    out: &mut dyn Write,
) -> Result<Outcome, OwnershipError> {
    writeln!(out, "COMMAND cargo {}", args.join(" "))?;
    out.flush()?;
    let (reader, writer) = io::pipe()?;
    let mut command = cargo.command();
    command
        .args(args)
        .current_dir(root)
        .envs(ENVIRONMENT)
        .stdout(writer.try_clone()?)
        .stderr(writer);
    let child = Supervised::spawn(&mut command)?;
    // The command holds the parent's copies of the write end; the read below ends when the children close theirs.
    drop(command);
    let mut reader = BufReader::new(reader);
    let mut output = String::new();
    let mut line = Vec::new();
    loop {
        line.clear();
        if reader.read_until(b'\n', &mut line)? == 0 {
            break;
        }
        let text = String::from_utf8_lossy(&line);
        out.write_all(text.as_bytes())?;
        out.flush()?;
        output.push_str(&text);
    }
    let exit = child.wait()?;
    Ok(Outcome { exit, output })
}

/// The last event of each test in the report: `ok`, `failed`, `ignored` or `hung`. A test is named
/// `<package>::<binary>$<test>`; lines that are not report events are skipped, and an unreadable event fails.
/// nextest ends a case at its slow-timeout limit, which only ends a hung case, and reports it as a failure with the
/// reason `time limit exceeded` (libtest's own `timeout` event is read the same way); that result is named `hung`.
pub fn results(output: &str) -> Result<BTreeMap<String, &'static str>, OwnershipError> {
    let mut events = BTreeMap::new();
    for line in output.lines() {
        if !line.starts_with("{\"type\":") {
            continue;
        }
        let message: TestMessage = serde_json::from_str(line).map_err(|error| {
            OwnershipError(format!("unreadable nextest report line: {error}: {line}"))
        })?;
        let TestMessage::Test(event) = message else {
            continue;
        };
        let result = match &event {
            TestEvent::Started { .. } => continue,
            TestEvent::Ok { .. } => "ok",
            TestEvent::Failed { reason, .. } if reason.as_deref() == Some(TIME_LIMIT_EXCEEDED) => {
                "hung"
            }
            TestEvent::Failed { .. } => "failed",
            TestEvent::Ignored { .. } => "ignored",
            TestEvent::Timeout { .. } => "hung",
        };
        events.insert(event.name().to_owned(), result);
    }
    Ok(events)
}
