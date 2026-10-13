//! Ordinal agreement against predeclared translated target competitors.
//! Common support and source-defined informative pairs avoid denominator selection.
use crate::{
    geometry::{ProjectiveTransform, projective_domain_eligible},
    linear::LinearRgbaView,
    rank_region::{RankRegionError, RankRegionPolicy, interpolated, luminance},
};
pub const MAX_RANK_COMPETITORS: usize = 16;
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RankCompetitorScore {
    pub offset: [i32; 2],
    pub agreeing_pairs: u64,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompetitiveRankEvidence {
    pub sites: u64,
    pub valid_sites: u64,
    /// Only source contrast defines this shared denominator for every candidate.
    pub informative_pairs: u64,
    pub baseline_agreeing_pairs: u64,
    pub competitor_count: usize,
    pub competitors: [RankCompetitorScore; MAX_RANK_COMPETITORS],
    pub best_competitor_agreeing_pairs: u64,
    /// Conservative attempted tap reads, including abandoned edge sites.
    pub pixel_reads: u64,
}
/// Compare the supplied mapping with translations fixed by the caller before
/// observing scores. Every candidate uses the same warped source template,
/// source-defined informative pairs and common valid sites. Target pairs with
/// too little contrast count as nonagreement, not removed denominators. Source
/// and target taps use opaque display-range linear RGB and the same box filter.
///
/// Each offset translates only target samples, leaving source samples fixed.
/// A site participates only if all nine source/target contexts and every
/// competitor context are within image bounds. Thus edges can leave insufficient
/// common support; this refuses evidence rather than proving non-copying.
/// Counts describe only the selected target region. Neither a margin nor ordinal
/// agreement establishes copying, pixel equality or statistical independence.
/// No allocation, model fitting, threshold decision or competitor selection.
///
/// # Errors
/// Invalid geometry/rectangle/policy, empty/duplicate/zero/too many offsets,
/// cumulative work refusal, unsupported pixels, insufficient source contrast or
/// cancellation; no partial evidence is returned.
#[allow(clippy::too_many_arguments)]
pub fn compare_competitive_rank_region(
    source: &LinearRgbaView<'_>,
    target: &LinearRgbaView<'_>,
    model: ProjectiveTransform,
    region: [u32; 4],
    offsets: &[[i32; 2]],
    policy: RankRegionPolicy,
    filter_radius: u32,
    cancel: impl Fn() -> bool,
) -> Result<CompetitiveRankEvidence, RankRegionError> {
    if cancel() {
        return Err(RankRegionError::Cancelled);
    }
    let (tw, th) = target.dimensions();
    if region[2] == 0
        || region[3] == 0
        || !region[0].checked_add(region[2]).is_some_and(|x| x <= tw)
        || !region[1].checked_add(region[3]).is_some_and(|y| y <= th)
        || offsets.is_empty()
        || offsets.len() > MAX_RANK_COMPETITORS
        || policy.radius == 0
        || policy.minimum_pairs == 0
        || !policy.minimum_contrast.is_finite()
        || policy.minimum_contrast <= 0.
        || policy.minimum_contrast > 1.
        || !projective_domain_eligible(model, source.dimensions(), target.dimensions())
    {
        return Err(RankRegionError::Invalid);
    }
    for (i, offset) in offsets.iter().enumerate() {
        if cancel() {
            return Err(RankRegionError::Cancelled);
        }
        if *offset == [0, 0] || offsets[..i].contains(offset) {
            return Err(RankRegionError::Invalid);
        }
    }
    let inverse = model.inverse().map_err(|_| RankRegionError::Invalid)?;
    let sites = u64::from(region[2]) * u64::from(region[3]);
    let side = u64::from(filter_radius) * 2 + 1;
    let reads = side
        .checked_mul(side)
        .and_then(|t| t.checked_mul(9))
        .and_then(|t| t.checked_mul(offsets.len() as u64 + 5))
        .and_then(|t| t.checked_mul(sites))
        .ok_or(RankRegionError::Budget)?;
    if sites > policy.maximum_sites || reads > policy.maximum_pixel_reads {
        return Err(RankRegionError::Budget);
    }
    let mut evidence = CompetitiveRankEvidence {
        sites,
        valid_sites: 0,
        informative_pairs: 0,
        baseline_agreeing_pairs: 0,
        competitor_count: offsets.len(),
        competitors: [RankCompetitorScore::default(); MAX_RANK_COMPETITORS],
        best_competitor_agreeing_pairs: 0,
        pixel_reads: 0,
    };
    for (score, offset) in evidence.competitors.iter_mut().zip(offsets) {
        score.offset = *offset;
    }
    let r = i64::from(policy.radius);
    let filter = i64::from(filter_radius);
    let neighbors = [
        (0, 0),
        (-r, -r),
        (0, -r),
        (r, -r),
        (-r, 0),
        (r, 0),
        (-r, r),
        (0, r),
        (r, r),
    ];
    let taps = (side * side) as f64;
    for y in region[1]..region[1] + region[3] {
        'site: for x in region[0]..region[0] + region[2] {
            if cancel() {
                return Err(RankRegionError::Cancelled);
            }
            let mut template = [0.; 9];
            let mut candidates = [[0.; 9]; MAX_RANK_COMPETITORS + 1];
            for (j, (dx, dy)) in neighbors.iter().enumerate() {
                for fy in -filter..=filter {
                    for fx in -filter..=filter {
                        if cancel() {
                            return Err(RankRegionError::Cancelled);
                        }
                        let tx = i64::from(x) + dx + fx;
                        let ty = i64::from(y) + dy + fy;
                        if tx < 0 || ty < 0 || tx >= i64::from(tw) || ty >= i64::from(th) {
                            continue 'site;
                        }
                        // Bounds-check all competitors before sampling this tap.
                        for offset in offsets {
                            let ox = tx + i64::from(offset[0]);
                            let oy = ty + i64::from(offset[1]);
                            if ox < 0 || oy < 0 || ox >= i64::from(tw) || oy >= i64::from(th) {
                                continue 'site;
                            }
                        }
                        let p = inverse
                            .apply([tx as f64, ty as f64])
                            .ok_or(RankRegionError::Invalid)?;
                        evidence.pixel_reads += 4;
                        let Some(value) = interpolated(source, p)? else {
                            continue 'site;
                        };
                        template[j] += value;
                        evidence.pixel_reads += 1;
                        candidates[0][j] += luminance(target, tx as u32, ty as u32)?;
                        for (k, offset) in offsets.iter().enumerate() {
                            if cancel() {
                                return Err(RankRegionError::Cancelled);
                            }
                            evidence.pixel_reads += 1;
                            candidates[k + 1][j] += luminance(
                                target,
                                (tx + i64::from(offset[0])) as u32,
                                (ty + i64::from(offset[1])) as u32,
                            )?;
                        }
                    }
                }
                template[j] /= taps;
                for values in candidates.iter_mut().take(offsets.len() + 1) {
                    values[j] /= taps;
                }
            }
            evidence.valid_sites += 1;
            for j in 1..9 {
                if cancel() {
                    return Err(RankRegionError::Cancelled);
                }
                let a = template[j] - template[0];
                if a.abs() < policy.minimum_contrast {
                    continue;
                }
                evidence.informative_pairs += 1;
                for (k, values) in candidates.iter().take(offsets.len() + 1).enumerate() {
                    let b = values[j] - values[0];
                    if b.abs() >= policy.minimum_contrast && (a > 0.) == (b > 0.) {
                        if k == 0 {
                            evidence.baseline_agreeing_pairs += 1;
                        } else {
                            evidence.competitors[k - 1].agreeing_pairs += 1;
                        }
                    }
                }
            }
        }
    }
    if cancel() {
        return Err(RankRegionError::Cancelled);
    }
    if evidence.informative_pairs < policy.minimum_pairs {
        return Err(RankRegionError::Uninformative);
    }
    evidence.best_competitor_agreeing_pairs = evidence.competitors[..offsets.len()]
        .iter()
        .map(|s| s.agreeing_pairs)
        .max()
        .unwrap_or(0);
    Ok(evidence)
}
