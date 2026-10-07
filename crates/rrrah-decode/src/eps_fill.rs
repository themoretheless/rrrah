//! Native scanline coverage for flattened EPS paths in raster coordinates.
use crate::{EpsPathSegment, EpsPoint};
use rrrah_core::{BufferError, MemoryBudget, SharedBuffer};

#[derive(Debug, Clone, Copy)]
pub struct EpsFillLimits {
    pub max_pixels: usize,
    pub max_edges: usize,
    pub max_work: usize,
    /// Samples per axis, in 1..=8.
    pub samples: u8,
}
impl Default for EpsFillLimits {
    fn default() -> Self {
        Self {
            max_pixels: 64_000_000,
            max_edges: 1_000_000,
            max_work: 100_000_000,
            samples: 4,
        }
    }
}
#[derive(Debug, thiserror::Error)]
pub enum EpsFillError {
    #[error("invalid flattened EPS path or raster dimensions")]
    Range,
    #[error("EPS fill work, edge or pixel limit exceeded")]
    Limit,
    #[error("EPS fill cancelled")]
    Cancelled,
    #[error(transparent)]
    Memory(#[from] BufferError),
}
#[derive(Clone, Copy)]
struct Edge {
    start: EpsPoint,
    end: EpsPoint,
}
#[derive(Clone, Copy)]
struct Crossing {
    x: f64,
    winding: i64,
}

fn edges<F: FnMut(Option<Edge>) -> Result<(), EpsFillError>>(
    path: &[EpsPathSegment],
    mut emit: F,
) -> Result<(), EpsFillError> {
    let mut origin = None;
    let mut current = None;
    let mut line = |start: EpsPoint, end: EpsPoint| {
        if !start.into_iter().chain(end).all(f64::is_finite) {
            return Err(EpsFillError::Range);
        }
        emit((start[1] != end[1]).then_some(Edge { start, end }))?;
        Ok(())
    };
    for &segment in path {
        match segment {
            EpsPathSegment::Move(p) => {
                if !p.into_iter().all(f64::is_finite) {
                    return Err(EpsFillError::Range);
                }
                // Even move-only/horizontal paths consume work and poll cancellation.
                line(p, p)?;
                if let (Some(a), Some(b)) = (current, origin) {
                    line(a, b)?;
                }
                origin = Some(p);
                current = Some(p);
            }
            EpsPathSegment::Line { start, end } => {
                if current != Some(start) {
                    return Err(EpsFillError::Range);
                }
                line(start, end)?;
                current = Some(end);
            }
            EpsPathSegment::Close { start, end } => {
                if current != Some(start) || origin != Some(end) {
                    return Err(EpsFillError::Range);
                }
                line(start, end)?;
                current = Some(end);
            }
            EpsPathSegment::Curve { .. } => return Err(EpsFillError::Range),
        }
    }
    if let (Some(a), Some(b)) = (current, origin) {
        line(a, b)?;
    }
    Ok(())
}
/// Returns an 8-bit coverage mask, with top-to-bottom rows. Paths must already
/// be mapped to raster coordinates and flattened. Open subpaths close for fill.
/// Every heap buffer uses the supplied root; failed output is discarded.
pub fn fill_eps_path<F: FnMut() -> bool>(
    path: &[EpsPathSegment],
    width: u32,
    height: u32,
    even_odd: bool,
    limits: EpsFillLimits,
    budget: &MemoryBudget,
    mut cancelled: F,
) -> Result<SharedBuffer<u8>, EpsFillError> {
    let pixels = (width as usize)
        .checked_mul(height as usize)
        .ok_or(EpsFillError::Limit)?;
    if width == 0 || height == 0 || !(1..=8).contains(&limits.samples) {
        return Err(EpsFillError::Range);
    }
    if pixels > limits.max_pixels {
        return Err(EpsFillError::Limit);
    }
    let mut work = 0usize;
    let mut tick = || {
        if cancelled() {
            return Err(EpsFillError::Cancelled);
        }
        work = work.checked_add(1).ok_or(EpsFillError::Limit)?;
        if work > limits.max_work {
            return Err(EpsFillError::Limit);
        }
        Ok(())
    };
    tick()?;
    let mut count = 0usize;
    edges(path, |edge| {
        tick()?;
        if edge.is_none() {
            return Ok(());
        }
        count = count.checked_add(1).ok_or(EpsFillError::Limit)?;
        if count > limits.max_edges {
            return Err(EpsFillError::Limit);
        }
        Ok(())
    })?;
    if count == 0 {
        // Geometry has been fully validated and cancellation-polled above.
        // Empty/move-only/horizontal paths cannot contribute coverage.
        tick()?;
        let output = budget.try_buffer(pixels, 0u8)?;
        tick()?;
        return Ok(output.freeze());
    }
    let mut storage = budget.try_buffer(
        count,
        Edge {
            start: [0.; 2],
            end: [0.; 2],
        },
    )?;
    let mut at = 0;
    edges(path, |edge| {
        tick()?;
        let Some(edge) = edge else {
            return Ok(());
        };
        storage[at] = edge;
        at += 1;
        Ok(())
    })?;
    sort(
        &mut storage,
        |edge: &Edge| edge.start[1].min(edge.end[1]),
        &mut tick,
    )?;
    tick()?;
    let mut crossings = budget.try_buffer(count, Crossing { x: 0., winding: 0 })?;
    tick()?;
    let mut active_edges = budget.try_buffer(count, 0usize)?;
    let mut active_count = 0usize;
    let mut next_edge = 0usize;
    tick()?;
    let mut output = budget.try_buffer(pixels, 0u8)?;
    let samples = usize::from(limits.samples);
    let subwidth = (width as usize).checked_mul(samples).ok_or(EpsFillError::Limit)?;
    for row in 0..height as usize {
        for sy in 0..samples {
            tick()?;
            let y = row as f64 + (sy as f64 + 0.5) / samples as f64;
            while next_edge < count && storage[next_edge].start[1].min(storage[next_edge].end[1]) <= y {
                tick()?;
                let edge = storage[next_edge];
                if y < edge.start[1].max(edge.end[1]) {
                    active_edges[active_count] = next_edge;
                    active_count += 1;
                }
                next_edge += 1;
            }
            let mut active = 0;
            let mut position = 0;
            while position < active_count {
                tick()?;
                let edge = storage[active_edges[position]];
                let a = edge.start;
                let b = edge.end;
                if y >= a[1].max(b[1]) {
                    active_count -= 1;
                    active_edges[position] = active_edges[active_count];
                    continue;
                }
                position += 1;
                let fraction = (y - a[1]) / (b[1] - a[1]);
                let x = a[0] * (1. - fraction) + b[0] * fraction;
                if !fraction.is_finite() || !x.is_finite() {
                    return Err(EpsFillError::Range);
                }
                crossings[active] = Crossing {
                    x,
                    winding: if b[1] > a[1] { 1 } else { -1 },
                };
                active += 1;
            }
            // In-place heapsort has bounded O(n log n) work, no allocation, and
            // polls cancellation during comparisons and swaps.
            sort(
                &mut crossings[..active],
                |crossing: &Crossing| crossing.x,
                &mut tick,
            )?;
            let mut winding = 0i64;
            let mut previous = 0.;
            let mut index = 0;
            while index < active {
                tick()?;
                let x = crossings[index].x;
                if if even_odd { winding % 2 != 0 } else { winding != 0 } {
                    let first = (previous * samples as f64 - 0.5)
                        .ceil()
                        .clamp(0., subwidth as f64) as usize;
                    let end = (x * samples as f64 - 0.5).ceil().clamp(0., subwidth as f64) as usize;
                    for sample in first..end {
                        tick()?;
                        output[row * width as usize + sample / samples] += 1;
                    }
                }
                while index < active && crossings[index].x == x {
                    tick()?;
                    winding = winding
                        .checked_add(crossings[index].winding)
                        .ok_or(EpsFillError::Limit)?;
                    index += 1;
                }
                previous = x;
            }
        }
    }
    let total = samples * samples;
    for value in output.iter_mut() {
        tick()?;
        *value = ((usize::from(*value) * 255 + total / 2) / total) as u8;
    }
    tick()?;
    Ok(output.freeze())
}
fn sort<T, K: Fn(&T) -> f64, F: FnMut() -> Result<(), EpsFillError>>(
    values: &mut [T],
    key: K,
    tick: &mut F,
) -> Result<(), EpsFillError> {
    for start in (0..values.len() / 2).rev() {
        sift(values, start, &key, tick)?;
    }
    for end in (1..values.len()).rev() {
        tick()?;
        values.swap(0, end);
        sift(&mut values[..end], 0, &key, tick)?;
    }
    Ok(())
}
fn sift<T, K: Fn(&T) -> f64, F: FnMut() -> Result<(), EpsFillError>>(
    values: &mut [T],
    mut parent: usize,
    key: &K,
    tick: &mut F,
) -> Result<(), EpsFillError> {
    while let Some(left) = parent
        .checked_mul(2)
        .and_then(|n| n.checked_add(1))
        .filter(|&n| n < values.len())
    {
        tick()?;
        let mut child = left;
        if left + 1 < values.len() && key(&values[left + 1]) > key(&values[left]) {
            child = left + 1;
        }
        if key(&values[parent]) >= key(&values[child]) {
            break;
        }
        values.swap(parent, child);
        parent = child;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn empty_coverage_skips_image_scan_and_preserves_admission_and_cancel() {
        let root = MemoryBudget::new(1_048_576);
        let limits = EpsFillLimits {
            max_work: 16,
            ..EpsFillLimits::default()
        };
        let horizontal = [
            EpsPathSegment::Move([0., 2.]),
            EpsPathSegment::Line {
                start: [0., 2.],
                end: [999., 2.],
            },
        ];
        for path in [&[][..], &horizontal[..]] {
            let mask = fill_eps_path(path, 1024, 1024, false, limits, &root, || false).unwrap();
            assert!(mask.iter().all(|&value| value == 0));
            assert_eq!(root.used(), 1_048_576);
            drop(mask);
            assert_eq!(root.used(), 0);
        }
        let short = MemoryBudget::new(1_048_575);
        assert!(matches!(
            fill_eps_path(&[], 1024, 1024, false, limits, &short, || false),
            Err(EpsFillError::Memory(_))
        ));
        assert_eq!(short.peak(), 0);
        let calls = std::cell::Cell::new(0);
        assert!(matches!(
            fill_eps_path(&[], 1024, 1024, false, limits, &root, || {
                calls.set(calls.get() + 1);
                calls.get() == 3
            }),
            Err(EpsFillError::Cancelled)
        ));
        assert_eq!(root.used(), 0);
        let invalid = [EpsPathSegment::Move([f64::NAN, 0.])];
        let clean = MemoryBudget::new(1_048_576);
        assert!(matches!(
            fill_eps_path(&invalid, 1024, 1024, false, limits, &clean, || false),
            Err(EpsFillError::Range)
        ));
        assert_eq!(clean.peak(), 0);
    }
    fn rect(x: f64, y: f64, w: f64, h: f64) -> [EpsPathSegment; 4] {
        [
            EpsPathSegment::Move([x, y]),
            EpsPathSegment::Line {
                start: [x, y],
                end: [x + w, y],
            },
            EpsPathSegment::Line {
                start: [x + w, y],
                end: [x + w, y + h],
            },
            EpsPathSegment::Line {
                start: [x + w, y + h],
                end: [x, y + h],
            },
        ]
    }
    #[test]
    fn analytic_coverage_open_closure_holes_and_root_lifetime() {
        let root = MemoryBudget::new(100_000);
        let limits = EpsFillLimits::default();
        let mask = fill_eps_path(&rect(0.5, 0.5, 2., 2.), 4, 4, false, limits, &root, || false).unwrap();
        assert_eq!(
            &*mask,
            &[64, 128, 64, 0, 128, 255, 128, 0, 64, 128, 64, 0, 0, 0, 0, 0]
        );
        assert_eq!(root.used(), 16);
        let last = mask.clone();
        drop(mask);
        assert_eq!(root.used(), 16);
        drop(last);
        assert_eq!(root.used(), 0);
        let path = [rect(0., 0., 4., 4.), rect(1., 1., 2., 2.)].concat();
        let nonzero = fill_eps_path(&path, 4, 4, false, limits, &root, || false).unwrap();
        assert!(nonzero.iter().all(|&v| v == 255));
        let parity = fill_eps_path(&path, 4, 4, true, limits, &root, || false).unwrap();
        assert_eq!(
            &*parity,
            &[
                255, 255, 255, 255, 255, 0, 0, 255, 255, 0, 0, 255, 255, 255, 255, 255
            ]
        );
    }
    #[test]
    fn sparse_tall_page_visits_only_active_edges_with_bounded_work() {
        // 100 disjoint one-row rectangles spread down a tall page. A full
        // edge scan needs over 200k checks; active admission stays below 30k.
        let mut path = Vec::new();
        for row in (0..1000).step_by(10) {
            path.extend(rect(0., row as f64, 1., 1.));
        }
        let root = MemoryBudget::new(100_000);
        let limits = EpsFillLimits {
            samples: 1,
            max_work: 30_000,
            ..EpsFillLimits::default()
        };
        let mask = fill_eps_path(&path, 1, 1000, false, limits, &root, || false).unwrap();
        for (row, &value) in mask.iter().enumerate() {
            assert_eq!(value, if row % 10 == 0 { 255 } else { 0 });
        }
        assert_eq!(root.used(), 1000);
        drop(mask);
        assert_eq!(root.used(), 0);
    }
    #[test]
    fn allocation_work_and_cancellation_fail_without_partial_mask() {
        let path = rect(0., 0., 4., 4.);
        let limits = EpsFillLimits::default();
        let root = MemoryBudget::new(1);
        assert!(matches!(
            fill_eps_path(&path, 4, 4, false, limits, &root, || false),
            Err(EpsFillError::Memory(_))
        ));
        assert_eq!(root.used(), 0);
        let root = MemoryBudget::new(100_000);
        let moves = [EpsPathSegment::Move([0., 0.]); 100];
        assert!(matches!(
            fill_eps_path(
                &moves,
                1,
                1,
                false,
                EpsFillLimits {
                    max_work: 10,
                    ..limits
                },
                &root,
                || false
            ),
            Err(EpsFillError::Limit)
        ));
        assert_eq!(root.peak(), 0);
        assert!(matches!(
            fill_eps_path(
                &path,
                4,
                4,
                false,
                EpsFillLimits {
                    max_work: 20,
                    ..limits
                },
                &root,
                || false
            ),
            Err(EpsFillError::Limit)
        ));
        assert_eq!(root.used(), 0);
        assert!(matches!(
            fill_eps_path(&path, 4, 4, false, limits, &root, || root.used() > 100),
            Err(EpsFillError::Cancelled)
        ));
        assert_eq!(root.used(), 0);
    }
}
