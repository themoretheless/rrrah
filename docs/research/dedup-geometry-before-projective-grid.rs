//! Bounded robust similarity-transform evidence for local feature correspondences.
//! Caller supplies descriptor-qualified one-to-one correspondences. Geometry
//! alone never proves image identity; repeated patterns need pixel/local evidence.

#[derive(Debug, Clone, Copy)]
pub struct Correspondence {
    pub source: [f64; 2],
    pub target: [f64; 2],
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform {
    pub a: f64,
    pub b: f64,
    pub translation: [f64; 2],
}
impl Transform {
    pub fn apply(&self, p: [f64; 2]) -> [f64; 2] {
        [
            self.a * p[0] - self.b * p[1] + self.translation[0],
            self.b * p[0] + self.a * p[1] + self.translation[1],
        ]
    }
}
/// Projective mapping of a planar surface; geometry alone never proves a match.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProjectiveTransform {
    pub matrix: [[f64; 3]; 3],
}
impl ProjectiveTransform {
    /// Invert a finite nonsingular matrix, retaining its arbitrary projective scale.
    ///
    /// # Errors
    /// Nonfinite coefficients or a singular/overflowing inverse.
    pub fn inverse(&self) -> Result<Self, GeometryError> {
        if self.matrix.iter().flatten().any(|v| !v.is_finite()) {
            return Err(GeometryError::Invalid);
        }
        let scale = self.matrix.iter().flatten().fold(0.0_f64, |a, b| a.max(b.abs()));
        if scale == 0.0 {
            return Err(GeometryError::Invalid);
        }
        let m = self.matrix.map(|row| row.map(|v| v / scale));
        let cofactors: [[f64; 3]; 3] = std::array::from_fn(|i| {
            std::array::from_fn(|j| {
                let r = [(i + 1) % 3, (i + 2) % 3];
                let c = [(j + 1) % 3, (j + 2) % 3];
                m[r[0]][c[0]] * m[r[1]][c[1]] - m[r[0]][c[1]] * m[r[1]][c[0]]
            })
        });
        let determinant = (0..3).map(|j| m[0][j] * cofactors[0][j]).sum::<f64>();
        if !determinant.is_finite() || determinant == 0.0 {
            return Err(GeometryError::Invalid);
        }
        // The adjugate is itself a projectively equivalent inverse; avoid dividing
        // by a tiny determinant and introducing an unnecessary overflow.
        let matrix = std::array::from_fn(|i| std::array::from_fn(|j| cofactors[j][i]));
        Ok(Self { matrix })
    }
    /// Returns None at a projective horizon or for nonfinite input/output.
    pub fn apply(&self, point: [f64; 2]) -> Option<[f64; 2]> {
        let q = self
            .matrix
            .map(|row| row[0] * point[0] + row[1] * point[1] + row[2]);
        if q.iter().any(|v| !v.is_finite()) || q[2] == 0.0 {
            return None;
        }
        let result = [q[0] / q[2], q[1] / q[2]];
        result.iter().all(|v| v.is_finite()).then_some(result)
    }
}

type NormalizedFour = ([[f64; 2]; 4], [f64; 2], f64);

