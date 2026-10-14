//! Supervision for a caller-owned worker executable.
//!
//! Stdio uses private temporary files so a blocked pipe cannot prevent deadline
//! or cancellation checks. Capture limits bound returned allocations and trigger
//! termination when observed; output can grow between polls. This does not cap
//! RSS, disk use between polls, descendants, or kernel scheduling latency. The
//! single-process entry point requires a trusted worker without descendants.
//! Linux/macOS additionally expose explicit process-group signalling; this does
//! not contain descendants that leave the group or impose hard resource caps.
use std::{
    fs::File,
    io::{self, Read, Seek, SeekFrom, Write},
    process::{Child, Command, ExitStatus, Stdio},
    time::{Duration, Instant},
};

#[derive(Debug, Clone, Copy)]
pub struct WorkerLimits {
    pub deadline: Duration,
    pub poll_interval: Duration,
    pub max_input_bytes: usize,
    pub max_stdout_bytes: usize,
    pub max_stderr_bytes: usize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkerStop {
    Cancelled,
    Deadline,
    StdoutLimit,
    StderrLimit,
}
#[derive(Debug, thiserror::Error)]
pub enum WorkerError {
    #[error("invalid worker policy")]
    Policy,
    #[error("worker input exceeds the caller limit")]
    InputLimit,
    #[error("worker stopped: {reason:?}")]
    Stopped {
        reason: WorkerStop,
        status: Option<ExitStatus>,
    },
    #[error("worker failed: {status}")]
    Failed { status: ExitStatus, stderr: Vec<u8> },
    #[error("worker capture allocation refused")]
    Allocation,
    #[error(transparent)]
    Io(#[from] io::Error),
}
#[derive(Debug)]
pub struct WorkerOutput {
    pub status: ExitStatus,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

// Every return path after spawn owns this guard. Explicit stop observes wait's
// result; errors during capture still kill and reap through Drop.
struct OwnedChild {
    child: Child,
    reaped: bool,
    // Group supervision is available only on Linux/macOS; other targets
    // retain the common single-child guard without reading this marker.
    #[cfg_attr(not(any(target_os = "linux", target_os = "macos")), allow(dead_code))]
    scope: WorkerScope,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum WorkerScope {
    Single,
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    Group,
}
impl Drop for OwnedChild {
    fn drop(&mut self) {
        if !self.reaped {
            let _ = self.terminate();
            let _ = self.child.wait();
        }
    }
}
impl OwnedChild {
    fn terminate(&mut self) -> io::Result<()> {
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        if self.scope == WorkerScope::Group {
            use rustix::process::{Pid, Signal, kill_process_group};
            let pid = i32::try_from(self.child.id())
                .ok()
                .and_then(Pid::from_raw)
                .filter(|p| p.as_raw_nonzero().get() > 1)
                .ok_or_else(|| io::Error::other("invalid owned worker group"))?;
            return match kill_process_group(pid, Signal::KILL) {
                Ok(()) | Err(rustix::io::Errno::SRCH) => Ok(()),
                // Darwin excludes zombies from killpg's eligible members and
                // can return EPERM for a group containing only exited members.
                // Never suppress a real permission refusal with live members.
                #[cfg(target_os = "macos")]
                Err(rustix::io::Errno::PERM) if mac_group_quiescent(pid)? => Ok(()),
                Err(error) => Err(error.into()),
            };
        }
        let _ = self.child.kill();
        Ok(())
    }
    fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        if self.scope == WorkerScope::Group {
            use rustix::process::{Pid, WaitId, WaitIdOptions, waitid};
            let pid = i32::try_from(self.child.id())
                .ok()
                .and_then(Pid::from_raw)
                .ok_or_else(|| io::Error::other("invalid owned worker pid"))?;
            // Do not reap the leader before signalling its group: an unreaped
            // leader keeps its PID reserved even after normal exit. Thus group
            // cleanup cannot target a subsequently reused leader PID.
            if waitid(
                WaitId::Pid(pid),
                WaitIdOptions::EXITED | WaitIdOptions::NOHANG | WaitIdOptions::NOWAIT,
            )?
            .is_none()
            {
                return Ok(None);
            }
            self.terminate()?;
            let status = self.child.wait()?;
            self.reaped = true;
            return Ok(Some(status));
        }
        let status = self.child.try_wait()?;
        self.reaped = status.is_some();
        Ok(status)
    }
    fn stop(&mut self, reason: WorkerStop) -> WorkerError {
        // A process can exit between the preceding observation and kill. wait
        // remains required; it also reports an actual termination failure.
        if let Err(error) = self.terminate() {
            return error.into();
        }
        match self.child.wait() {
            Ok(status) => {
                self.reaped = true;
                WorkerError::Stopped {
                    reason,
                    status: Some(status),
                }
            }
            Err(error) => error.into(),
        }
    }
}

#[cfg(target_os = "macos")]
fn mac_group_quiescent(leader: rustix::process::Pid) -> io::Result<bool> {
    use libproc::{
        bsd_info::BSDInfo,
        proc_pid::pidinfo,
        processes::{ProcFilter, pids_by_type},
    };
    use rustix::process::{Pid, WaitId, WaitIdOptions, getpgid, waitid};
    if waitid(
        WaitId::Pid(leader),
        WaitIdOptions::EXITED | WaitIdOptions::NOHANG | WaitIdOptions::NOWAIT,
    )?
    .is_none()
    {
        return Ok(false);
    }
    let group = leader.as_raw_nonzero().get() as u32;
    // libproc's zero-length success/error distinction observes errno. Clear the
    // preceding killpg EPERM before making this independent query.
    errno::set_errno(errno::Errno(0));
    let members = pids_by_type(ProcFilter::ByProgramGroup { pgrpid: group })?;
    if !members.is_empty() && members.len() == members.capacity() {
        return Err(io::Error::other("worker group enumeration may be truncated"));
    }
    for member in members {
        if member == group {
            continue;
        } // Already observed exited, not reaped.
        let pid = i32::try_from(member)
            .ok()
            .and_then(Pid::from_raw)
            .ok_or_else(|| io::Error::other("invalid worker group member"))?;
        match pidinfo::<BSDInfo>(pid.as_raw_nonzero().get(), 0) {
            Ok(info) if info.pbi_pgid != group || info.pbi_status == libc::SZOMB => {}
            Ok(_) => return Ok(false),
            Err(error) => match getpgid(Some(pid)) {
                Err(rustix::io::Errno::SRCH) => {}
                Ok(current) if current != leader => {}
                _ => return Err(io::Error::other(error)),
            },
        }
    }
    Ok(true)
}

#[cfg(all(test, target_os = "macos"))]
mod group_quiescence_tests {
    use super::*;
    use rustix::process::{Pid, WaitId, WaitIdOptions, waitid};
    use std::os::unix::process::CommandExt;

