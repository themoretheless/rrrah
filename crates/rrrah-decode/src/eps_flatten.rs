//! Bounded native cubic subdivision in the renderer's coordinate space.
use crate::{EpsPathSegment, EpsPoint};
use rrrah_core::{BufferError, MemoryBudget, SharedBuffer};

#[derive(Clone, Copy, Debug)]
pub struct EpsFlattenLimits {
    pub tolerance: f64,
    pub max_segments: usize,
    /// Shared work allowance across validation/counting and output passes.
    pub max_work: usize,
    pub max_depth: u8,
}
impl Default for EpsFlattenLimits {
    fn default() -> Self {
        Self {
            tolerance: 0.125,
            max_segments: 1_000_000,
            max_work: 4_000_000,
            max_depth: 24,
        }
    }
}
#[derive(Debug, thiserror::Error)]
pub enum EpsFlattenError {
    #[error("invalid or nonfinite flattening geometry/configuration")]
    Range,
    #[error("EPS flattening work, depth or segment limit exceeded")]
    Limit,
    #[error("EPS flattening cancelled")]
    Cancelled,
    #[error(transparent)]
    Memory(#[from] BufferError),
}
type Cubic = [EpsPoint; 4];
fn midpoint(a: EpsPoint, b: EpsPoint) -> EpsPoint {
    // Avoid overflowing a+b for large finite coordinates.
    [a[0] * 0.5 + b[0] * 0.5, a[1] * 0.5 + b[1] * 0.5]
}
fn split(p: Cubic) -> (Cubic, Cubic) {
    let a = midpoint(p[0], p[1]);
    let b = midpoint(p[1], p[2]);
    let c = midpoint(p[2], p[3]);
    let d = midpoint(a, b);
    let e = midpoint(b, c);
    let m = midpoint(d, e);
    ([p[0], a, d, m], [m, e, c, p[3]])
}
fn distance(p: EpsPoint, a: EpsPoint, b: EpsPoint) -> Result<f64, EpsFlattenError> {
    let dx = b[0] - a[0];
    let dy = b[1] - a[1];
    let length = dx.hypot(dy);
    let result = if length == 0. {
        (p[0] - a[0]).hypot(p[1] - a[1])
    } else {
        let ux = dx / length;
        let uy = dy / length;
        let projection = ((p[0] - a[0]) * ux + (p[1] - a[1]) * uy).clamp(0., length);
        (p[0] - a[0] - projection * ux).hypot(p[1] - a[1] - projection * uy)
    };
    if length.is_finite() && result.is_finite() {
        Ok(result)
    } else {
        Err(EpsFlattenError::Range)
    }
}
fn walk<F: FnMut() -> bool, E: FnMut(EpsPathSegment)>(
    path: &[EpsPathSegment],
    limits: EpsFlattenLimits,
    work: &mut usize,
    cancelled: &mut F,
    mut emit: E,
) -> Result<usize, EpsFlattenError> {
    let mut count = 0usize;
    let mut tick = || {
        if cancelled() {
            return Err(EpsFlattenError::Cancelled);
        }
        *work = work.checked_add(1).ok_or(EpsFlattenError::Limit)?;
        if *work > limits.max_work {
            return Err(EpsFlattenError::Limit);
        }
        Ok(())
    };
    let mut output = |segment| {
        count = count.checked_add(1).ok_or(EpsFlattenError::Limit)?;
        if count > limits.max_segments {
            return Err(EpsFlattenError::Limit);
        }
        emit(segment);
        Ok(())
    };
    for &segment in path {
        tick()?;
        let points = match segment {
            EpsPathSegment::Move(p) => [p; 4],
            EpsPathSegment::Line { start, end } | EpsPathSegment::Close { start, end } => {
                [start, end, start, end]
            }
            EpsPathSegment::Curve {
                start,
                control1,
                control2,
                end,
            } => [start, control1, control2, end],
        };
        if !points.iter().flatten().all(|v| v.is_finite()) {
            return Err(EpsFlattenError::Range);
        }
        if !matches!(segment, EpsPathSegment::Curve { .. }) {
            output(segment)?;
            continue;
        }
        let mut stack = [([[0.; 2]; 4], 0u8); 33];
        stack[0] = (points, 0);
        let mut pending = 1;
        while pending != 0 {
            tick()?;
            pending -= 1;
            let (p, depth) = stack[pending];
            if distance(p[1], p[0], p[3])?.max(distance(p[2], p[0], p[3])?) <= limits.tolerance {
                output(EpsPathSegment::Line {
                    start: p[0],
                    end: p[3],
                })?;
            } else {
                if depth >= limits.max_depth {
                    return Err(EpsFlattenError::Limit);
                }
                let (left, right) = split(p);
                stack[pending] = (right, depth + 1);
                stack[pending + 1] = (left, depth + 1);
                pending += 2;
            }
        }
    }
    tick()?;
    Ok(count)
}
/// Call after mapping paths into raster coordinates, so tolerance is in pixels.
/// Two passes admit exact storage; no heap working stack or recursive calls.
pub fn flatten_eps_path<F: FnMut() -> bool>(
    path: &[EpsPathSegment],
    limits: EpsFlattenLimits,
    budget: &MemoryBudget,
    mut cancelled: F,
) -> Result<SharedBuffer<EpsPathSegment>, EpsFlattenError> {
    if !limits.tolerance.is_finite() || limits.tolerance <= 0. || limits.max_depth > 32 {
        return Err(EpsFlattenError::Range);
    }
    let mut work = 0;
    let count = walk(path, limits, &mut work, &mut cancelled, |_| {})?;
    if cancelled() {
        return Err(EpsFlattenError::Cancelled);
    }
    let mut output = budget.try_buffer(count, EpsPathSegment::Move([0.; 2]))?;
    let mut at = 0;
    walk(path, limits, &mut work, &mut cancelled, |segment| {
        output[at] = segment;
        at += 1;
    })?;
    Ok(output.freeze())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn curves_preserve_topology_and_bound_dense_independent_samples() {
        for controls in [
            [[0., 0.], [0., 10.], [10., 10.], [10., 0.]],
            [[0., 0.], [10., 20.], [-10., 20.], [0., 0.]],
            [[0., 0.], [20., 0.], [-20., 0.], [1., 0.]],
        ] {
            let root = MemoryBudget::new(1_000_000);
            let path = [
                EpsPathSegment::Move(controls[0]),
                EpsPathSegment::Curve {
                    start: controls[0],
                    control1: controls[1],
                    control2: controls[2],
                    end: controls[3],
                },
                EpsPathSegment::Close {
                    start: controls[3],
                    end: controls[0],
                },
            ];
            let limits = EpsFlattenLimits::default();
            let flat = flatten_eps_path(&path, limits, &root, || false).unwrap();
            assert_eq!(flat[0], path[0]);
            assert_eq!(flat[flat.len() - 1], path[2]);
            let mut previous = controls[0];
            for segment in &flat[1..flat.len() - 1] {
                let EpsPathSegment::Line { start, end } = segment else {
                    panic!()
                };
                assert_eq!(*start, previous);
                previous = *end;
            }
            assert_eq!(previous, controls[3]);
            for sample in 0..=1000 {
                let t = sample as f64 / 1000.;
                let s = 1. - t;
                let p = std::array::from_fn(|axis| {
                    s * s * s * controls[0][axis]
                        + 3. * s * s * t * controls[1][axis]
                        + 3. * s * t * t * controls[2][axis]
                        + t * t * t * controls[3][axis]
                });
                let best = flat[1..flat.len() - 1]
                    .iter()
                    .map(|segment| match segment {
                        EpsPathSegment::Line { start, end } => distance(p, *start, *end).unwrap(),
                        _ => unreachable!(),
                    })
                    .fold(f64::INFINITY, f64::min);
                assert!(best <= limits.tolerance + 1e-12, "{best}");
            }
            assert_eq!(
                root.used(),
                (flat.len() * std::mem::size_of::<EpsPathSegment>()) as u64
            );
            drop(flat);
            assert_eq!(root.used(), 0);
        }
    }
    #[test]
    fn refusals_and_output_pass_cancellation_release_credit() {
        let path = [EpsPathSegment::Curve {
            start: [0., 0.],
            control1: [0., 10.],
            control2: [10., 10.],
            end: [10., 0.],
        }];
        let root = MemoryBudget::new(1_000_000);
        let limits = EpsFlattenLimits::default();
        for bad in [
            EpsFlattenLimits {
                max_depth: 0,
                ..limits
            },
            EpsFlattenLimits {
                max_work: 1,
                ..limits
            },
            EpsFlattenLimits {
                max_segments: 1,
                ..limits
            },
        ] {
            assert!(matches!(
                flatten_eps_path(&path, bad, &root, || false),
                Err(EpsFlattenError::Limit)
            ));
            assert_eq!(root.used(), 0);
        }
        let refused = MemoryBudget::new(1);
        assert!(matches!(
            flatten_eps_path(&path, limits, &refused, || false),
            Err(EpsFlattenError::Memory(_))
        ));
        assert_eq!(refused.peak(), 0);
        assert!(matches!(
            flatten_eps_path(&path, limits, &root, || root.used() != 0),
            Err(EpsFlattenError::Cancelled)
        ));
        assert!(root.peak() > 0);
        assert_eq!(root.used(), 0);
    }
}
