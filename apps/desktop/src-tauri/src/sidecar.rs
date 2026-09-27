//! Sidecar supervision.
//!
//! Two guarantees, both bought with evidence:
//!
//! 1. **No orphans.** The sidecar is assigned to a Windows Job Object created
//!    with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`. When this process dies — even
//!    by hard kill, even without running any cleanup — Windows terminates every
//!    process still in the job. Spike 001 could not measure this from inside
//!    the agent's own process tree; here it is built deliberately.
//!
//! 2. **Graceful stop before force.** Spike 001 measured that `Child::kill()`
//!    on Windows is `TerminateProcess`: no handler runs in the child. So the
//!    sidecar is asked to stop over its stdin, given a bounded grace period,
//!    and only then killed.

use std::io::{BufRead, BufReader, Write};
use std::os::windows::io::AsRawHandle;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use windows::core::PCWSTR;
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
    SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
    JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
};

/// An owned Job Object. Dropping it closes the handle, which — because of
/// `KILL_ON_JOB_CLOSE` — terminates any process still assigned to it.
pub struct Job(HANDLE);

// A HANDLE is an owned kernel object reference; moving it across threads is
// sound. There is no shared mutable state behind it.
unsafe impl Send for Job {}
unsafe impl Sync for Job {}

impl Job {
    pub fn kill_on_close() -> std::io::Result<Self> {
        unsafe {
            let handle = CreateJobObjectW(None, PCWSTR::null())
                .map_err(|e| std::io::Error::other(format!("CreateJobObjectW: {e}")))?;

            let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;

            SetInformationJobObject(
                handle,
                JobObjectExtendedLimitInformation,
                &info as *const _ as *const core::ffi::c_void,
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
            .map_err(|e| {
                let _ = CloseHandle(handle);
                std::io::Error::other(format!("SetInformationJobObject: {e}"))
            })?;

            Ok(Job(handle))
        }
    }

    pub fn assign(&self, child: &Child) -> std::io::Result<()> {
        unsafe {
            let process = HANDLE(child.as_raw_handle());
            AssignProcessToJobObject(self.0, process)
                .map_err(|e| std::io::Error::other(format!("AssignProcessToJobObject: {e}")))
        }
    }
}

impl Drop for Job {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

/// How a stop request resolved.
#[derive(Debug)]
pub struct StopOutcome {
    /// The shutdown request reached the sidecar's stdin.
    pub requested: bool,
    /// The process exited on its own within the grace period.
    pub graceful: bool,
    /// The process had to be terminated.
    pub forced: bool,
    pub exit_code: Option<i32>,
    pub waited_ms: u128,
}

pub struct Sidecar {
    child: Child,
    /// `None` when the job could not be created — the reason is preserved so
    /// the UI can say so instead of silently claiming a guarantee it lacks.
    /// Held only for its RAII effect: dropping this closes the job handle, which
    /// terminates any process still assigned to it. Never read directly, and
    /// that is the point — the guarantee is the drop, not a call.
    #[allow(dead_code)]
    job: Option<Job>,
    job_error: Option<String>,
    assigned: bool,
    exited: bool,
    /// Protocol lines from the sidecar, delivered by a reader thread.
    rx: Receiver<String>,
}

impl Sidecar {
    pub fn spawn(node: &Path, script: &Path, state_dir: &Path) -> std::io::Result<Self> {
        // The job is created BEFORE the process, so there is no window in which
        // the sidecar exists outside it.
        let (job, mut job_error) = match Job::kill_on_close() {
            Ok(j) => (Some(j), None),
            Err(e) => (None, Some(e.to_string())),
        };

        let mut child = Command::new(node)
            .arg(script)
            .arg(format!("--state={}", state_dir.display()))
            .stdin(Stdio::piped())
            // Piped, NOT discarded. The sidecar speaks protocol v1 on stdout
            // (docs/protocol.md). Before M4 this was `Stdio::null()`, which
            // meant the sidecar had no way to reply at all.
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;

        // Drain stdout on its own thread and hand lines to the protocol reader.
        // The channel is unbounded deliberately: a full pipe would block the
        // child process, whereas an unconsumed channel only costs memory.
        let (tx, rx) = mpsc::channel::<String>();
        if let Some(out) = child.stdout.take() {
            std::thread::spawn(move || {
                for line in BufReader::new(out).lines() {
                    match line {
                        Ok(text) => {
                            if tx.send(text).is_err() {
                                break; // receiver gone: the shell is shutting down
                            }
                        }
                        Err(_) => break,
                    }
                }
            });
        }

        let mut assigned = false;
        if let Some(job) = &job {
            match job.assign(&child) {
                Ok(()) => assigned = true,
                Err(e) => {
                    // Nested jobs are supported on Windows 8+, but a parent job
                    // can still refuse. Report it rather than pretending.
                    job_error = Some(format!("{e} (sidecar is NOT orphan-safe)"));
                }
            }
        }

        // Keep the field only if it was usable; otherwise drop the handle so we
        // do not imply a guarantee we do not have.
        let job = if assigned { job } else { None };

        Ok(Sidecar {
            child,
            job,
            job_error,
            assigned,
            exited: false,
            rx,
        })
    }

