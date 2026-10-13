#![cfg(any(target_os = "linux", target_os = "macos"))]
use rrrah_dedup::worker::{WorkerError, WorkerLimits, WorkerStop, run_worker_process_group};
use std::{
    path::Path,
    process::Command,
    time::{Duration, Instant},
};
fn policy() -> WorkerLimits {
    WorkerLimits {
        deadline: Duration::from_secs(5),
        poll_interval: Duration::from_millis(2),
        max_input_bytes: 128,
        max_stdout_bytes: 4096,
        max_stderr_bytes: 4096,
    }
}
fn command(mode: &str, directory: &Path) -> Command {
    let mut c = Command::new(std::env::current_exe().unwrap());
    c.args(["--exact", "group_fixture", "--nocapture", "--test-threads=1"])
        .env("RRRAH_GROUP_FIXTURE", mode)
        .env("RRRAH_GROUP_DIRECTORY", directory);
    c
}
#[test]
fn group_fixture() {
    let Ok(mode) = std::env::var("RRRAH_GROUP_FIXTURE") else {
        return;
    };
    let dir = std::path::PathBuf::from(std::env::var_os("RRRAH_GROUP_DIRECTORY").unwrap());
    if mode == "leaf" {
        std::fs::write(dir.join("leaf"), std::process::id().to_string()).unwrap();
        let until = Instant::now() + Duration::from_secs(8);
        let mut counter = 0;
        while Instant::now() < until {
            counter += 1;
            std::fs::write(dir.join("heartbeat"), counter.to_string()).unwrap();
            std::thread::sleep(Duration::from_millis(5));
        }
        return;
    }
    let leaf = command("leaf", &dir).spawn().unwrap();
    let until = Instant::now() + Duration::from_secs(3);
    while !dir.join("heartbeat").exists() {
        assert!(Instant::now() < until, "leaf did not start");
        std::thread::sleep(Duration::from_millis(2));
    }
    std::fs::write(dir.join("ready"), leaf.id().to_string()).unwrap();
    match mode.as_str() {
        "exit" => std::process::exit(0),
        "fail" => std::process::exit(7),
        "block" => {
            std::thread::sleep(Duration::from_secs(8));
        }
        "flood" | "stderr_flood" => {
            use std::io::Write;
            loop {
                if mode == "flood" {
                    std::io::stdout().write_all(&[42; 16384]).unwrap();
                } else {
                    std::io::stderr().write_all(&[42; 16384]).unwrap();
                }
            }
        }
        _ => panic!("invalid fixture"),
    }
}
fn assert_leaf_stopped(dir: &Path) {
    assert!(dir.join("ready").exists());
    let pid: i32 = std::fs::read_to_string(dir.join("leaf"))
        .unwrap()
        .parse()
        .unwrap();
    // A grandchild cannot be waited on by this process. Accept a non-running
    // zombie awaiting its OS parent, but never a live process retaining handles.
    let until = Instant::now() + Duration::from_secs(2);
    loop {
        let out = Command::new("ps")
            .args(["-o", "stat=", "-p", &pid.to_string()])
            .output()
            .unwrap();
        let state = String::from_utf8(out.stdout).unwrap();
        if state.trim().is_empty() || state.trim_start().starts_with('Z') {
            break;
        }
        assert!(Instant::now() < until, "leaf still running: {state}");
        std::thread::sleep(Duration::from_millis(5));
    }
    let before = std::fs::read(dir.join("heartbeat")).unwrap();
    std::thread::sleep(Duration::from_millis(30));
    assert_eq!(std::fs::read(dir.join("heartbeat")).unwrap(), before);
}
#[test]
fn group_cancel_stops_live_descendant_and_retry_succeeds() {
    let d = tempfile::tempdir().unwrap();
    let result = run_worker_process_group(command("block", d.path()), b"", policy(), || {
        d.path().join("ready").exists()
    });
    assert!(
        matches!(result, Err(WorkerError::Stopped{reason:WorkerStop::Cancelled,status:Some(s)}) if !s.success())
    );
    assert_leaf_stopped(d.path());
    let retry = tempfile::tempdir().unwrap();
    assert!(run_worker_process_group(command("exit", retry.path()), b"", policy(), || false).is_ok());
    assert_leaf_stopped(retry.path());
}
#[test]
fn group_deadline_stops_live_descendant() {
    let d = tempfile::tempdir().unwrap();
    let mut p = policy();
    p.deadline = Duration::from_millis(500);
    let result = run_worker_process_group(command("block", d.path()), b"", p, || false);
    assert!(
        matches!(result,Err(WorkerError::Stopped{reason:WorkerStop::Deadline,status:Some(s)}) if !s.success())
    );
    assert_leaf_stopped(d.path());
}
#[test]
fn group_normal_and_failed_leader_exit_stop_descendants() {
    for mode in ["exit", "fail"] {
        let d = tempfile::tempdir().unwrap();
        let result = run_worker_process_group(command(mode, d.path()), b"", policy(), || false);
        if mode == "exit" {
            assert!(result.unwrap().status.success());
        } else {
            assert!(matches!(result,Err(WorkerError::Failed{status,..}) if status.code()==Some(7)));
        }
        assert_leaf_stopped(d.path());
    }
}
#[test]
fn group_capture_limit_stops_descendant() {
    for (mode, reason) in [
        ("flood", WorkerStop::StdoutLimit),
        ("stderr_flood", WorkerStop::StderrLimit),
    ] {
        let d = tempfile::tempdir().unwrap();
        let result = run_worker_process_group(command(mode, d.path()), b"", policy(), || false);
        assert!(
            matches!(result,Err(WorkerError::Stopped{reason:actual,status:Some(s)}) if actual == reason && !s.success())
        );
        assert_leaf_stopped(d.path());
    }
}

#[test]
fn group_callback_panic_still_stops_descendant() {
    let d = tempfile::tempdir().unwrap();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        run_worker_process_group(command("block", d.path()), b"", policy(), || {
            assert!(!d.path().join("ready").exists(), "injected callback panic");
            false
        })
    }));
    assert!(result.is_err());
    assert_leaf_stopped(d.path());
}
#[test]
fn group_initial_refusals_do_not_spawn() {
    let d = tempfile::tempdir().unwrap();
    assert!(matches!(
        run_worker_process_group(command("exit", d.path()), b"", policy(), || true),
        Err(WorkerError::Stopped {
            reason: WorkerStop::Cancelled,
            status: None
        })
    ));
    let mut p = policy();
    p.max_input_bytes = 1;
    assert!(matches!(
        run_worker_process_group(command("exit", d.path()), b"12", p, || false),
        Err(WorkerError::InputLimit)
    ));
    assert!(!d.path().join("leaf").exists());
}

#[test]
fn group_fast_empty_exit_keeps_status_and_diagnostics() {
    for _ in 0..10 {
        let output =
            run_worker_process_group(Command::new("/usr/bin/true"), b"", policy(), || false).unwrap();
        assert!(output.status.success());
        assert!(output.stdout.is_empty() && output.stderr.is_empty());
        let mut command = Command::new("/bin/sh");
        command.args(["-c", "printf diagnostic >&2; exit 7"]);
        let result = run_worker_process_group(command, b"", policy(), || false);
        assert!(
            matches!(result,Err(WorkerError::Failed{status,stderr}) if status.code() == Some(7) && stderr == b"diagnostic")
        );
    }
}
