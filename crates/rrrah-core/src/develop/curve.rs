// SPDX-License-Identifier: Apache-2.0
// Adapted from storytold/lightcraft, commit 294012742e277d95e59db0072c88bfd3d296f6cc.
// Modified for Rrrah: strict validation, sensor clipping masks, memory admission,
// sequential cancellable kernels and the existing scene-linear display path.
//! Curves for tone mapping: monotone cubic Hermite splines (Fritsch–Carlson) and GPU Hermite knots.
//!
//! Monotone interpolation never overshoots between control points, so a point curve can't invert
//! tones by accident (a common complaint with natural cubic splines).

/// A curve through control points in `0..1 × 0..1` (x strictly increasing; spacing at least 1e-6).
#[derive(Clone, Debug, PartialEq)]
pub struct MonotoneCurve {
    xs: Vec<f64>,
    ys: Vec<f64>,
    ms: Vec<f64>,
}

impl MonotoneCurve {
    /// Build from ordered monotone points; invalid input is rejected without normalization.
    pub fn new(points: &[(f64, f64)]) -> Result<MonotoneCurve, super::DevelopError> {
        if points.len() < 2
            || points.len() > 256
            || points.iter().any(|(x, y)| {
                !x.is_finite() || !y.is_finite() || !(0.0..=1.0).contains(x) || !(0.0..=1.0).contains(y)
            })
            || points
                .windows(2)
                .any(|p| p[1].0 - p[0].0 < 1e-6 || p[0].1 > p[1].1)
        {
            return Err(super::DevelopError::Invalid(
                "curve needs 2..=256 finite monotone points in [0,1], x spacing >= 1e-6",
            ));
        }
        let p = points.to_vec();
        let n = p.len();
        let xs: Vec<f64> = p.iter().map(|q| q.0).collect();
        let ys: Vec<f64> = p.iter().map(|q| q.1).collect();
        let d: Vec<f64> = (0..n - 1)
            .map(|i| (ys[i + 1] - ys[i]) / (xs[i + 1] - xs[i]))
            .collect();
        let mut ms = vec![0.0; n];
        ms[0] = d[0];
        ms[n - 1] = d[n - 2];
        for i in 1..n - 1 {
            ms[i] = if d[i - 1] * d[i] <= 0.0 {
                0.0
            } else {
                d[i - 1] * 0.5 + d[i] * 0.5
            };
        }
        // Fritsch–Carlson limiter where the data is monotone within a segment.
        for i in 0..n - 1 {
            if d[i] == 0.0 {
                ms[i] = 0.0;
                ms[i + 1] = 0.0;
                continue;
            }
            let a = ms[i] / d[i];
            let b = ms[i + 1] / d[i];
            let s = a * a + b * b;
            if s > 9.0 {
                let t = 3.0 / s.sqrt();
                ms[i] = t * a * d[i];
                ms[i + 1] = t * b * d[i];
            }
        }
        Ok(MonotoneCurve { xs, ys, ms })
    }

    pub fn identity() -> MonotoneCurve {
        Self {
            xs: vec![0.0, 1.0],
            ys: vec![0.0, 1.0],
            ms: vec![1.0, 1.0],
        }
    }

    pub fn eval(&self, x: f64) -> f64 {
        if x.is_nan() {
            return f64::NAN;
        }
        let n = self.xs.len();
        if x <= self.xs[0] {
            return self.ys[0];
        }
        if x >= self.xs[n - 1] {
            return self.ys[n - 1];
        }
        let i = match self.xs.binary_search_by(|v| v.total_cmp(&x)) {
            Ok(i) => return self.ys[i],
            Err(i) => i - 1,
        };
        let h = self.xs[i + 1] - self.xs[i];
        let t = (x - self.xs[i]) / h;
        let (t2, t3) = (t * t, t * t * t);
        (2.0 * t3 - 3.0 * t2 + 1.0) * self.ys[i]
            + (t3 - 2.0 * t2 + t) * h * self.ms[i]
            + (-2.0 * t3 + 3.0 * t2) * self.ys[i + 1]
            + (t3 - t2) * h * self.ms[i + 1]
    }

    /// Hermite knots for GPU uniforms: x, y, derivative, reserved.
    pub fn gpu_knots(&self) -> Vec<[f32; 4]> {
        self.xs
            .iter()
            .zip(&self.ys)
            .zip(&self.ms)
            .map(|((&x, &y), &m)| [x as f32, y as f32, m as f32, 0.0])
            .collect()
    }

    pub fn points(&self) -> Vec<(f64, f64)> {
        self.xs.iter().copied().zip(self.ys.iter().copied()).collect()
    }
}