    /// Send one protocol message. Newline-terminated, always — see
    /// `docs/protocol.md` for the framing.
    pub fn send(&mut self, message: &serde_json::Value) -> std::io::Result<()> {
        let stdin = self
            .child
            .stdin
            .as_mut()
            .ok_or_else(|| std::io::Error::other("sidecar stdin is closed"))?;
        let mut line = serde_json::to_string(message)
            .map_err(|e| std::io::Error::other(format!("serialising message: {e}")))?;
        line.push('\n');
        stdin.write_all(line.as_bytes())?;
        stdin.flush()
    }

    /// Wait for the next protocol message, or `None` on timeout.
    pub fn recv_json(&self, timeout: Duration) -> Option<serde_json::Value> {
        self.rx
            .recv_timeout(timeout)
            .ok()
            .and_then(|line| serde_json::from_str(&line).ok())
    }

    pub fn pid(&self) -> Option<u32> {
        if self.exited {
            None
        } else {
            Some(self.child.id())
        }
    }

    pub fn is_running(&mut self) -> bool {
        if self.exited {
            return false;
        }
        match self.child.try_wait() {
            Ok(Some(_)) => {
                self.exited = true;
                false
            }
            Ok(None) => true,
            Err(_) => false,
        }
    }

    pub fn job_assigned(&self) -> bool {
        self.assigned
    }

    pub fn job_error(&self) -> Option<&str> {
        self.job_error.as_deref()
    }

    /// Ask the sidecar to stop; force only if it does not.
    pub fn stop(&mut self, grace: Duration) -> StopOutcome {
        let start = Instant::now();
        if self.exited {
            return StopOutcome {
                requested: false,
                graceful: true,
                forced: false,
                exit_code: None,
                waited_ms: 0,
            };
        }

        // 1. Ask. A well-behaved sidecar exits on its own.
        let mut requested = false;
        if let Some(mut stdin) = self.child.stdin.take() {
            requested = stdin.write_all(b"{\"cmd\":\"shutdown\"}\n").is_ok();
            let _ = stdin.flush();
            // Dropping stdin closes the pipe, so a sidecar blocked on read also
            // sees EOF. Belt and braces: two independent stop signals.
        }

        // 2. Wait, bounded.
        let deadline = start + grace;
        loop {
            match self.child.try_wait() {
                Ok(Some(status)) => {
                    self.exited = true;
                    return StopOutcome {
                        requested,
                        graceful: true,
                        forced: false,
                        exit_code: status.code(),
                        waited_ms: start.elapsed().as_millis(),
                    };
                }
                Ok(None) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(25));
                }
                Ok(None) => break,
                Err(_) => break,
            }
        }

        // 3. Force. This is the fallback, not the mechanism.
        let _ = self.child.kill();
        let status = self.child.wait().ok();
        self.exited = true;
        StopOutcome {
            requested,
            graceful: false,
            forced: true,
            exit_code: status.and_then(|s| s.code()),
            waited_ms: start.elapsed().as_millis(),
        }
    }
}

impl Drop for Sidecar {
    fn drop(&mut self) {
        if !self.exited {
            let _ = self.stop(Duration::from_secs(3));
        }
        // Dropping `self.job` closes the job handle. Anything the sidecar left
        // behind dies here — this is the orphan guarantee.
    }
}
