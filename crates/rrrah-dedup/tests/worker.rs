use rrrah_dedup::worker::{WorkerError, WorkerLimits, WorkerStop, run_worker};
use std::{
    io::{Read, Write},
    process::Command,
    time::Duration,
};
fn limits() -> WorkerLimits {
    WorkerLimits {
        deadline: Duration::from_secs(5),
        poll_interval: Duration::from_millis(2),
        max_input_bytes: 1024,
        max_stdout_bytes: 4096,
        max_stderr_bytes: 4096,
    }
}
fn command(mode: &str, marker: &std::path::Path) -> Command {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args(["--exact", "worker_fixture", "--nocapture", "--test-threads=1"])
        .env("RRRAH_TEST_WORKER_MODE", mode)
        .env("RRRAH_TEST_WORKER_MARKER", marker);
    command
}
#[test]
fn worker_fixture() {
    let Ok(mode) = std::env::var("RRRAH_TEST_WORKER_MODE") else {
        return;
    };
    std::fs::write(
        std::env::var_os("RRRAH_TEST_WORKER_MARKER").unwrap(),
        std::process::id().to_string(),
    )
    .unwrap();
    match mode.as_str() {
        "echo" => {
            let mut data = Vec::new();
            std::io::stdin().read_to_end(&mut data).unwrap();
            std::io::stdout().write_all(&data).unwrap();
        }
        "block" => loop {
            std::thread::sleep(Duration::from_secs(60));
        },
        "stdout" | "stderr" => loop {
            let bytes = [42_u8; 16384];
            if mode == "stdout" {
                std::io::stdout().write_all(&bytes).unwrap();
            } else {
                std::io::stderr().write_all(&bytes).unwrap();
            }
        },
        "fail" => {
            std::io::stderr().write_all(b"failure\0\xff").unwrap();
            std::process::exit(7);
        }
        _ => panic!("invalid fixture mode"),
    }
}
#[test]
fn binary_input_and_output_survive_private_stdio_files() {
    let dir = tempfile::tempdir().unwrap();
    let input = b"binary input\0\xff\x01";
    let output = run_worker(
        command("echo", &dir.path().join("started")),
        input,
        limits(),
        || false,
    )
    .unwrap();
    assert!(output.status.success());
    assert!(output.stdout.windows(input.len()).any(|v| v == input));
    assert!(output.stderr.is_empty());
}
#[test]
fn invalid_policy_input_and_initial_cancellation_never_spawn() {
    let dir = tempfile::tempdir().unwrap();
    let marker = dir.path().join("started");
    let mut policy = limits();
    policy.poll_interval = Duration::ZERO;
    assert!(matches!(
        run_worker(command("echo", &marker), b"", policy, || false),
        Err(WorkerError::Policy)
    ));
    policy = limits();
    policy.max_input_bytes = 2;
    assert!(matches!(
        run_worker(command("echo", &marker), b"123", policy, || false),
        Err(WorkerError::InputLimit)
    ));
    assert!(matches!(
        run_worker(command("echo", &marker), b"", limits(), || true),
        Err(WorkerError::Stopped {
            reason: WorkerStop::Cancelled,
            status: None
        })
    ));
    assert!(!marker.exists());
}
#[test]
fn deadline_terminates_and_reaps_a_blocking_worker() {
    let dir = tempfile::tempdir().unwrap();
    let mut policy = limits();
    policy.deadline = Duration::from_millis(100);
    assert!(
        matches!(run_worker(command("block",&dir.path().join("started")),b"",policy,||false),Err(WorkerError::Stopped{reason:WorkerStop::Deadline,status:Some(status)}) if !status.success())
    );
}
#[test]
fn live_cancellation_terminates_and_reaps_then_a_retry_succeeds() {
    let dir = tempfile::tempdir().unwrap();
    let marker = dir.path().join("started");
    assert!(
        matches!(run_worker(command("block",&marker),b"",limits(),||marker.exists()),Err(WorkerError::Stopped{reason:WorkerStop::Cancelled,status:Some(status)}) if !status.success())
    );
    assert!(run_worker(command("echo", &marker), b"retry", limits(), || false).is_ok());
}
#[test]
fn stdout_and_stderr_floods_stop_without_pipe_deadlock() {
    let dir = tempfile::tempdir().unwrap();
    for (mode, reason) in [
        ("stdout", WorkerStop::StdoutLimit),
        ("stderr", WorkerStop::StderrLimit),
    ] {
        assert!(
            matches!(run_worker(command(mode,&dir.path().join(mode)),b"",limits(),||false),Err(WorkerError::Stopped{reason:actual,status:Some(status)}) if actual==reason && !status.success())
        );
    }
}
#[test]
fn nonzero_exit_preserves_bounded_binary_diagnostics() {
    let dir = tempfile::tempdir().unwrap();
    let error = run_worker(
        command("fail", &dir.path().join("started")),
        b"",
        limits(),
        || false,
    )
    .unwrap_err();
    assert!(
        matches!(error,WorkerError::Failed{status,stderr} if status.code()==Some(7) && stderr==b"failure\0\xff")
    );
}