/// Fit four supplied correspondences using centered/scaled coordinates and
/// pivoted elimination. Singular configurations refuse explicitly. This is a
/// minimal hypothesis, not robust search or pixel verification.
///
/// # Errors
/// Nonfinite/degenerate inputs, unstable systems or cancellation.
pub fn fit_projective_four(
    points: &[Correspondence; 4],
    cancel: impl Fn() -> bool,
) -> Result<ProjectiveTransform, GeometryError> {
    if cancel() {
        return Err(GeometryError::Cancelled);
    }
    let normalize = |target: bool| -> Result<NormalizedFour, GeometryError> {
        let p = points.map(|p| if target { p.target } else { p.source });
        if p.iter().flatten().any(|v| !v.is_finite()) {
            return Err(GeometryError::Invalid);
        }
        let center = [
            p.iter().map(|p| p[0] / 4.0).sum::<f64>(),
            p.iter().map(|p| p[1] / 4.0).sum::<f64>(),
        ];
        let scale = p
            .iter()
            .flat_map(|p| [(p[0] - center[0]).abs(), (p[1] - center[1]).abs()])
            .fold(0.0_f64, f64::max);
        if !scale.is_finite() || scale <= 0.0 {
            return Err(GeometryError::Invalid);
        }
        Ok((
            p.map(|p| [(p[0] - center[0]) / scale, (p[1] - center[1]) / scale]),
            center,
            scale,
        ))
    };
    let (source, sc, ss) = normalize(false)?;
    let (target, tc, ts) = normalize(true)?;
    let mut rows = [[0.0; 9]; 8];
    for i in 0..4 {
        let [x, y] = source[i];
        let [u, v] = target[i];
        rows[2 * i] = [x, y, 1.0, 0.0, 0.0, 0.0, -u * x, -u * y, u];
        rows[2 * i + 1] = [0.0, 0.0, 0.0, x, y, 1.0, -v * x, -v * y, v];
    }
    for column in 0..8 {
        if cancel() {
            return Err(GeometryError::Cancelled);
        }
        let pivot = (column..8)
            .max_by(|a, b| rows[*a][column].abs().total_cmp(&rows[*b][column].abs()))
            .ok_or(GeometryError::Invalid)?;
        if rows[pivot][column].abs() < 1e-12 {
            return Err(GeometryError::Invalid);
        }
        rows.swap(column, pivot);
        let divisor = rows[column][column];
        for item in &mut rows[column][column..] {
            *item /= divisor;
        }
        let pivot_row = rows[column];
        for (index, row) in rows.iter_mut().enumerate() {
            if index == column {
                continue;
            }
            let factor = row[column];
            for (value, pivot_value) in row.iter_mut().zip(pivot_row).skip(column) {
                *value -= factor * pivot_value;
            }
        }
    }
    let h = [
        [rows[0][8], rows[1][8], rows[2][8]],
        [rows[3][8], rows[4][8], rows[5][8]],
        [rows[6][8], rows[7][8], 1.0],
    ];
    let det = h[0][0] * (h[1][1] * h[2][2] - h[1][2] * h[2][1])
        - h[0][1] * (h[1][0] * h[2][2] - h[1][2] * h[2][0])
        + h[0][2] * (h[1][0] * h[2][1] - h[1][1] * h[2][0]);
    let norm = h.iter().flatten().fold(0.0_f64, |a, b| a.max(b.abs()));
    if !det.is_finite() || det.abs() <= 1e-12 * norm.powi(3) {
        return Err(GeometryError::Invalid);
    }
    let multiply = |a: [[f64; 3]; 3], b: [[f64; 3]; 3]| -> [[f64; 3]; 3] {
        std::array::from_fn(|i| std::array::from_fn(|j| (0..3).map(|k| a[i][k] * b[k][j]).sum()))
    };
    let matrix = multiply(
        [[ts, 0.0, tc[0]], [0.0, ts, tc[1]], [0.0, 0.0, 1.0]],
        multiply(
            h,
            [
                [1.0 / ss, 0.0, -sc[0] / ss],
                [0.0, 1.0 / ss, -sc[1] / ss],
                [0.0, 0.0, 1.0],
            ],
        ),
    );
    if matrix.iter().flatten().any(|v| !v.is_finite()) {
        return Err(GeometryError::Invalid);
    }
    if cancel() {
        return Err(GeometryError::Cancelled);
    }
    Ok(ProjectiveTransform { matrix })
}

