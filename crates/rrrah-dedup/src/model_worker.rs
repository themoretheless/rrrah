//! Versioned binary proposals protocol for an explicitly selected LoFTR worker.
//!
//! Parent-side lifecycle, geometry and original-pixel confirmation remain in
//! `model_compare`. This provider moves only inference out of process. One child
//! is started per call; it does not reuse a model across a 25-bank search. On
//! Linux/macOS the provider signals the owned process group before reaping its
//! leader. Escaped descendants and hard RSS/resource containment remain outside
//! this protocol.
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
use crate::worker::run_worker;
#[cfg(any(target_os = "linux", target_os = "macos"))]
use crate::worker::run_worker_process_group as run_worker;
use crate::{
    model_input::GrayModelInput,
    proposals::ScoredProposal,
    worker::{WorkerError, WorkerLimits},
};
use rrrah_core::{BufferError, MemoryBudget, SharedBuffer};
use std::{
    io::{self, Cursor, Read, Write},
    path::Path,
    process::Command,
};
const REQUEST: &[u8; 8] = b"RRMOD001";
const RESPONSE: &[u8; 8] = b"RRPTS001";
const HEADER: usize = 48;
const POINT: usize = 40;
#[derive(Debug, Clone, Copy)]
pub struct ModelWorkerPolicy {
    pub process: WorkerLimits,
    pub max_input_pixels: usize,
    pub max_proposals: usize,
    pub threads: i32,
    pub seed: i64,
}
#[derive(Debug, thiserror::Error)]
pub enum ModelWorkerError {
    #[error("invalid model worker policy or input")]
    Policy,
    #[error("invalid model worker protocol: {0}")]
    Protocol(&'static str),
    #[error("model worker operation cancelled")]
    Cancelled,
    #[error(transparent)]
    Budget(#[from] BufferError),
    #[error(transparent)]
    Worker(#[from] WorkerError),
    #[error("worker failed: {status}")]
    WorkerFailed {
        status: std::process::ExitStatus,
        /// Retained diagnostics keep their conservative capture reservation.
        stderr: SharedBuffer<u8>,
    },
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Provider(#[from] crate::model_compare::ProposalProviderError),
}
fn check(cancel: &dyn Fn() -> bool) -> Result<(), ModelWorkerError> {
    if cancel() {
        Err(ModelWorkerError::Cancelled)
    } else {
        Ok(())
    }
}
fn pixels(dimensions: [u32; 2], original: [u32; 2], maximum: usize) -> Result<usize, ModelWorkerError> {
    if dimensions.iter().any(|n| *n < 8 || n % 8 != 0) || original.contains(&0) {
        return Err(ModelWorkerError::Policy);
    }
    let n = usize::try_from(dimensions[0])
        .ok()
        .and_then(|w| usize::try_from(dimensions[1]).ok().and_then(|h| w.checked_mul(h)))
        .ok_or(ModelWorkerError::Policy)?;
    if n > maximum {
        return Err(ModelWorkerError::Policy);
    }
    Ok(n)
}
impl ModelWorkerPolicy {
    /// Validate process/payload limits before allocation or worker launch.
    pub fn validate(self) -> Result<(), ModelWorkerError> {
        let response = self
            .max_proposals
            .checked_mul(POINT)
            .and_then(|n| n.checked_add(16))
            .ok_or(ModelWorkerError::Policy)?;
        let p = self.process;
        if self.max_input_pixels == 0
            || self.threads <= 0
            || p.max_input_bytes < HEADER
            || p.max_stdout_bytes < response
            || p.max_stderr_bytes == 0
            || p.deadline.is_zero()
            || p.poll_interval.is_zero()
            || p.poll_interval > p.deadline
        {
            return Err(ModelWorkerError::Policy);
        }
        Ok(())
    }
}
fn encode_request(
    left: &GrayModelInput,
    right: &GrayModelInput,
    policy: ModelWorkerPolicy,
    budget: &MemoryBudget,
    cancel: &dyn Fn() -> bool,
) -> Result<SharedBuffer<u8>, ModelWorkerError> {
    policy.validate()?;
    check(cancel)?;
    let a = pixels(left.dimensions, left.original, policy.max_input_pixels)?;
    let b = pixels(right.dimensions, right.original, policy.max_input_pixels)?;
    if left.pixels.len() != a || right.pixels.len() != b {
        return Err(ModelWorkerError::Policy);
    }
    let length = HEADER
        .checked_add(a)
        .and_then(|n| n.checked_add(b))
        .ok_or(ModelWorkerError::Policy)?;
    if length > policy.process.max_input_bytes {
        return Err(ModelWorkerError::Policy);
    }
    let mut output = budget.try_buffer(length, 0u8)?;
    let mut writer = Cursor::new(&mut output[..]);
    writer.write_all(REQUEST)?;
    writer.write_all(
        &u64::try_from(policy.max_proposals)
            .map_err(|_| ModelWorkerError::Policy)?
            .to_le_bytes(),
    )?;
    for input in [left, right] {
        for n in input.dimensions.into_iter().chain(input.original) {
            writer.write_all(&n.to_le_bytes())?;
        }
    }
    for input in [left, right] {
        for chunk in input.pixels.chunks(65536) {
            check(cancel)?;
            writer.write_all(chunk)?;
        }
    }
    check(cancel)?;
    Ok(output.freeze())
}
fn valid(point: &ScoredProposal) -> bool {
    point.source.into_iter().chain(point.target).all(f64::is_finite)
        && point.confidence.is_finite()
        && (0.0..=1.0).contains(&point.confidence)
}
fn decode_response(
    bytes: &[u8],
    maximum: usize,
    budget: &MemoryBudget,
    cancel: &dyn Fn() -> bool,
) -> Result<SharedBuffer<ScoredProposal>, ModelWorkerError> {
    check(cancel)?;
    if bytes.len() < 16 || &bytes[..8] != RESPONSE {
        return Err(ModelWorkerError::Protocol("response header"));
    }
    let count = usize::try_from(u64::from_le_bytes(bytes[8..16].try_into().unwrap()))
        .map_err(|_| ModelWorkerError::Protocol("response count"))?;
    if count > maximum || count.checked_mul(POINT).and_then(|n| n.checked_add(16)) != Some(bytes.len()) {
        return Err(ModelWorkerError::Protocol("response size/count"));
    }
    let mut output = budget.try_buffer(
        count,
        ScoredProposal {
            source: [0.; 2],
            target: [0.; 2],
            confidence: 0.,
        },
    )?;
    for (point, row) in output.iter_mut().zip(bytes[16..].chunks_exact(POINT)) {
        check(cancel)?;
        let mut values = [0.; 5];
        for (value, raw) in values.iter_mut().zip(row.chunks_exact(8)) {
            *value = f64::from_bits(u64::from_le_bytes(raw.try_into().unwrap()));
        }
        *point = ScoredProposal {
            source: [values[0], values[1]],
            target: [values[2], values[3]],
            confidence: values[4],
        };
        if !valid(point) {
            return Err(ModelWorkerError::Protocol(
                "nonfinite coordinates or invalid confidence",
            ));
        }
    }
    check(cancel)?;
    Ok(output.freeze())
}
/// Start a compatible `rrrah-loftr-worker` executable with this invocation's
/// weights and exact prepared grayscale inputs. Returned proposals are managed;
/// framed input and worst-case captured bytes are admitted against `budget`.
/// The caller selects and trusts the executable and its runtime environment.
/// Existing supplied-provider APIs retain input/weight/generation guards and
/// must still perform geometry and original-pixel confirmation.
///
/// # Errors
/// Invalid input/policy, admission, cancellation, child deadline/exit/capture
/// refusal or malformed response. No execution failure becomes an empty bank.
#[allow(clippy::too_many_arguments)]
pub fn proposals_from_worker(
    executable: &Path,
    weights: &Path,
    left: &GrayModelInput,
    right: &GrayModelInput,
    policy: ModelWorkerPolicy,
    budget: &MemoryBudget,
    cancel: &dyn Fn() -> bool,
    configure: impl FnOnce(&mut Command),
) -> Result<SharedBuffer<ScoredProposal>, ModelWorkerError> {
    let request = encode_request(left, right, policy, budget, cancel)?;
    let captures = policy
        .process
        .max_stdout_bytes
        .checked_add(policy.process.max_stderr_bytes)
        .ok_or(ModelWorkerError::Policy)?;
    let capture_credit =
        budget.try_reserve(u64::try_from(captures).map_err(|_| ModelWorkerError::Policy)?)?;
    let mut command = Command::new(executable);
    command
        .arg(weights)
        .arg(policy.max_input_pixels.to_string())
        .arg(policy.max_proposals.to_string())
        .arg(policy.threads.to_string())
        .arg(policy.seed.to_string());
    configure(&mut command);
    let output = match run_worker(command, &request, policy.process, cancel) {
        Ok(output) => output,
        Err(WorkerError::Failed { status, stderr }) => {
            return Err(ModelWorkerError::WorkerFailed {
                status,
                stderr: capture_credit.try_adopt(stderr)?,
            });
        }
        Err(error) => return Err(error.into()),
    };
    check(cancel)?;
    decode_response(&output.stdout, policy.max_proposals, budget, cancel)
}
fn read_payload(
    reader: &mut impl Read,
    output: &mut [u8],
    cancel: &dyn Fn() -> bool,
) -> Result<(), ModelWorkerError> {
    for chunk in output.chunks_mut(65536) {
        check(cancel)?;
        reader.read_exact(chunk)?;
    }
    check(cancel)
}
/// Serve one bounded framed request, useful for compatible native providers.
/// Input is validated before invoking the provider; output count/numerics are
/// validated before writing. An error requires a failed process exit so the
/// supervisor discards any partial output. The transport must provide EOF.
///
/// # Errors
/// Framing/input/configuration, admission, cancellation, provider or IO errors.
#[allow(clippy::too_many_arguments)]
pub fn serve_model_worker_once(
    reader: &mut impl Read,
    writer: &mut impl Write,
    weights: &Path,
    max_input_pixels: usize,
    max_proposals: usize,
    budget: &MemoryBudget,
    cancel: &dyn Fn() -> bool,
    mut provider: impl FnMut(
        &Path,
        &GrayModelInput,
        &GrayModelInput,
        &MemoryBudget,
        &dyn Fn() -> bool,
    )
        -> Result<SharedBuffer<ScoredProposal>, crate::model_compare::ProposalProviderError>,
) -> Result<(), ModelWorkerError> {
    check(cancel)?;
    if max_input_pixels == 0 {
        return Err(ModelWorkerError::Policy);
    }
    let mut header = [0u8; HEADER];
    reader.read_exact(&mut header)?;
    if &header[..8] != REQUEST
        || u64::from_le_bytes(header[8..16].try_into().unwrap())
            != u64::try_from(max_proposals).map_err(|_| ModelWorkerError::Policy)?
    {
        return Err(ModelWorkerError::Protocol("request header/configuration"));
    }
    let mut fields = [0u32; 8];
    for (n, b) in fields.iter_mut().zip(header[16..].chunks_exact(4)) {
        *n = u32::from_le_bytes(b.try_into().unwrap());
    }
    let dimensions = [[fields[0], fields[1]], [fields[4], fields[5]]];
    let original = [[fields[2], fields[3]], [fields[6], fields[7]]];
    let a = pixels(dimensions[0], original[0], max_input_pixels)?;
    let b = pixels(dimensions[1], original[1], max_input_pixels)?;
    let mut left = budget.try_buffer(a, 0u8)?;
    let mut right = budget.try_buffer(b, 0u8)?;
    read_payload(reader, &mut left, cancel)?;
    read_payload(reader, &mut right, cancel)?;
    let mut trailing = [0u8; 1];
    if reader.read(&mut trailing)? != 0 {
        return Err(ModelWorkerError::Protocol("trailing request bytes"));
    }
    let left = GrayModelInput {
        dimensions: dimensions[0],
        original: original[0],
        pixels: left.freeze(),
    };
    let right = GrayModelInput {
        dimensions: dimensions[1],
        original: original[1],
        pixels: right.freeze(),
    };
    check(cancel)?;
    let proposals = provider(weights, &left, &right, budget, cancel)?;
    check(cancel)?;
    if proposals.len() > max_proposals {
        return Err(ModelWorkerError::Protocol("provider proposal count"));
    }
    for point in proposals.iter() {
        check(cancel)?;
        if !valid(point) {
            return Err(ModelWorkerError::Protocol("provider proposal numeric domain"));
        }
    }
    writer.write_all(RESPONSE)?;
    writer.write_all(
        &u64::try_from(proposals.len())
            .map_err(|_| ModelWorkerError::Policy)?
            .to_le_bytes(),
    )?;
    for point in proposals.iter() {
        check(cancel)?;
        for value in point
            .source
            .into_iter()
            .chain(point.target)
            .chain([point.confidence])
        {
            writer.write_all(&value.to_bits().to_le_bytes())?;
        }
    }
    writer.flush()?;
    check(cancel)
}
/// Actual CPU LoFTR implementation of the worker protocol. Inference stays in
/// this process, which the parent supervisor can terminate. Model/Torch memory
/// is outside `budget`; process-level memory enforcement is a separate layer.
#[cfg(feature = "loftr")]
#[allow(clippy::too_many_arguments)]
pub fn serve_loftr_worker_once(
    reader: &mut impl Read,
    writer: &mut impl Write,
    weights: &Path,
    max_input_pixels: usize,
    max_proposals: usize,
    budget: &MemoryBudget,
    cancel: &dyn Fn() -> bool,
) -> Result<(), ModelWorkerError> {
    serve_model_worker_once(
        reader,
        writer,
        weights,
        max_input_pixels,
        max_proposals,
        budget,
        cancel,
        |weights, left, right, budget, cancel| {
            crate::model_loftr::proposals(weights, left, right, max_proposals, budget, cancel)
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    fn linux_stat_running(stat: &str) -> io::Result<bool> {
        let state = stat
            .rsplit_once(") ")
            .and_then(|(_, fields)| fields.as_bytes().first())
            .ok_or_else(|| io::Error::other("malformed owned process status"))?;
        Ok(*state != b'Z')
    }
    #[test]
    fn linux_stat_parser_preserves_command_parentheses_and_rejects_missing_state() {
        for (stat, running) in [
            ("42 (child worker) S 1 2", true),
            ("42 (child worker) Z 1 2", false),
            ("42 (name) Z inside) S 1 2", true),
            ("42 (name) S inside) Z 1 2", false),
        ] {
            assert_eq!(linux_stat_running(stat).unwrap(), running, "{stat}");
        }
        for stat in ["", "42 child S 1 2", "42 (child) "] {
            assert!(linux_stat_running(stat).is_err(), "{stat}");
        }
    }
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    fn owned_pid_running(pid: rustix::process::Pid) -> io::Result<bool> {
        #[cfg(target_os = "macos")]
        {
            use libproc::{bsd_info::BSDInfo, proc_pid::pidinfo};
            match pidinfo::<BSDInfo>(pid.as_raw_nonzero().get(), 0) {
                Ok(info) => Ok(info.pbi_status != libc::SZOMB),
                Err(error) => match rustix::process::getpgid(Some(pid)) {
                    Err(rustix::io::Errno::SRCH) => Ok(false),
                    _ => Err(io::Error::other(error)),
                },
            }
        }
        #[cfg(target_os = "linux")]
        {
            match std::fs::read_to_string(format!("/proc/{}/stat", pid.as_raw_nonzero())) {
                Ok(stat) => linux_stat_running(&stat),
                Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
                Err(error) => Err(error),
            }
        }
    }
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn native_owned_pid_observation_detects_live_and_reaped_worker() {
        let mut child = Command::new("/bin/sleep").arg("10").spawn().unwrap();
        let pid = rustix::process::Pid::from_raw(i32::try_from(child.id()).unwrap()).unwrap();
        let live = owned_pid_running(pid);
        child.kill().unwrap();
        child.wait().unwrap();
        assert!(live.unwrap(), "live owned worker was not observed");
        assert!(
            !owned_pid_running(pid).unwrap(),
            "reaped owned worker remains live"
        );
    }
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn successful_worker_response_does_not_leave_background_process() {
        let directory = tempfile::tempdir().unwrap();
        let pid_file = directory.path().join("pid");
        let budget = MemoryBudget::new(8192);
        let input = inputs(&budget);
        let before = budget.used();
        let result = proposals_from_worker(
            Path::new("/bin/sh"), Path::new("weights"), &input[0], &input[1],
            policy(), &budget, &|| false, |command| {
                *command = Command::new("/bin/sh");
                command.arg("-c").arg(r#"sleep 3 & printf '%s' "$!" > "$RRRAH_GROUP_CHILD_PID"; printf 'RRPTS001\000\000\000\000\000\000\000\000'"#)
                    .env("RRRAH_GROUP_CHILD_PID", &pid_file);
            },
        ).unwrap();
        assert!(result.is_empty());
        drop(result);
        assert_eq!(budget.used(), before);
        let pid = std::fs::read_to_string(pid_file).unwrap();
        let pid = rustix::process::Pid::from_raw(pid.trim().parse().unwrap()).unwrap();
        let until = std::time::Instant::now() + Duration::from_millis(500);
        loop {
            if !owned_pid_running(pid).unwrap() {
                break;
            }
            assert!(
                std::time::Instant::now() < until,
                "successful model worker left descendant running: {pid:?}"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    fn policy() -> ModelWorkerPolicy {
        ModelWorkerPolicy {
            process: WorkerLimits {
                deadline: Duration::from_secs(5),
                poll_interval: Duration::from_millis(2),
                max_input_bytes: 1024,
                max_stdout_bytes: 1024,
                max_stderr_bytes: 1024,
            },
            max_input_pixels: 64,
            max_proposals: 2,
            threads: 2,
            seed: 173,
        }
    }
    fn inputs(budget: &MemoryBudget) -> [GrayModelInput; 2] {
        std::array::from_fn(|side| {
            let mut pixels = budget.try_buffer(64, 0u8).unwrap();
            for (i, v) in pixels.iter_mut().enumerate() {
                *v = if side == 0 { i as u8 } else { 255 - i as u8 };
            }
            GrayModelInput {
                dimensions: [8, 8],
                original: if side == 0 { [11, 13] } else { [17, 19] },
                pixels: pixels.freeze(),
            }
        })
    }
    fn framed(points: &[ScoredProposal]) -> Vec<u8> {
        let mut output = Vec::from(&RESPONSE[..]);
        output.extend_from_slice(&(points.len() as u64).to_le_bytes());
        for p in points {
            for n in p.source.into_iter().chain(p.target).chain([p.confidence]) {
                output.extend_from_slice(&n.to_bits().to_le_bytes());
            }
        }
        output
    }
    #[test]
    fn binary_roundtrip_preserves_pixels_crop_domains_and_proposal_bits() {
        let budget = MemoryBudget::new(8192);
        {
            let input = inputs(&budget);
            let request = encode_request(&input[0], &input[1], policy(), &budget, &|| false).unwrap();
            let mut response = Vec::new();
            serve_model_worker_once(
                &mut Cursor::new(&request[..]),
                &mut response,
                Path::new("weights"),
                64,
                2,
                &budget,
                &|| false,
                |weights, left, right, budget, _| {
                    assert_eq!(weights, Path::new("weights"));
                    assert_eq!(left.original, [11, 13]);
                    assert_eq!(right.original, [17, 19]);
                    assert_eq!(&left.pixels[..], &input[0].pixels[..]);
                    assert_eq!(&right.pixels[..], &input[1].pixels[..]);
                    Ok(budget
                        .try_buffer(
                            1,
                            ScoredProposal {
                                source: [-0.0, 1.25],
                                target: [2.5, 3.75],
                                confidence: f64::from(0.75_f32),
                            },
                        )?
                        .freeze())
                },
            )
            .unwrap();
            let decoded = decode_response(&response, 2, &budget, &|| false).unwrap();
            assert_eq!(decoded.len(), 1);
            assert_eq!(decoded[0].source[0].to_bits(), (-0.0_f64).to_bits());
            assert_eq!(decoded[0].confidence.to_bits(), f64::from(0.75_f32).to_bits());
            assert_eq!(decoded[0].target, [2.5, 3.75]);
        }
        assert_eq!(budget.used(), 0);
    }
    #[test]
    fn empty_provider_is_a_valid_empty_bank_not_an_execution_failure() {
        let budget = MemoryBudget::new(8192);
        {
            let input = inputs(&budget);
            let request = encode_request(&input[0], &input[1], policy(), &budget, &|| false).unwrap();
            let mut response = Vec::new();
            serve_model_worker_once(
                &mut Cursor::new(&request[..]),
                &mut response,
                Path::new("weights"),
                64,
                2,
                &budget,
                &|| false,
                |_, _, _, b, _| {
                    Ok(b.try_buffer(
                        0,
                        ScoredProposal {
                            source: [0.; 2],
                            target: [0.; 2],
                            confidence: 0.,
                        },
                    )?
                    .freeze())
                },
            )
            .unwrap();
            assert!(
                decode_response(&response, 2, &budget, &|| false)
                    .unwrap()
                    .is_empty()
            );
        }
        assert_eq!(budget.used(), 0);
    }
    #[test]
    fn malformed_requests_never_invoke_provider_and_release_credit() {
        let budget = MemoryBudget::new(8192);
        let input = inputs(&budget);
        let good = encode_request(&input[0], &input[1], policy(), &budget, &|| false).unwrap();
        let before = budget.used();
        for case in 0..6 {
            let mut request = good.to_vec();
            match case {
                0 => request[0] ^= 1,
                1 => request[8..16].copy_from_slice(&3u64.to_le_bytes()),
                2 => request[16..20].copy_from_slice(&9u32.to_le_bytes()),
                3 => {
                    request.pop();
                }
                4 => request.push(42),
                _ => {
                    request.truncate(20);
                }
            }
            let mut response = Vec::new();
            let result = serve_model_worker_once(
                &mut Cursor::new(request),
                &mut response,
                Path::new("weights"),
                64,
                2,
                &budget,
                &|| false,
                |_, _, _, _, _| panic!("malformed request reached provider"),
            );
            assert!(result.is_err());
            assert!(response.is_empty());
            assert_eq!(budget.used(), before);
        }
    }
    #[test]
    fn malformed_response_count_truncation_and_trailing_bytes_are_not_empty_banks() {
        let budget = MemoryBudget::new(8192);
        let good = framed(&[ScoredProposal {
            source: [1., 2.],
            target: [3., 4.],
            confidence: 0.5,
        }]);
        for case in 0..5 {
            let mut bytes = good.clone();
            match case {
                0 => bytes[0] ^= 1,
                1 => bytes[8..16].copy_from_slice(&u64::MAX.to_le_bytes()),
                2 => {
                    bytes.pop();
                }
                3 => bytes.push(42),
                _ => bytes.truncate(8),
            }
            assert!(matches!(
                decode_response(&bytes, 2, &budget, &|| false),
                Err(ModelWorkerError::Protocol(_))
            ));
            assert_eq!(budget.used(), 0);
        }
    }
    #[test]
    fn nonfinite_coordinates_and_invalid_confidence_discard_partial_response() {
        let budget = MemoryBudget::new(8192);
        for case in 0..5 {
            let mut bad = ScoredProposal {
                source: [1., 2.],
                target: [3., 4.],
                confidence: 0.5,
            };
            match case {
                0 => bad.source[0] = f64::NAN,
                1 => bad.target[1] = f64::INFINITY,
                2 => bad.confidence = f64::NAN,
                3 => bad.confidence = -0.1,
                _ => bad.confidence = 1.1,
            }
            let good = ScoredProposal {
                source: [0.; 2],
                target: [0.; 2],
                confidence: 1.,
            };
            assert!(decode_response(&framed(&[good, bad]), 2, &budget, &|| false).is_err());
            assert_eq!(budget.used(), 0);
        }
    }
    #[test]
    fn invalid_input_policy_and_low_credit_refuse_before_worker_io() {
        let source = MemoryBudget::new(8192);
        let mut input = inputs(&source);
        let budget = MemoryBudget::new(0);
        let missing = Path::new("missing-worker-executable");
        let weights = Path::new("missing-weights");
        for case in 0..4 {
            let mut p = policy();
            match case {
                0 => p.threads = 0,
                1 => p.process.max_stdout_bytes = 16,
                2 => p.process.max_input_bytes = 48,
                _ => p.max_input_pixels = 63,
            }
            assert!(matches!(
                proposals_from_worker(
                    missing,
                    weights,
                    &input[0],
                    &input[1],
                    p,
                    &budget,
                    &|| false,
                    |_| {}
                ),
                Err(ModelWorkerError::Policy)
            ));
            assert_eq!(budget.used(), 0);
        }
        input[0].dimensions = [8, 16];
        assert!(matches!(
            proposals_from_worker(
                missing,
                weights,
                &input[0],
                &input[1],
                policy(),
                &budget,
                &|| false,
                |_| {}
            ),
            Err(ModelWorkerError::Policy)
        ));
        input[0].dimensions = [8, 8];
        assert!(matches!(
            proposals_from_worker(
                missing,
                weights,
                &input[0],
                &input[1],
                policy(),
                &budget,
                &|| false,
                |_| {}
            ),
            Err(ModelWorkerError::Budget(_))
        ));
        assert_eq!(budget.used(), 0);
    }
    #[test]
    fn response_budget_refusal_and_cancellation_release_all_credit() {
        let bytes = framed(&[ScoredProposal {
            source: [1., 2.],
            target: [3., 4.],
            confidence: 0.5,
        }]);
        let budget = MemoryBudget::new(0);
        assert!(matches!(
            decode_response(&bytes, 2, &budget, &|| false),
            Err(ModelWorkerError::Budget(_))
        ));
        assert!(matches!(
            decode_response(&bytes, 2, &budget, &|| true),
            Err(ModelWorkerError::Cancelled)
        ));
        assert_eq!(budget.used(), 0);
    }
    #[test]
    fn bad_provider_results_never_write_a_response_header() {
        let budget = MemoryBudget::new(8192);
        let input = inputs(&budget);
        let request = encode_request(&input[0], &input[1], policy(), &budget, &|| false).unwrap();
        let before = budget.used();
        for case in 0..3 {
            let mut response = Vec::new();
            let result = serve_model_worker_once(
                &mut Cursor::new(&request[..]),
                &mut response,
                Path::new("weights"),
                64,
                2,
                &budget,
                &|| false,
                |_, _, _, b, _| {
                    if case == 2 {
                        return Err(std::io::Error::other("provider failure").into());
                    }
                    Ok(b.try_buffer(
                        if case == 0 { 3 } else { 1 },
                        ScoredProposal {
                            source: [0.; 2],
                            target: [0.; 2],
                            confidence: if case == 1 { 2.0 } else { 0.5 },
                        },
                    )?
                    .freeze())
                },
            );
            assert!(result.is_err());
            assert!(response.is_empty());
            assert_eq!(budget.used(), before);
        }
    }
    #[cfg(unix)]
    #[test]
    fn retained_failed_child_diagnostics_keep_capture_credit_until_dropped() {
        let budget = MemoryBudget::new(8192);
        let input = inputs(&budget);
        let before = budget.used();
        let error = proposals_from_worker(
            Path::new("/bin/sh"),
            Path::new("weights"),
            &input[0],
            &input[1],
            policy(),
            &budget,
            &|| false,
            |command| {
                *command = Command::new("/bin/sh");
                command.arg("-c").arg("printf diagnostic >&2; exit 7");
            },
        )
        .unwrap_err();
        assert!(
            budget.used() > before,
            "retained diagnostics escaped their reservation"
        );
        let retained = match &error {
            ModelWorkerError::WorkerFailed { status, stderr } => {
                assert_eq!(status.code(), Some(7));
                assert_eq!(&stderr[..], b"diagnostic");
                stderr.clone()
            }
            other => panic!("unexpected worker failure: {other:?}"),
        };
        drop(error);
        assert!(budget.used() > before);
        drop(retained);
        assert_eq!(budget.used(), before);
        drop(input);
        assert_eq!(budget.used(), 0);
    }
    #[cfg(unix)]
    #[test]
    fn successful_child_with_no_protocol_is_an_error_not_an_empty_bank() {
        let budget = MemoryBudget::new(8192);
        let input = inputs(&budget);
        let before = budget.used();
        assert!(matches!(
            proposals_from_worker(
                Path::new("/usr/bin/true"),
                Path::new("weights"),
                &input[0],
                &input[1],
                policy(),
                &budget,
                &|| false,
                |_| {}
            ),
            Err(ModelWorkerError::Protocol(_))
        ));
        assert_eq!(budget.used(), before);
    }
}