    #[test]
    fn quiescence_refuses_live_leader_and_live_descendant_after_leader_exit() {
        let mut command = Command::new("/bin/sleep");
        command.arg("10").process_group(0);
        let leader = OwnedChild {
            child: command.spawn().unwrap(),
            reaped: false,
            scope: WorkerScope::Group,
        };
        let pid = Pid::from_raw(i32::try_from(leader.child.id()).unwrap()).unwrap();
        assert!(!mac_group_quiescent(pid).unwrap());
        drop(leader);

        let mut command = Command::new("/bin/sh");
        command.args(["-c", "sleep 10 & exit 0"]).process_group(0);
        let leader = OwnedChild {
            child: command.spawn().unwrap(),
            reaped: false,
            scope: WorkerScope::Group,
        };
        let pid = Pid::from_raw(i32::try_from(leader.child.id()).unwrap()).unwrap();
        let until = Instant::now() + Duration::from_secs(3);
        while waitid(
            WaitId::Pid(pid),
            WaitIdOptions::EXITED | WaitIdOptions::NOHANG | WaitIdOptions::NOWAIT,
        )
        .unwrap()
        .is_none()
        {
            assert!(Instant::now() < until);
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(!mac_group_quiescent(pid).unwrap());
        // Drop kills this still-live descendant before reaping the leader.
        drop(leader);
    }
}
fn check(cancel: &impl Fn() -> bool, deadline: Instant) -> Result<(), WorkerError> {
    let reason = if cancel() {
        Some(WorkerStop::Cancelled)
    } else if Instant::now() >= deadline {
        Some(WorkerStop::Deadline)
    } else {
        None
    };
    match reason {
        Some(reason) => Err(WorkerError::Stopped { reason, status: None }),
        None => Ok(()),
    }
}
fn excess(stdout: &File, stderr: &File, limits: WorkerLimits) -> io::Result<Option<WorkerStop>> {
    if stdout.metadata()?.len() > limits.max_stdout_bytes as u64 {
        Ok(Some(WorkerStop::StdoutLimit))
    } else if stderr.metadata()?.len() > limits.max_stderr_bytes as u64 {
        Ok(Some(WorkerStop::StderrLimit))
    } else {
        Ok(None)
    }
}
fn capture(file: &mut File, limit: usize) -> Result<Vec<u8>, WorkerError> {
    let length = usize::try_from(file.metadata()?.len()).map_err(|_| WorkerError::Allocation)?;
    if length > limit {
        return Err(WorkerError::Allocation);
    }
    let mut output = Vec::new();
    output
        .try_reserve_exact(length)
        .map_err(|_| WorkerError::Allocation)?;
    file.seek(SeekFrom::Start(0))?;
    // Even an unexpected writer cannot make the returned allocation exceed the
    // caller cap. A trusted worker has no descendants retaining this handle.
    output.resize(length, 0);
    file.read_exact(&mut output)?;
    Ok(output)
}

/// Run one executable, replacing its stdin/stdout/stderr with private files.
/// Only a successful exit returns output. Stop/failure discards stdout evidence.
/// Cancellation is checked before spawn, throughout polling and before return.
/// Every spawned child is waited on, including failed capture and cancellation.
///
/// # Errors
/// Policy/input refusal before spawn; cancellation/deadline/capture excess;
/// nonzero exit, allocation or IO failure. Timeout is an observed deadline:
/// termination/reaping and filesystem operations depend on the operating system.
pub fn run_worker(
    command: Command,
    input: &[u8],
    limits: WorkerLimits,
    cancelled: impl Fn() -> bool,
) -> Result<WorkerOutput, WorkerError> {
    run_worker_scoped(command, input, limits, cancelled, WorkerScope::Single)
}

/// Run a trusted worker in a fresh process group, terminating the group before
/// reaping its leader on stop, failure, or normal exit. Descendants that remain
/// in this group receive SIGKILL, including writers retaining capture handles.
/// Only the direct child can be reaped here; descendant exit/reaping is managed
/// by the OS or their own parents. This is not a sandbox: descendants can leave
/// the group, and RSS/disk use/scheduling remain unbounded. The caller must not
/// independently reap this owned child (including from a SIGCHLD handler).
///
/// # Errors
/// The same policy, stop, capture and IO errors as [`run_worker`], plus process
/// group signalling or non-reaping exit-observation failure. Available only on
/// Linux and macOS; other platforms retain single-process supervision.
#[cfg(any(target_os = "linux", target_os = "macos"))]
pub fn run_worker_process_group(
    command: Command,
    input: &[u8],
    limits: WorkerLimits,
    cancelled: impl Fn() -> bool,
) -> Result<WorkerOutput, WorkerError> {
    run_worker_scoped(command, input, limits, cancelled, WorkerScope::Group)
}

fn run_worker_scoped(
    mut command: Command,
    input: &[u8],
    limits: WorkerLimits,
    cancelled: impl Fn() -> bool,
    scope: WorkerScope,
) -> Result<WorkerOutput, WorkerError> {
    if limits.deadline.is_zero()
        || limits.poll_interval.is_zero()
        || limits.poll_interval > limits.deadline
        || limits.max_stdout_bytes == 0
        || limits.max_stderr_bytes == 0
    {
        return Err(WorkerError::Policy);
    }
    let deadline = Instant::now()
        .checked_add(limits.deadline)
        .ok_or(WorkerError::Policy)?;
    check(&cancelled, deadline)?;
    if input.len() > limits.max_input_bytes {
        return Err(WorkerError::InputLimit);
    }
    let mut stdin = tempfile::tempfile()?;
    for chunk in input.chunks(65536) {
        check(&cancelled, deadline)?;
        stdin.write_all(chunk)?;
    }
    stdin.seek(SeekFrom::Start(0))?;
    let mut stdout = tempfile::tempfile()?;
    let mut stderr = tempfile::tempfile()?;
    command.stdin(Stdio::from(stdin));
    command.stdout(Stdio::from(stdout.try_clone()?));
    command.stderr(Stdio::from(stderr.try_clone()?));
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    if scope == WorkerScope::Group {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    check(&cancelled, deadline)?;
    let mut child = OwnedChild {
        child: command.spawn()?,
        reaped: false,
        scope,
    };
    let status = loop {
        if let Err(error) = check(&cancelled, deadline) {
            if let WorkerError::Stopped { reason, .. } = error {
                return Err(child.stop(reason));
            }
            return Err(error);
        }
        if let Some(reason) = excess(&stdout, &stderr, limits)? {
            return Err(child.stop(reason));
        }
        if let Some(status) = child.try_wait()? {
            break status;
        }
        std::thread::sleep(
            limits
                .poll_interval
                .min(deadline.saturating_duration_since(Instant::now())),
        );
    };
    check(&cancelled, deadline)?;
    if let Some(reason) = excess(&stdout, &stderr, limits)? {
        return Err(WorkerError::Stopped {
            reason,
            status: Some(status),
        });
    }
    let stderr = capture(&mut stderr, limits.max_stderr_bytes)?;
    if !status.success() {
        check(&cancelled, deadline)?;
        return Err(WorkerError::Failed { status, stderr });
    }
    let stdout = capture(&mut stdout, limits.max_stdout_bytes)?;
    check(&cancelled, deadline)?;
    Ok(WorkerOutput {
        status,
        stdout,
        stderr,
    })
}