#[derive(Debug, Clone, PartialEq)]
pub struct GeometryEvidence {
    pub transform: Transform,
    pub inliers: Vec<usize>,
    pub squared_error: f64,
}
fn copy_evidence(previous: &GeometryEvidence) -> Result<GeometryEvidence, GeometryError> {
    let mut inliers = Vec::new();
    inliers
        .try_reserve_exact(previous.inliers.len())
        .map_err(|_| GeometryError::Budget)?;
    inliers.extend_from_slice(&previous.inliers);
    Ok(GeometryEvidence {
        transform: previous.transform,
        inliers,
        squared_error: previous.squared_error,
    })
}
#[derive(Debug, Clone, Copy)]
pub struct GeometryPolicy {
    pub tolerance: f64,
    pub min_inliers: usize,
    pub max_points: usize,
    pub max_hypotheses: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GeometryError {
    #[error("invalid geometry policy or non-finite/duplicate feature coordinates")]
    Invalid,
    #[error("geometric search exceeded the supplied work budget; inconclusive")]
    Budget,
    #[error("geometric search cancelled; inconclusive")]
    Cancelled,
}

/// Exhaustively verify deterministic two-point scale/rotation/translation models.
/// Rank two-point models by inlier count then residual; ties retain first model.
/// Refine the winning support once by centered least squares, then rescore every
/// correspondence. Keep refinement only if the same ranking improves. This adds
/// bounded linear work after exhaustive hypotheses, with no iterative fitting.
/// Collinear/degenerate support is rejected using its 2D covariance determinant.
/// The model excludes perspective warps and reflections. Empty result means no
/// qualifying model among these supplied correspondences, not unrelated images.
///
/// # Errors
/// Rejects nonfinite values, duplicate source/target points, invalid policy,
/// hypothesis-budget excess and cancellation without a partial result.
pub fn verify_similarity(
    points: &[Correspondence],
    policy: GeometryPolicy,
    cancel: impl Fn() -> bool,
) -> Result<Option<GeometryEvidence>, GeometryError> {
    Ok(verify_similarity_selected(points, policy, &cancel, false)?[0].take())
}

/// Retain at most two geometrically qualified models from the same bounded
/// hypothesis search: the count-first winner and a capped-residual winner.
/// The latter minimizes sum(min(squared residual, tolerance squared)) over all
/// correspondences, so a marginal extra inlier cannot dominate many exact ones.
/// Each winner receives one bounded least-squares refinement. Identical final
/// transforms are returned once. These are candidates requiring pixel evidence,
/// not a declaration of image identity or an exhaustive list of valid models.
///
/// # Errors
/// Same invalid input, hypothesis/allocation budget and cancellation rules as
/// `verify_similarity`; no partially searched candidates are returned.
pub fn verify_similarity_candidates(
    points: &[Correspondence],
    policy: GeometryPolicy,
    cancel: impl Fn() -> bool,
) -> Result<[Option<GeometryEvidence>; 2], GeometryError> {
    verify_similarity_selected(points, policy, &cancel, true)
}

#[allow(clippy::too_many_lines)]
fn verify_similarity_selected(
    points: &[Correspondence],
    policy: GeometryPolicy,
    cancel: &impl Fn() -> bool,
    retain_robust: bool,
) -> Result<[Option<GeometryEvidence>; 2], GeometryError> {
    let tolerance2 = validate(points, policy, cancel)?;
    let mut hypotheses = 0;
    let mut best: Option<GeometryEvidence> = None;
    let mut robust: Option<GeometryEvidence> = None;
    let mut robust_cost = f64::INFINITY;
    for i in 0..points.len() {
        for j in i + 1..points.len() {
            if cancel() {
                return Err(GeometryError::Cancelled);
            }
            if hypotheses >= policy.max_hypotheses {
                return Err(GeometryError::Budget);
            }
            hypotheses += 1;
            let s = [
                points[j].source[0] - points[i].source[0],
                points[j].source[1] - points[i].source[1],
            ];
            let t = [
                points[j].target[0] - points[i].target[0],
                points[j].target[1] - points[i].target[1],
            ];
            let den = s[0] * s[0] + s[1] * s[1];
            if !den.is_finite() || den <= 0.0 {
                return Err(GeometryError::Invalid);
            }
            let a = (s[0] * t[0] + s[1] * t[1]) / den;
            let b = (s[0] * t[1] - s[1] * t[0]) / den;
            if !a.is_finite() || !b.is_finite() {
                return Err(GeometryError::Invalid);
            }
            if a.hypot(b) == 0.0 {
                continue;
            }
            let transform = Transform {
                a,
                b,
                translation: [
                    points[i].target[0] - a * points[i].source[0] + b * points[i].source[1],
                    points[i].target[1] - b * points[i].source[0] - a * points[i].source[1],
                ],
            };
            if transform.translation.iter().any(|v| !v.is_finite()) {
                return Err(GeometryError::Invalid);
            }
            let mut inliers = Vec::new();
            inliers
                .try_reserve_exact(points.len())
                .map_err(|_| GeometryError::Budget)?;
            let mut squared_error = 0.0;
            let mut capped_error = 0.0;
            for (index, point) in points.iter().enumerate() {
                if cancel() {
                    return Err(GeometryError::Cancelled);
                }
                let predicted = transform.apply(point.source);
                let error =
                    (predicted[0] - point.target[0]).powi(2) + (predicted[1] - point.target[1]).powi(2);
                if retain_robust {
                    capped_error += if error.is_finite() {
                        error.min(tolerance2)
                    } else {
                        tolerance2
                    };
                    if !capped_error.is_finite() {
                        return Err(GeometryError::Invalid);
                    }
                }
                if error.is_finite() && error <= tolerance2 {
                    inliers.push(index);
                    squared_error += error;
                    if !squared_error.is_finite() {
                        return Err(GeometryError::Invalid);
                    }
                }
            }
            if inliers.len() < policy.min_inliers || !spread(points, &inliers, &cancel)? {
                continue;
            }
            let improves_count = best.as_ref().is_none_or(|previous| {
                inliers.len() > previous.inliers.len()
                    || (inliers.len() == previous.inliers.len() && squared_error < previous.squared_error)
            });
            let improves_robust = retain_robust && capped_error < robust_cost;
            if improves_count || improves_robust {
                let evidence = GeometryEvidence {
                    transform,
                    inliers,
                    squared_error,
                };
                if improves_robust {
                    robust = Some(copy_evidence(&evidence)?);
                    robust_cost = capped_error;
                }
                if improves_count {
                    best = Some(evidence);
                }
            }
        }
    }
    if let Some(previous) = &best {
        let refined = refine(points, previous, tolerance2, &cancel)?;
        if refined.inliers.len() >= policy.min_inliers
            && spread(points, &refined.inliers, &cancel)?
            && (refined.inliers.len() > previous.inliers.len()
                || (refined.inliers.len() == previous.inliers.len()
                    && refined.squared_error < previous.squared_error))
        {
            best = Some(refined);
        }
    }
    if let Some(previous) = &robust {
        let refined = refine(points, previous, tolerance2, cancel)?;
        if refined.inliers.len() >= policy.min_inliers
            && spread(points, &refined.inliers, cancel)?
            && capped_cost(points, refined.transform, tolerance2, cancel)? < robust_cost
        {
            robust = Some(refined);
        }
    }
    if best
        .as_ref()
        .zip(robust.as_ref())
        .is_some_and(|(a, b)| a.transform == b.transform)
    {
        robust = None;
    }
    if cancel() {
        return Err(GeometryError::Cancelled);
    }
    Ok([best, robust])
}

fn capped_cost(
    points: &[Correspondence],
    transform: Transform,
    tolerance2: f64,
    cancel: &impl Fn() -> bool,
) -> Result<f64, GeometryError> {
    let mut cost = 0.0;
    for point in points {
        if cancel() {
            return Err(GeometryError::Cancelled);
        }
        let predicted = transform.apply(point.source);
        let error = (predicted[0] - point.target[0]).powi(2) + (predicted[1] - point.target[1]).powi(2);
        cost += if error.is_finite() {
            error.min(tolerance2)
        } else {
            tolerance2
        };
        if !cost.is_finite() {
            return Err(GeometryError::Invalid);
        }
    }
    Ok(cost)
}

#[allow(clippy::cast_precision_loss)]
fn spread(
    points: &[Correspondence],
    indices: &[usize],
    cancel: &impl Fn() -> bool,
) -> Result<bool, GeometryError> {
    let mut mean = [0.0; 2];
    for &index in indices {
        if cancel() {
            return Err(GeometryError::Cancelled);
        }
        for (channel, value) in mean.iter_mut().enumerate() {
            *value += points[index].source[channel] / indices.len() as f64;
        }
    }
    let mut covariance = [0.0; 3];
    for &index in indices {
        if cancel() {
            return Err(GeometryError::Cancelled);
        }
        let x = points[index].source[0] - mean[0];
        let y = points[index].source[1] - mean[1];
        covariance[0] += x * x;
        covariance[1] += y * y;
        covariance[2] += x * y;
    }
    let determinant = covariance[0] * covariance[1] - covariance[2] * covariance[2];
    Ok(determinant.is_finite() && determinant > (covariance[0] + covariance[1]).powi(2) * 1e-8)
}

/// Bounded perspective hypothesis evidence requiring subsequent pixel checks.
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectiveEvidence {
    pub transform: ProjectiveTransform,
    pub inliers: Vec<usize>,
    pub squared_error: f64,
    pub hypotheses: u64,
}

/// Exhaustive deterministic four-point projective search with explicit work limits.
/// Ranks hypotheses by support count, then residual. No partially searched result
/// is published on cancellation or budget exhaustion. This is planar geometry
/// evidence only, with no assertion of visual identity or confidence calibration.
///
/// # Errors
/// Invalid/duplicate inputs, `min_inliers` below four, work/allocation refusal,
/// and cancellation. Singular four-point hypotheses are skipped.
pub fn verify_projective(
    points: &[Correspondence],
    policy: GeometryPolicy,
    cancel: impl Fn() -> bool,
) -> Result<Option<ProjectiveEvidence>, GeometryError> {
    verify_projective_selected(points, policy, cancel, |_| true)
}

/// Exhaustively select only models valid on both full image rectangles.
/// Invalid candidates are excluded before ranking and after refinement; a
/// stronger ineligible model cannot hide a weaker eligible model. Geometry is
/// not copy identity. All hypotheses still consume the supplied work limit.
///
/// # Errors
/// Zero dimensions, invalid policy/points, budget exhaustion or cancellation.
pub fn verify_projective_for_domains(
    points: &[Correspondence],
    policy: GeometryPolicy,
    source: (u32, u32),
    target: (u32, u32),
    cancel: impl Fn() -> bool,
) -> Result<Option<ProjectiveEvidence>, GeometryError> {
    if source.0 == 0 || source.1 == 0 || target.0 == 0 || target.1 == 0 {
        return Err(GeometryError::Invalid);
    }
    verify_projective_selected(points, policy, cancel, |transform| {
        projective_domain_eligible(transform, source, target)
    })
}

fn verify_projective_selected(
    points: &[Correspondence],
    policy: GeometryPolicy,
    cancel: impl Fn() -> bool,
    eligible: impl Fn(ProjectiveTransform) -> bool,
) -> Result<Option<ProjectiveEvidence>, GeometryError> {
    let tolerance2 = validate(points, policy, &cancel)?;
    if policy.min_inliers < 4 {
        return Err(GeometryError::Invalid);
    }
    let mut best: Option<ProjectiveEvidence> = None;
    let mut hypotheses = 0;
    for a in 0..points.len() {
        for b in a + 1..points.len() {
            for c in b + 1..points.len() {
                for d in c + 1..points.len() {
                    if cancel() {
                        return Err(GeometryError::Cancelled);
                    }
                    if hypotheses >= policy.max_hypotheses {
                        return Err(GeometryError::Budget);
                    }
                    hypotheses += 1;
                    let transform =
                        match fit_projective_four(&[points[a], points[b], points[c], points[d]], &cancel) {
                            Ok(transform) => transform,
                            Err(GeometryError::Invalid) => continue,
                            Err(error) => return Err(error),
                        };
                    if !eligible(transform) {
                        continue;
                    }
                    let mut inliers = Vec::new();
                    inliers
                        .try_reserve_exact(points.len())
                        .map_err(|_| GeometryError::Budget)?;
                    let mut squared_error = 0.0;
                    for (index, point) in points.iter().enumerate() {
                        if cancel() {
                            return Err(GeometryError::Cancelled);
                        }
                        let Some(predicted) = transform.apply(point.source) else {
                            continue;
                        };
                        let residual = (predicted[0] - point.target[0]).powi(2)
                            + (predicted[1] - point.target[1]).powi(2);
                        if residual.is_finite() && residual <= tolerance2 {
                            inliers.push(index);
                            squared_error += residual;
                        }
                    }
                    if !squared_error.is_finite() {
                        return Err(GeometryError::Invalid);
                    }
                    if inliers.len() < policy.min_inliers || !spread(points, &inliers, &cancel)? {
                        continue;
                    }
                    if best.as_ref().is_none_or(|previous| {
                        inliers.len() > previous.inliers.len()
                            || (inliers.len() == previous.inliers.len()
                                && squared_error < previous.squared_error)
                    }) {
                        best = Some(ProjectiveEvidence {
                            transform,
                            inliers,
                            squared_error,
                            hypotheses,
                        });
                    }
                }
            }
        }
    }
    refine_projective_evidence(points, policy, tolerance2, best, hypotheses, &cancel, &eligible)
}

fn refine_projective_evidence(
    points: &[Correspondence],
    policy: GeometryPolicy,
    tolerance2: f64,
    mut best: Option<ProjectiveEvidence>,
    hypotheses: u64,
    cancel: &impl Fn() -> bool,
    eligible: &impl Fn(ProjectiveTransform) -> bool,
) -> Result<Option<ProjectiveEvidence>, GeometryError> {
    if cancel() {
        return Err(GeometryError::Cancelled);
    }
    if let Some(previous) = &best {
        match fit_projective_support(points, &previous.inliers, &cancel) {
            Ok(transform) => {
                let mut inliers = Vec::new();
                inliers
                    .try_reserve_exact(points.len())
                    .map_err(|_| GeometryError::Budget)?;
                let mut squared_error = 0.0;
                for (index, point) in points.iter().enumerate() {
                    if cancel() {
                        return Err(GeometryError::Cancelled);
                    }
                    let Some(p) = transform.apply(point.source) else {
                        continue;
                    };
                    let residual = (p[0] - point.target[0]).powi(2) + (p[1] - point.target[1]).powi(2);
                    if residual.is_finite() && residual <= tolerance2 {
                        inliers.push(index);
                        squared_error += residual;
                    }
                }
                if eligible(transform)
                    && squared_error.is_finite()
                    && inliers.len() >= policy.min_inliers
                    && spread(points, &inliers, &cancel)?
                    && (inliers.len() > previous.inliers.len()
                        || (inliers.len() == previous.inliers.len()
                            && squared_error < previous.squared_error))
                {
                    best = Some(ProjectiveEvidence {
                        transform,
                        inliers,
                        squared_error,
                        hypotheses,
                    });
                }
            }
            Err(GeometryError::Invalid) => {}
            Err(error) => return Err(error),
        }
    }
    if cancel() {
        return Err(GeometryError::Cancelled);
    }
    if let Some(evidence) = &mut best {
        evidence.hypotheses = hypotheses;
    }
    Ok(best)
}

// Streaming Givens QR retains only an 8x9 triangular system. Coordinate
// normalization limits conditioning without forming squared normal equations.
#[allow(clippy::cast_precision_loss)] // Support count was admitted by max_points.
fn fit_projective_support(
    points: &[Correspondence],
    support: &[usize],
    cancel: &impl Fn() -> bool,
) -> Result<ProjectiveTransform, GeometryError> {
    let support_count = support.len() as f64;
    let mut center = [[0.0; 2]; 2];
    for &index in support {
        if cancel() {
            return Err(GeometryError::Cancelled);
        }
        for (side, p) in [points[index].source, points[index].target].iter().enumerate() {
            for axis in 0..2 {
                center[side][axis] += p[axis] / support_count;
            }
        }
    }
    let mut scale = [0.0_f64; 2];
    for &index in support {
        if cancel() {
            return Err(GeometryError::Cancelled);
        }
        for (side, p) in [points[index].source, points[index].target].iter().enumerate() {
            for axis in 0..2 {
                scale[side] = scale[side].max((p[axis] - center[side][axis]).abs());
            }
        }
    }
    if scale.iter().any(|v| !v.is_finite() || *v <= 0.0) {
        return Err(GeometryError::Invalid);
    }
    let mut qr = [[0.0_f64; 9]; 8];
    for &index in support {
        let point = points[index];
        let [x, y] = std::array::from_fn(|axis| (point.source[axis] - center[0][axis]) / scale[0]);
        let [u, v] = std::array::from_fn(|axis| (point.target[axis] - center[1][axis]) / scale[1]);
        for mut row in [
            [x, y, 1.0, 0.0, 0.0, 0.0, -u * x, -u * y, u],
            [0.0, 0.0, 0.0, x, y, 1.0, -v * x, -v * y, v],
        ] {
            for column in 0..8 {
                if cancel() {
                    return Err(GeometryError::Cancelled);
                }
                let radius = qr[column][column].hypot(row[column]);
                if radius == 0.0 {
                    continue;
                }
                let cosine = qr[column][column] / radius;
                let sine = row[column] / radius;
                for (top, bottom) in qr[column].iter_mut().zip(&mut row).skip(column) {
                    let previous = *top;
                    *top = cosine * previous + sine * *bottom;
                    *bottom = -sine * previous + cosine * *bottom;
                }
            }
        }
    }
    let mut solution = [0.0; 8];
    for row in (0..8).rev() {
        if cancel() {
            return Err(GeometryError::Cancelled);
        }
        if qr[row][row].abs() < 1e-12 {
            return Err(GeometryError::Invalid);
        }
        solution[row] =
            (qr[row][8] - (row + 1..8).map(|col| qr[row][col] * solution[col]).sum::<f64>()) / qr[row][row];
    }
    let normalized_matrix = [
        [solution[0], solution[1], solution[2]],
        [solution[3], solution[4], solution[5]],
        [solution[6], solution[7], 1.0],
    ];
    let multiply = |a: [[f64; 3]; 3], b: [[f64; 3]; 3]| -> [[f64; 3]; 3] {
        std::array::from_fn(|i| std::array::from_fn(|j| (0..3).map(|k| a[i][k] * b[k][j]).sum()))
    };
    let matrix = multiply(
        [
            [scale[1], 0.0, center[1][0]],
            [0.0, scale[1], center[1][1]],
            [0.0, 0.0, 1.0],
        ],
        multiply(
            normalized_matrix,
            [
                [1.0 / scale[0], 0.0, -center[0][0] / scale[0]],
                [0.0, 1.0 / scale[0], -center[0][1] / scale[0]],
                [0.0, 0.0, 1.0],
            ],
        ),
    );
    let transform = ProjectiveTransform { matrix };
    transform.inverse()?;
    Ok(transform)
}

fn validate(
    points: &[Correspondence],
    policy: GeometryPolicy,
    cancel: &impl Fn() -> bool,
) -> Result<f64, GeometryError> {
    if !policy.tolerance.is_finite() || policy.tolerance <= 0.0 || policy.min_inliers < 3 {
        return Err(GeometryError::Invalid);
    }
    if points.len() > policy.max_points {
        return Err(GeometryError::Budget);
    }
    let mut sources = std::collections::HashSet::new();
    let mut targets = std::collections::HashSet::new();
    sources
        .try_reserve(points.len())
        .map_err(|_| GeometryError::Budget)?;
    targets
        .try_reserve(points.len())
        .map_err(|_| GeometryError::Budget)?;
    for point in points {
        if cancel() {
            return Err(GeometryError::Cancelled);
        }
        if point.source.iter().chain(&point.target).any(|v| !v.is_finite()) {
            return Err(GeometryError::Invalid);
        }
        let key = |p: [f64; 2]| p.map(|v| if v == 0.0 { 0 } else { v.to_bits() });
        if !sources.insert(key(point.source)) || !targets.insert(key(point.target)) {
            return Err(GeometryError::Invalid);
        }
    }
    let tolerance2 = policy.tolerance * policy.tolerance;
    if !tolerance2.is_finite() {
        return Err(GeometryError::Invalid);
    }
    Ok(tolerance2)
}

#[allow(clippy::cast_precision_loss)] // Support count is admitted by max_points.
fn refine(
    points: &[Correspondence],
    previous: &GeometryEvidence,
    tolerance2: f64,
    cancel: &impl Fn() -> bool,
) -> Result<GeometryEvidence, GeometryError> {
    let n = previous.inliers.len() as f64;
    let mut source = [0.; 2];
    let mut target = [0.; 2];
    for &i in &previous.inliers {
        if cancel() {
            return Err(GeometryError::Cancelled);
        }
        for c in 0..2 {
            source[c] += points[i].source[c] / n;
            target[c] += points[i].target[c] / n;
        }
    }
    let mut den = 0.;
    let mut real = 0.;
    let mut imaginary = 0.;
    for &i in &previous.inliers {
        if cancel() {
            return Err(GeometryError::Cancelled);
        }
        let s = [points[i].source[0] - source[0], points[i].source[1] - source[1]];
        let t = [points[i].target[0] - target[0], points[i].target[1] - target[1]];
        den += s[0] * s[0] + s[1] * s[1];
        real += s[0] * t[0] + s[1] * t[1];
        imaginary += s[0] * t[1] - s[1] * t[0];
    }
    let a = real / den;
    let b = imaginary / den;
    let transform = Transform {
        a,
        b,
        translation: [
            target[0] - a * source[0] + b * source[1],
            target[1] - b * source[0] - a * source[1],
        ],
    };
    if !den.is_finite()
        || den <= 0.
        || [a, b, transform.translation[0], transform.translation[1]]
            .iter()
            .any(|v| !v.is_finite())
        || a.hypot(b) == 0.
    {
        return copy_evidence(previous);
    }
    let mut inliers = Vec::new();
    inliers
        .try_reserve_exact(points.len())
        .map_err(|_| GeometryError::Budget)?;
    let mut squared_error = 0.;
    for (i, p) in points.iter().enumerate() {
        if cancel() {
            return Err(GeometryError::Cancelled);
        }
        let q = transform.apply(p.source);
        let error = (q[0] - p.target[0]).powi(2) + (q[1] - p.target[1]).powi(2);
        if error.is_finite() && error <= tolerance2 {
            inliers.push(i);
            squared_error += error;
        }
    }
    if !squared_error.is_finite() {
        return copy_evidence(previous);
    }
    Ok(GeometryEvidence {
        transform,
        inliers,
        squared_error,
    })
}

/// Reflection followed by scale, rotation and translation.
/// Inner support indices refer to the original correspondence ordering, while
/// its transform consumes coordinates with the source x coordinate negated.
/// Use `apply` to map original source coordinates to target coordinates.
#[derive(Debug, Clone, PartialEq)]
pub struct ReflectedGeometryEvidence {
    pub similarity: GeometryEvidence,
}
impl ReflectedGeometryEvidence {
    pub fn apply(&self, source: [f64; 2]) -> [f64; 2] {
        self.similarity.transform.apply([-source[0], source[1]])
    }
}

/// Verify orientation-reversing similarity models over descriptor-qualified
/// correspondences. Any planar reflection followed by a similarity can be
/// represented by this fixed x reflection plus the fitted rotation.
/// This is geometric evidence only; pixel verification is still required.
/// The hypothesis limit applies to this reflected search, independently of
/// any separate orientation-preserving search performed by the caller.
///
/// # Errors
/// Returns invalid input, work/allocation budget exhaustion or cancellation
/// without partial evidence. Source reflection storage is fallibly admitted.
pub fn verify_reflected_similarity(
    points: &[Correspondence],
    policy: GeometryPolicy,
    cancel: impl Fn() -> bool,
) -> Result<Option<ReflectedGeometryEvidence>, GeometryError> {
    Ok(verify_reflected_similarity_selected(points, policy, &cancel, false)?[0].take())
}

/// Reflected counterparts of `verify_similarity_candidates`, with support
/// indices and transforms retaining the same reflection semantics as
/// `verify_reflected_similarity`. At most two models share one hypothesis cap.
///
/// # Errors
/// Invalid input, exhausted work/storage or cancellation without partial models.
pub fn verify_reflected_similarity_candidates(
    points: &[Correspondence],
    policy: GeometryPolicy,
    cancel: impl Fn() -> bool,
) -> Result<[Option<ReflectedGeometryEvidence>; 2], GeometryError> {
    verify_reflected_similarity_selected(points, policy, &cancel, true)
}

fn verify_reflected_similarity_selected(
    points: &[Correspondence],
    policy: GeometryPolicy,
    cancel: &impl Fn() -> bool,
    retain_robust: bool,
) -> Result<[Option<ReflectedGeometryEvidence>; 2], GeometryError> {
    validate(points, policy, cancel)?;
    let mut reflected = Vec::new();
    reflected
        .try_reserve_exact(points.len())
        .map_err(|_| GeometryError::Budget)?;
    for point in points {
        if cancel() {
            return Err(GeometryError::Cancelled);
        }
        reflected.push(Correspondence {
            source: [-point.source[0], point.source[1]],
            target: point.target,
        });
    }
    let evidence = verify_similarity_selected(&reflected, policy, cancel, retain_robust)?;
    if cancel() {
        return Err(GeometryError::Cancelled);
    }
    Ok(evidence.map(|model| model.map(|similarity| ReflectedGeometryEvidence { similarity })))
}

/// Explicit deterministic sampling effort, independent of final pixel admission.
/// Sampling is approximate and does not certify confidence or exhaustive recovery.
#[derive(Debug, Clone, Copy)]
pub struct ProjectiveSamplingPolicy {
    pub trials: u64,
    /// Nonzero reproducible xorshift state. No statistical confidence is implied.
    pub seed: u64,
}

/// Fit projective models from bounded four-point samples, scoring every input.
/// Runs all requested trials; repeated sampled subsets consume work as well.
/// The exhaustive entrypoint retains its existing refusal semantics.
///
/// # Errors
/// Invalid/duplicate inputs, invalid sampling, resource refusal or cancellation.
#[allow(clippy::cast_possible_truncation)] // Draw is reduced below admitted usize length.
pub fn verify_projective_sampled(
    points: &[Correspondence],
    policy: GeometryPolicy,
    sampling: ProjectiveSamplingPolicy,
    cancel: impl Fn() -> bool,
) -> Result<Option<ProjectiveEvidence>, GeometryError> {
    verify_projective_sampled_selected(points,policy,sampling,cancel,|_|true)
}

/// Sample only models valid on both whole-image rectangles, including refinement.
/// Corner denominator signs must agree in each forward/inverse domain. This
/// selects geometry, without relaxing correspondence or pixel thresholds.
///
/// # Errors
/// Zero dimensions, invalid geometry policy, work limits or cancellation.
pub fn verify_projective_sampled_for_domains(
    points:&[Correspondence],policy:GeometryPolicy,sampling:ProjectiveSamplingPolicy,
    source:(u32,u32),target:(u32,u32),cancel:impl Fn()->bool,
)->Result<Option<ProjectiveEvidence>,GeometryError>{
    if source.0==0 || source.1==0 || target.0==0 || target.1==0 {return Err(GeometryError::Invalid);}
    verify_projective_sampled_selected(points,policy,sampling,cancel,|transform|{
        projective_domain_eligible(transform, source, target)
    })
}

fn projective_domain_eligible(
    transform: ProjectiveTransform,
    source: (u32, u32),
    target: (u32, u32),
) -> bool {
    let Ok(inverse) = transform.inverse() else { return false; };
    let valid = |model: ProjectiveTransform, (w, h): (u32, u32)| {
        let values = [[0., 0.], [f64::from(w - 1), 0.], [0., f64::from(h - 1)],
            [f64::from(w - 1), f64::from(h - 1)]]
            .map(|[x, y]| model.matrix[2][0] * x + model.matrix[2][1] * y + model.matrix[2][2]);
        values.iter().all(|v| v.is_finite() && *v != 0.
            && v.is_sign_positive() == values[0].is_sign_positive())
    };
    valid(transform, source) && valid(inverse, target)
}

fn verify_projective_sampled_selected(
    points:&[Correspondence],policy:GeometryPolicy,sampling:ProjectiveSamplingPolicy,
    cancel:impl Fn()->bool,eligible:impl Fn(ProjectiveTransform)->bool,
)->Result<Option<ProjectiveEvidence>,GeometryError>{
    let tolerance2 = validate(points, policy, &cancel)?;
    if policy.min_inliers < 4 || sampling.trials == 0 || sampling.seed == 0 {
        return Err(GeometryError::Invalid);
    }
    if sampling.trials > policy.max_hypotheses {
        return Err(GeometryError::Budget);
    }
    if points.len() < 4 {
        return Ok(None);
    }
    let mut state = sampling.seed;
    let mut best: Option<ProjectiveEvidence> = None;
    for trial in 0..sampling.trials {
        if cancel() {
            return Err(GeometryError::Cancelled);
        }
        let mut selected = [0usize; 4];
        for i in 0..4 {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            let remaining = u64::try_from(points.len() - i).map_err(|_| GeometryError::Budget)?;
            let mut draw = (state % remaining) as usize;
            // Map the reduced draw into the complement of sorted prior choices.
            selected[..i].sort_unstable();
            for previous in &selected[..i] {
                if draw >= *previous {
                    draw += 1;
                }
            }
            selected[i] = draw;
        }
        let transform = match fit_projective_four(&selected.map(|i| points[i]), &cancel) {
            Ok(h) => h,
            Err(GeometryError::Invalid) => continue,
            Err(e) => return Err(e),
        };
        if !eligible(transform) {continue;}
        let mut inliers = Vec::new();
        inliers
            .try_reserve_exact(points.len())
            .map_err(|_| GeometryError::Budget)?;
        let mut squared_error = 0.0;
        for (index, point) in points.iter().enumerate() {
            if cancel() {
                return Err(GeometryError::Cancelled);
            }
            let Some(predicted) = transform.apply(point.source) else {
                continue;
            };
            let residual =
                (predicted[0] - point.target[0]).powi(2) + (predicted[1] - point.target[1]).powi(2);
            if residual.is_finite() && residual <= tolerance2 {
                inliers.push(index);
                squared_error += residual;
            }
        }
        if !squared_error.is_finite() {
            return Err(GeometryError::Invalid);
        }
        if inliers.len() < policy.min_inliers || !spread(points, &inliers, &cancel)? {
            continue;
        }
        if best.as_ref().is_none_or(|old| {
            inliers.len() > old.inliers.len()
                || (inliers.len() == old.inliers.len() && squared_error < old.squared_error)
        }) {
            best = Some(ProjectiveEvidence {
                transform,
                inliers,
                squared_error,
                hypotheses: trial + 1,
            });
        }
    }
    refine_projective_evidence(points, policy, tolerance2, best, sampling.trials, &cancel, &eligible)
}
