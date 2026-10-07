//! Affine-aware stroke curve preparation, with shared-root managed storage.
use crate::{
    EpsFlattenError, EpsFlattenLimits, EpsGraphicsError, EpsMatrix, EpsPaintKind, EpsPathSegment,
    EpsStrokeError, EpsStrokeStyle, EpsVectorScene, flatten_eps_path,
};
use rrrah_core::{MemoryBudget, SharedBuffer};
#[derive(Debug, Clone)]
pub struct EpsPreparedStroke {
    pub path: SharedBuffer<EpsPathSegment>,
    /// Maps the flattened stroke-user geometry and its pen into raster space.
    pub matrix: EpsMatrix,
    pub style: EpsStrokeStyle,
}
#[derive(Debug, Clone, Copy)]
pub struct EpsStrokeAssemblyLimits {
    pub tolerance: f64,
    pub max_subpaths: usize,
    pub max_segments: usize,
    pub max_work_per_subpath: usize,
}
impl Default for EpsStrokeAssemblyLimits {
    fn default() -> Self {
        Self {
            tolerance: 0.125,
            max_subpaths: 4096,
            max_segments: 1_000_000,
            max_work_per_subpath: 4_000_000,
        }
    }
}
/// Assembles every prepared subpath into one nonzero-fill stroke outline.
/// Exact final output plus temporary point/outline buffers share the root.
pub fn outline_eps_prepared_stroke<F: FnMut() -> bool>(
    prepared: &EpsPreparedStroke,
    limits: EpsStrokeAssemblyLimits,
    budget: &MemoryBudget,
    mut cancelled: F,
) -> Result<SharedBuffer<EpsPathSegment>, EpsStrokeError> {
    let cancelled = std::cell::RefCell::new(&mut cancelled);
    let run = |emit: &mut dyn FnMut(EpsPathSegment) -> Result<(), EpsStrokeError>| {
        let mut at = 0;
        let mut subpaths = 0usize;
        let mut count = 0usize;
        while at < prepared.path.len() {
            if (cancelled.borrow_mut())() {
                return Err(EpsStrokeError::Cancelled);
            }
            subpaths = subpaths.checked_add(1).ok_or(EpsStrokeError::Limit)?;
            if subpaths > limits.max_subpaths {
                return Err(EpsStrokeError::Limit);
            }
            let EpsPathSegment::Move(origin) = prepared.path[at] else {
                return Err(EpsStrokeError::Range);
            };
            let begin = at;
            at += 1;
            let mut previous = origin;
            let mut closed = false;
            while at < prepared.path.len() {
                if (cancelled.borrow_mut())() {
                    return Err(EpsStrokeError::Cancelled);
                }
                match prepared.path[at] {
                    EpsPathSegment::Move(_) => break,
                    EpsPathSegment::Line { start, end } => {
                        if closed || start != previous {
                            return Err(EpsStrokeError::Range);
                        }
                        previous = end;
                    }
                    EpsPathSegment::Close { start, end } => {
                        if closed || start != previous || end != origin {
                            return Err(EpsStrokeError::Range);
                        }
                        closed = true;
                    }
                    EpsPathSegment::Curve { .. } => return Err(EpsStrokeError::Range),
                }
                at += 1;
                if at - begin > limits.max_work_per_subpath {
                    return Err(EpsStrokeError::Limit);
                }
            }
            let point_count = at - begin - usize::from(closed);
            let mut points = budget.try_buffer(point_count, [0.; 2])?;
            points[0] = origin;
            for (index, segment) in prepared.path[begin + 1..begin + point_count].iter().enumerate() {
                if (cancelled.borrow_mut())() {
                    return Err(EpsStrokeError::Cancelled);
                }
                let EpsPathSegment::Line { end, .. } = segment else {
                    return Err(EpsStrokeError::Range);
                };
                points[index + 1] = *end;
            }
            let outline = crate::outline_eps_polyline(
                &points,
                closed,
                prepared.style,
                prepared.matrix,
                limits.tolerance,
                limits.max_segments,
                limits.max_work_per_subpath,
                budget,
                || (cancelled.borrow_mut())(),
            )?;
            for &segment in outline.iter() {
                if (cancelled.borrow_mut())() {
                    return Err(EpsStrokeError::Cancelled);
                }
                count = count.checked_add(1).ok_or(EpsStrokeError::Limit)?;
                if count > limits.max_segments {
                    return Err(EpsStrokeError::Limit);
                }
                emit(segment)?;
            }
        }
        if (cancelled.borrow_mut())() {
            return Err(EpsStrokeError::Cancelled);
        }
        Ok(count)
    };
    let count = run(&mut |_| Ok(()))?;
    let mut output = budget.try_buffer(count, EpsPathSegment::Move([0.; 2]))?;
    let mut at = 0;
    run(&mut |segment| {
        output[at] = segment;
        at += 1;
        Ok(())
    })?;
    if (cancelled.borrow_mut())() {
        return Err(EpsStrokeError::Cancelled);
    }
    Ok(output.freeze())
}
#[derive(Debug, thiserror::Error)]
pub enum EpsStrokePrepareError {
    #[error("paint is not a stroke or transform/tolerance is invalid")]
    Range,
    #[error(transparent)]
    Graphics(#[from] EpsGraphicsError),
    #[error(transparent)]
    Flatten(#[from] EpsFlattenError),
    #[error(transparent)]
    Stroke(#[from] EpsStrokeError),
}
/// `limits.tolerance` is in final pixels. The Frobenius norm conservatively
/// maps that error allowance back into stroke-user coordinates before native
/// flattening. Final paint CTM defines the pen, even if path nodes predate it.
pub fn prepare_eps_stroke<F: FnMut() -> bool>(
    scene: &EpsVectorScene,
    paint: usize,
    viewport: EpsMatrix,
    mut limits: EpsFlattenLimits,
    budget: &MemoryBudget,
    mut cancelled: F,
) -> Result<EpsPreparedStroke, EpsStrokePrepareError> {
    if cancelled() {
        return Err(EpsGraphicsError::Cancelled.into());
    }
    let paint_info = scene.paints().get(paint).ok_or(EpsStrokePrepareError::Range)?;
    if paint_info.kind != EpsPaintKind::Stroke || !viewport.iter().all(|v| v.is_finite()) {
        return Err(EpsStrokePrepareError::Range);
    }
    if paint_info.style.width == 0. {
        // Software raster device policy: one-pixel circular pen. World path
        // nodes already contain construction CTMs, so no paint-CTM inverse is
        // needed, including when the final paint matrix is singular.
        let path = scene.prepare_path(paint, viewport, budget, &mut cancelled)?;
        let path = flatten_eps_path(&path, limits, budget, &mut cancelled)?;
        if cancelled() {
            return Err(EpsGraphicsError::Cancelled.into());
        }
        return Ok(EpsPreparedStroke {
            path,
            matrix: [1., 0., 0., 1., 0., 0.],
            style: EpsStrokeStyle {
                width: 1.,
                ..paint_info.style
            },
        });
    }
    let a = viewport;
    let b = paint_info.matrix;
    let matrix = [
        a[0] * b[0] + a[2] * b[1],
        a[1] * b[0] + a[3] * b[1],
        a[0] * b[2] + a[2] * b[3],
        a[1] * b[2] + a[3] * b[3],
        a[0] * b[4] + a[2] * b[5] + a[4],
        a[1] * b[4] + a[3] * b[5] + a[5],
    ];
    let norm = matrix[0].hypot(matrix[1]).hypot(matrix[2].hypot(matrix[3]));
    if !matrix.iter().all(|v| v.is_finite()) || norm == 0. || !norm.is_finite() {
        return Err(EpsStrokePrepareError::Range);
    }
    limits.tolerance /= norm;
    if !limits.tolerance.is_finite() || limits.tolerance <= 0. {
        return Err(EpsStrokePrepareError::Range);
    }
    let inverse = crate::eps_graphics::inverse(paint_info.matrix)?;
    let path = scene.prepare_path(paint, inverse, budget, &mut cancelled)?;
    let path = flatten_eps_path(&path, limits, budget, &mut cancelled)?;
    if cancelled() {
        return Err(EpsGraphicsError::Cancelled.into());
    }
    Ok(EpsPreparedStroke {
        path,
        matrix,
        style: paint_info.style,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn prepared_multiple_subpaths_stroke_into_one_managed_outline() {
        let root = MemoryBudget::new(100_000);
        let mut g = crate::EpsGraphics::new(
            crate::EpsGraphicsLimits {
                max_nodes: 20,
                max_paints: 1,
                max_saved_states: 0,
            },
            &root,
            || false,
        )
        .unwrap();
        g.set_line_width(2.).unwrap();
        g.move_to(1., 1.).unwrap();
        g.line_to(4., 1.).unwrap();
        g.line_to(4., 4.).unwrap();
        g.line_to(1., 4.).unwrap();
        g.close_path().unwrap();
        g.move_to(6., 1.).unwrap();
        g.line_to(6., 4.).unwrap();
        g.paint(EpsPaintKind::Stroke).unwrap();
        let scene = g.finish();
        let output = MemoryBudget::new(100_000);
        let prepared = prepare_eps_stroke(
            &scene,
            0,
            [1., 0., 0., 1., 0., 0.],
            EpsFlattenLimits::default(),
            &output,
            || false,
        )
        .unwrap();
        let outline =
            outline_eps_prepared_stroke(&prepared, EpsStrokeAssemblyLimits::default(), &output, || false)
                .unwrap();
        let mask = crate::fill_eps_path(
            &outline,
            8,
            5,
            false,
            crate::EpsFillLimits::default(),
            &output,
            || false,
        )
        .unwrap();
        assert_eq!(mask[2 * 8 + 2], 0);
        assert_eq!(mask[2 * 8], 255);
        assert_eq!(mask[2 * 8 + 5], 255);
        assert_eq!(mask[2 * 8 + 6], 255);
        assert_eq!(mask[2 * 8 + 7], 0);
        drop(mask);
        drop(outline);
        assert!(matches!(
            outline_eps_prepared_stroke(
                &prepared,
                EpsStrokeAssemblyLimits {
                    max_subpaths: 1,
                    ..EpsStrokeAssemblyLimits::default()
                },
                &output,
                || false
            ),
            Err(EpsStrokeError::Limit)
        ));
        drop(prepared);
        assert_eq!(output.used(), 0);
    }
    #[test]
    fn inverse_final_ctm_and_pixel_tolerance_prepare_native_curves() {
        let root = MemoryBudget::new(100_000);
        let mut g = crate::EpsGraphics::new(
            crate::EpsGraphicsLimits {
                max_nodes: 10,
                max_paints: 1,
                max_saved_states: 0,
            },
            &root,
            || false,
        )
        .unwrap();
        g.move_to(0., 0.).unwrap();
        g.curve_to([0., 10., 10., 10., 10., 0.]).unwrap();
        g.scale(2., 3.).unwrap();
        g.paint(EpsPaintKind::Stroke).unwrap();
        let scene = g.finish();
        let output = MemoryBudget::new(100_000);
        let prepared = prepare_eps_stroke(
            &scene,
            0,
            [1., 0., 0., -1., 0., 20.],
            EpsFlattenLimits::default(),
            &output,
            || false,
        )
        .unwrap();
        assert_eq!(prepared.matrix, [2., 0., 0., -3., 0., 20.]);
        assert_eq!(prepared.path[0], EpsPathSegment::Move([0., 0.]));
        let EpsPathSegment::Line { end, .. } = prepared.path[prepared.path.len() - 1] else {
            panic!()
        };
        assert_eq!(end, [5., 0.]);
        assert!(
            !prepared
                .path
                .iter()
                .any(|s| matches!(s, EpsPathSegment::Curve { .. }))
        );
        assert_eq!(
            output.used(),
            (prepared.path.len() * std::mem::size_of::<EpsPathSegment>()) as u64
        );
        drop(prepared);
        assert_eq!(output.used(), 0);
        assert!(matches!(
            prepare_eps_stroke(
                &scene,
                0,
                [1., 0., 0., 1., 0., 0.],
                EpsFlattenLimits::default(),
                &output,
                || output.used() != 0
            ),
            Err(EpsStrokePrepareError::Graphics(EpsGraphicsError::Cancelled))
        ));
        assert_eq!(output.used(), 0);
    }
}
