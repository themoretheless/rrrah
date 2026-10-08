//! Explicit opaque, display-range linear RGB color fitting. A fit is never
//! identity evidence. Callers own geometric correspondence and disjoint
//! training/validation selection; predictions are never clipped.
#[derive(Debug, Clone, Copy)]
pub struct RgbPair {
    pub source: [f64; 3],
    pub target: [f64; 3],
}
#[derive(Debug, Clone, Copy)]
pub struct AffineColorPolicy {
    pub minimum_samples: usize,
    pub maximum_samples: usize,
    pub minimum_variance: f64,
    /// Every elimination pivot must exceed this fraction of the initial
    /// largest matrix entry. This is a rank safeguard, not a condition number.
    pub minimum_relative_pivot: f64,
    pub maximum_coefficient: f64,
    pub maximum_offset: f64,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AffineColorModel {
    pub matrix: [[f64; 3]; 3],
    pub offset: [f64; 3],
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum AffineColorError {
    #[error("invalid display-range RGB input or policy")]
    Invalid,
    #[error("insufficient samples or color variation")]
    Uninformative,
    #[error("rank-deficient color transform")]
    IllConditioned,
    #[error("color coefficient bounds exceeded")]
    Bounds,
    #[error("sample work budget exceeded")]
    Budget,
    #[error("color fitting cancelled")]
    Cancelled,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ColorValidation {
    pub samples: usize,
    pub matched: usize,
    pub squared_error: f64,
}
fn valid_pair(p: &RgbPair) -> bool {
    p.source
        .iter()
        .chain(&p.target)
        .all(|v| v.is_finite() && (0.0..=1.0).contains(v))
}
fn validate(policy: AffineColorPolicy, samples: usize) -> Result<(), AffineColorError> {
    if policy.minimum_samples < 4
        || policy.maximum_samples < policy.minimum_samples
        || policy.maximum_samples > 1_000_000
        || !policy.minimum_variance.is_finite()
        || policy.minimum_variance <= 0.0
        || !policy.minimum_relative_pivot.is_finite()
        || !(0.0..1.0).contains(&policy.minimum_relative_pivot)
        || policy.minimum_relative_pivot == 0.0
        || !policy.maximum_coefficient.is_finite()
        || policy.maximum_coefficient <= 0.0
        || !policy.maximum_offset.is_finite()
        || policy.maximum_offset < 0.0
    {
        return Err(AffineColorError::Invalid);
    }
    if samples > policy.maximum_samples {
        return Err(AffineColorError::Budget);
    }
    if samples < policy.minimum_samples {
        return Err(AffineColorError::Uninformative);
    }
    Ok(())
}
fn solve(
    mut a: [[f64; 3]; 3],
    mut b: [[f64; 3]; 3],
    relative: f64,
    cancel: &impl Fn() -> bool,
) -> Result<[[f64; 3]; 3], AffineColorError> {
    let scale = a.iter().flatten().fold(0.0_f64, |v, x| v.max(x.abs()));
    if !scale.is_finite() || scale == 0.0 {
        return Err(AffineColorError::IllConditioned);
    }
    for c in 0..3 {
        if cancel() {
            return Err(AffineColorError::Cancelled);
        }
        let pivot = (c..3)
            .max_by(|&i, &j| a[i][c].abs().total_cmp(&a[j][c].abs()))
            .unwrap();
        if a[pivot][c].abs() <= scale * relative {
            return Err(AffineColorError::IllConditioned);
        }
        a.swap(c, pivot);
        b.swap(c, pivot);
        let divisor = a[c][c];
        for j in 0..3 {
            a[c][j] /= divisor;
            b[c][j] /= divisor;
        }
        for i in 0..3 {
            if i != c {
                let factor = a[i][c];
                for j in 0..3 {
                    a[i][j] -= factor * a[c][j];
                    b[i][j] -= factor * b[c][j];
                }
            }
        }
    }
    if !b.iter().flatten().all(|v| v.is_finite()) {
        return Err(AffineColorError::IllConditioned);
    }
    Ok(b)
}
impl AffineColorModel {
    #[must_use]
    pub fn apply(self, source: [f64; 3]) -> [f64; 3] {
        std::array::from_fn(|i| self.offset[i] + (0..3).map(|j| self.matrix[i][j] * source[j]).sum::<f64>())
    }
}
/// Fit with constant auxiliary storage and bounded work. RGB pairs are opaque,
/// finite linear-light values in0..=1; HDR/alpha policies belong to the caller.
/// A successful model still requires independent held-out residual evidence.
#[allow(clippy::cast_precision_loss)] // At most1M samples.
pub fn fit_affine_color(
    pairs: &[RgbPair],
    policy: AffineColorPolicy,
    cancel: impl Fn() -> bool,
) -> Result<AffineColorModel, AffineColorError> {
    if cancel() {
        return Err(AffineColorError::Cancelled);
    }
    validate(policy, pairs.len())?;
    let mut source = [0.0; 3];
    let mut target = [0.0; 3];
    let mut ss = [[0.0; 3]; 3];
    let mut st = [[0.0; 3]; 3];
    let mut tt = [0.0; 3];
    for (index, pair) in pairs.iter().enumerate() {
        if cancel() {
            return Err(AffineColorError::Cancelled);
        }
        if !valid_pair(pair) {
            return Err(AffineColorError::Invalid);
        }
        let count = (index + 1) as f64;
        let ds: [f64; 3] = std::array::from_fn(|i| pair.source[i] - source[i]);
        let dt: [f64; 3] = std::array::from_fn(|i| pair.target[i] - target[i]);
        for i in 0..3 {
            source[i] += ds[i] / count;
            target[i] += dt[i] / count;
        }
        for i in 0..3 {
            tt[i] += dt[i] * (pair.target[i] - target[i]);
            for j in 0..3 {
                ss[i][j] += ds[i] * (pair.source[j] - source[j]);
                st[i][j] += ds[i] * (pair.target[j] - target[j]);
            }
        }
    }
    for i in 0..3 {
        if ss[i][i] / (pairs.len() as f64) < policy.minimum_variance
            || tt[i] / (pairs.len() as f64) < policy.minimum_variance
        {
            return Err(AffineColorError::Uninformative);
        }
    }
    let columns = solve(ss, st, policy.minimum_relative_pivot, &cancel)?;
    let matrix = std::array::from_fn(|i| std::array::from_fn(|j| columns[j][i]));
    let offset = std::array::from_fn(|i| target[i] - (0..3).map(|j| matrix[i][j] * source[j]).sum::<f64>());
    if matrix
        .iter()
        .flatten()
        .any(|v| v.abs() > policy.maximum_coefficient)
        || offset
            .iter()
            .any(|v| !v.is_finite() || v.abs() > policy.maximum_offset)
    {
        return Err(AffineColorError::Bounds);
    }
    let identity = [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]];
    let _ = solve(matrix, identity, policy.minimum_relative_pivot, &cancel)?;
    if cancel() {
        return Err(AffineColorError::Cancelled);
    }
    Ok(AffineColorModel { matrix, offset })
}
/// Score separately supplied held-out pairs. Never turns a score into a copy
/// decision. The caller must establish disjoint sampling and geometric support.
pub fn validate_affine_color(
    model: AffineColorModel,
    pairs: &[RgbPair],
    tolerance: f64,
    maximum_samples: usize,
    cancel: impl Fn() -> bool,
) -> Result<ColorValidation, AffineColorError> {
    if cancel() {
        return Err(AffineColorError::Cancelled);
    }
    if !tolerance.is_finite()
        || tolerance < 0.0
        || !model
            .matrix
            .iter()
            .flatten()
            .chain(&model.offset)
            .all(|v| v.is_finite())
    {
        return Err(AffineColorError::Invalid);
    }
    if pairs.len() > maximum_samples {
        return Err(AffineColorError::Budget);
    }
    if pairs.is_empty() {
        return Err(AffineColorError::Uninformative);
    }
    let mut evidence = ColorValidation {
        samples: pairs.len(),
        matched: 0,
        squared_error: 0.0,
    };
    for pair in pairs {
        if cancel() {
            return Err(AffineColorError::Cancelled);
        }
        if !valid_pair(pair) {
            return Err(AffineColorError::Invalid);
        }
        let expected = model.apply(pair.source);
        let mut matched = true;
        for i in 0..3 {
            let delta = expected[i] - pair.target[i];
            if !delta.is_finite() {
                return Err(AffineColorError::Invalid);
            }
            matched &= delta.abs() <= tolerance;
            evidence.squared_error += delta * delta;
        }
        if !evidence.squared_error.is_finite() {
            return Err(AffineColorError::Invalid);
        }
        evidence.matched += usize::from(matched);
    }
    if cancel() {
        return Err(AffineColorError::Cancelled);
    }
    Ok(evidence)
}
