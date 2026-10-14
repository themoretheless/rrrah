use std::{cell::Cell,hint::black_box,time::Instant};
#[derive(Clone,Copy,Debug,PartialEq)]struct GradientDescriptor([f64;128]);
#[derive(Clone,Copy,Debug,PartialEq)]struct GradientFeature{position:[f64;2],descriptor:GradientDescriptor}
#[derive(Clone,Copy)]struct GradientMatchPolicy{max_comparisons:u64,max_squared_distance:f64,squared_ratio:f64}
#[derive(Debug,PartialEq)]enum LocalError{Budget,Invalid,Cancelled}
mod geometry{#[derive(Debug,PartialEq)]pub struct Correspondence{pub source:[f64;2],pub target:[f64;2]}}
fn baseline_distances(a: &GradientDescriptor, b: &[GradientFeature]) -> [f64; 4] {
    // No repeated/phantom comparisons in partial blocks.
    if b.len() < 4 {
        let mut output = [0.; 4];
        for (lane, b) in b.iter().enumerate() {
            output[lane] =
                a.0.iter()
                    .zip(&b.descriptor.0)
                    .map(|(x, y)| (x - y) * (x - y))
                    .sum();
        }
        return output;
    }
    let last = 3;
    let b0 = &b[0].descriptor.0;
    let b1 = &b[1.min(last)].descriptor.0;
    let b2 = &b[2.min(last)].descriptor.0;
    let b3 = &b[3.min(last)].descriptor.0;
    let (mut s0, mut s1, mut s2, mut s3) = (0., 0., 0., 0.);
    for base in (0..128).step_by(4) {
        {
            let k = base + 0;
            let x = a.0[k];
            let d0 = x - b0[k];
            s0 += d0 * d0;
            let d1 = x - b1[k];
            s1 += d1 * d1;
            let d2 = x - b2[k];
            s2 += d2 * d2;
            let d3 = x - b3[k];
            s3 += d3 * d3;
        }
        {
            let k = base + 1;
            let x = a.0[k];
            let d0 = x - b0[k];
            s0 += d0 * d0;
            let d1 = x - b1[k];
            s1 += d1 * d1;
            let d2 = x - b2[k];
            s2 += d2 * d2;
            let d3 = x - b3[k];
            s3 += d3 * d3;
        }
        {
            let k = base + 2;
            let x = a.0[k];
            let d0 = x - b0[k];
            s0 += d0 * d0;
            let d1 = x - b1[k];
            s1 += d1 * d1;
            let d2 = x - b2[k];
            s2 += d2 * d2;
            let d3 = x - b3[k];
            s3 += d3 * d3;
        }
        {
            let k = base + 3;
            let x = a.0[k];
            let d0 = x - b0[k];
            s0 += d0 * d0;
            let d1 = x - b1[k];
            s1 += d1 * d1;
            let d2 = x - b2[k];
            s2 += d2 * d2;
            let d3 = x - b3[k];
            s3 += d3 * d3;
        }
    }
    [s0, s1, s2, s3]
}

fn baseline(
    left: &[GradientFeature],
    right: &[GradientFeature],
    policy: GradientMatchPolicy,
    distinct_radius_squared: Option<f64>,
    cancel: impl Fn() -> bool,
) -> Result<Vec<crate::geometry::Correspondence>, LocalError> {
    if !policy.max_squared_distance.is_finite()
        || !(0. ..=4.).contains(&policy.max_squared_distance)
        || !policy.squared_ratio.is_finite()
        || policy.squared_ratio <= 0.
        || policy.squared_ratio >= 1.
    {
        return Err(LocalError::Invalid);
    }
    if cancel() {
        return Err(LocalError::Cancelled);
    }
    let work = u64::try_from(left.len())
        .ok()
        .and_then(|a| u64::try_from(right.len()).ok().and_then(|b| a.checked_mul(b)))
        .ok_or(LocalError::Budget)?;
    let work = work
        .checked_mul(if distinct_radius_squared.is_some() { 2 } else { 1 })
        .ok_or(LocalError::Budget)?;
    if work > policy.max_comparisons {
        return Err(LocalError::Budget);
    }
    for feature in left.iter().chain(right) {
        if cancel() {
            return Err(LocalError::Cancelled);
        }
        let d = &feature.descriptor.0;
        if feature.position.iter().any(|v| !v.is_finite())
            || d.iter().any(|v| !v.is_finite() || *v < 0.)
            || (d.iter().map(|v| v * v).sum::<f64>() - 1.).abs() > 1e-6
        {
            return Err(LocalError::Invalid);
        }
    }
    if left.is_empty() || right.is_empty() {
        if cancel() {
            return Err(LocalError::Cancelled);
        }
        return Ok(Vec::new());
    }
    let allocate = |len| {
        let mut v = Vec::new();
        v.try_reserve_exact(len).map_err(|_| LocalError::Budget)?;
        v.resize(len, (f64::INFINITY, f64::INFINITY, usize::MAX));
        Ok::<_, LocalError>(v)
    };
    let mut forward = allocate(left.len())?;
    let mut reverse = allocate(right.len())?;
 let mut forward_runner=vec![usize::MAX;left.len()];let mut reverse_runner=vec![usize::MAX;right.len()];
 let mut forward_resolved=vec![false;left.len()];let mut reverse_resolved=vec![false;right.len()];

    let update=|best:&mut(f64,f64,usize),runner:&mut usize,distance:f64,index:usize|{
 if distance<best.0{*runner=best.2;*best=(distance,best.0,index);}else if distance<best.1{best.1=distance;*runner=index;}
 };
    for (i, a) in left.iter().enumerate() {
        for (block, chunk) in right.chunks(4).enumerate() {
            for _ in chunk {
                if cancel() {
                    return Err(LocalError::Cancelled);
                }
            }
            let distances = baseline_distances(&a.descriptor, chunk);
            for (lane, _b) in chunk.iter().enumerate() {
                let j = block * 4 + lane;

                let distance = distances[lane];
                update(&mut forward[i], &mut forward_runner[i],distance, j);
                update(&mut reverse[j], &mut reverse_runner[j],distance, i);
            }
        }
    }
    if let Some(radius) = distinct_radius_squared {
        for i in 0..forward.len(){let runner=forward_runner[i];let best=&mut forward[i];if runner!=usize::MAX{let a=right[best.2].position;let b=right[runner].position;forward_resolved[i]=(a[0]-b[0]).powi(2)+(a[1]-b[1]).powi(2)>radius;}if !forward_resolved[i]{best.1=f64::INFINITY;}}
        for j in 0..reverse.len(){let runner=reverse_runner[j];let best=&mut reverse[j];if runner!=usize::MAX{let a=left[best.2].position;let b=left[runner].position;reverse_resolved[j]=(a[0]-b[0]).powi(2)+(a[1]-b[1]).powi(2)>radius;}if !reverse_resolved[j]{best.1=f64::INFINITY;}}
        for (i, a) in left.iter().enumerate() {
            for (block, chunk) in right.chunks(4).enumerate() {
                for _ in chunk {
                    if cancel() {
                        return Err(LocalError::Cancelled);
                    }
                }
                if forward_resolved[i] && (block*4..block*4+chunk.len()).all(|j|reverse_resolved[j]){continue;}
                let distances = baseline_distances(&a.descriptor, chunk);
                for (lane, b) in chunk.iter().enumerate() {
                    let j = block * 4 + lane;

                    let distance = distances[lane];
                    let nearest = right[forward[i].2].position;
                    if !forward_resolved[i] && ((b.position[0] - nearest[0]).powi(2) + (b.position[1] - nearest[1]).powi(2) > radius) {
                        forward[i].1 = forward[i].1.min(distance);
                    }
                    let nearest = left[reverse[j].2].position;
                    if !reverse_resolved[j] && ((a.position[0] - nearest[0]).powi(2) + (a.position[1] - nearest[1]).powi(2) > radius) {
                        reverse[j].1 = reverse[j].1.min(distance);
                    }
                }
            }
        }
    }
    let mut output = Vec::new();
    output
        .try_reserve_exact(left.len().min(right.len()))
        .map_err(|_| LocalError::Budget)?;
    for (i, &(distance, second, j)) in forward.iter().enumerate() {
        if cancel() {
            return Err(LocalError::Cancelled);
        }
        if (distinct_radius_squared.is_some() && !second.is_finite())
            || distance > policy.max_squared_distance
            || distance >= second * policy.squared_ratio
        {
            continue;
        }
        let (back, next, index) = reverse[j];
        if index == i
            && back < next * policy.squared_ratio
            && (distinct_radius_squared.is_none() || next.is_finite())
        {
            output.push(crate::geometry::Correspondence {
                source: left[i].position,
                target: right[j].position,
            });
        }
    }
    if cancel() {
        return Err(LocalError::Cancelled);
    }
    Ok(output)
}

fn candidate(
    left: &[GradientFeature],
    right: &[GradientFeature],
    policy: GradientMatchPolicy,
    distinct_radius_squared: Option<f64>,
    cancel: impl Fn() -> bool,
) -> Result<Vec<crate::geometry::Correspondence>, LocalError> {
    if !policy.max_squared_distance.is_finite()
        || !(0. ..=4.).contains(&policy.max_squared_distance)
        || !policy.squared_ratio.is_finite()
        || policy.squared_ratio <= 0.
        || policy.squared_ratio >= 1.
    {
        return Err(LocalError::Invalid);
    }
    if cancel() {
        return Err(LocalError::Cancelled);
    }
    let work = u64::try_from(left.len())
        .ok()
        .and_then(|a| u64::try_from(right.len()).ok().and_then(|b| a.checked_mul(b)))
        .ok_or(LocalError::Budget)?;
    let work = work
        .checked_mul(if distinct_radius_squared.is_some() { 2 } else { 1 })
        .ok_or(LocalError::Budget)?;
    if work > policy.max_comparisons {
        return Err(LocalError::Budget);
    }
    for feature in left.iter().chain(right) {
        if cancel() {
            return Err(LocalError::Cancelled);
        }
        let d = &feature.descriptor.0;
        if feature.position.iter().any(|v| !v.is_finite())
            || d.iter().any(|v| !v.is_finite() || *v < 0.)
            || (d.iter().map(|v| v * v).sum::<f64>() - 1.).abs() > 1e-6
        {
            return Err(LocalError::Invalid);
        }
    }
    if left.is_empty() || right.is_empty() {
        if cancel() {
            return Err(LocalError::Cancelled);
        }
        return Ok(Vec::new());
    }
    let allocate = |len| {
        let mut v = Vec::new();
        v.try_reserve_exact(len).map_err(|_| LocalError::Budget)?;
        v.resize(len, (f64::INFINITY, f64::INFINITY, usize::MAX));
        Ok::<_, LocalError>(v)
    };
    let mut forward = allocate(left.len())?;
    let mut reverse = allocate(right.len())?;
 let mut forward_runner=vec![usize::MAX;left.len()];let mut reverse_runner=vec![usize::MAX;right.len()];
 let mut forward_resolved=vec![false;left.len()];let mut reverse_resolved=vec![false;right.len()];

    let update=|best:&mut(f64,f64,usize),runner:&mut usize,distance:f64,index:usize|{
 if distance<best.0{*runner=best.2;*best=(distance,best.0,index);}else if distance<best.1{best.1=distance;*runner=index;}
 };
    for (i, a) in left.iter().enumerate() {
        for (block, chunk) in right.chunks(4).enumerate() {
            for _ in chunk {
                if cancel() {
                    return Err(LocalError::Cancelled);
                }
            }
            let distances = baseline_distances(&a.descriptor, chunk);
            for (lane, _b) in chunk.iter().enumerate() {
                let j = block * 4 + lane;

                let distance = distances[lane];
                update(&mut forward[i], &mut forward_runner[i],distance, j);
                update(&mut reverse[j], &mut reverse_runner[j],distance, i);
            }
        }
    }
    if let Some(radius) = distinct_radius_squared {
        for i in 0..forward.len(){let runner=forward_runner[i];let best=&mut forward[i];if runner!=usize::MAX{let a=right[best.2].position;let b=right[runner].position;forward_resolved[i]=(a[0]-b[0]).powi(2)+(a[1]-b[1]).powi(2)>radius;}if !forward_resolved[i]{best.1=f64::INFINITY;}}
        for j in 0..reverse.len(){let runner=reverse_runner[j];let best=&mut reverse[j];if runner!=usize::MAX{let a=left[best.2].position;let b=left[runner].position;reverse_resolved[j]=(a[0]-b[0]).powi(2)+(a[1]-b[1]).powi(2)>radius;}if !reverse_resolved[j]{best.1=f64::INFINITY;}}
        for (i, a) in left.iter().enumerate() {
            for (block, chunk) in right.chunks(4).enumerate() {
                for _ in chunk {
                    if cancel() {
                        return Err(LocalError::Cancelled);
                    }
                }
                let distances = if forward_resolved[i] {
                    let mut needed = 0u8;
                    for lane in 0..chunk.len() {
                        if !reverse_resolved[block*4+lane] { needed |= 1 << lane; }
                    }
                    if needed == 0 { continue; }
                    if needed.count_ones() <= 2 {
                        let mut values = [0.;4];
                        for lane in 0..chunk.len() {
                            if needed & (1 << lane) != 0 {
                                values[lane] = a.descriptor.0.iter().zip(&chunk[lane].descriptor.0).map(|(x,y)|(x-y)*(x-y)).sum();
                            }
                        }
                        values
                    } else { baseline_distances(&a.descriptor,chunk) }
                } else { baseline_distances(&a.descriptor,chunk) };
                for (lane, b) in chunk.iter().enumerate() {
                    let j = block * 4 + lane;

                    let distance = distances[lane];
                    let nearest = right[forward[i].2].position;
                    if !forward_resolved[i] && ((b.position[0] - nearest[0]).powi(2) + (b.position[1] - nearest[1]).powi(2) > radius) {
                        forward[i].1 = forward[i].1.min(distance);
                    }
                    let nearest = left[reverse[j].2].position;
                    if !reverse_resolved[j] && ((a.position[0] - nearest[0]).powi(2) + (a.position[1] - nearest[1]).powi(2) > radius) {
                        reverse[j].1 = reverse[j].1.min(distance);
                    }
                }
            }
        }
    }
    let mut output = Vec::new();
    output
        .try_reserve_exact(left.len().min(right.len()))
        .map_err(|_| LocalError::Budget)?;
    for (i, &(distance, second, j)) in forward.iter().enumerate() {
        if cancel() {
            return Err(LocalError::Cancelled);
        }
        if (distinct_radius_squared.is_some() && !second.is_finite())
            || distance > policy.max_squared_distance
            || distance >= second * policy.squared_ratio
        {
            continue;
        }
        let (back, next, index) = reverse[j];
        if index == i
            && back < next * policy.squared_ratio
            && (distinct_radius_squared.is_none() || next.is_finite())
        {
            output.push(crate::geometry::Correspondence {
                source: left[i].position,
                target: right[j].position,
            });
        }
    }
    if cancel() {
        return Err(LocalError::Cancelled);
    }
    Ok(output)
}

fn baseline_trace(
    left: &[GradientFeature],
    right: &[GradientFeature],
    policy: GradientMatchPolicy,
    distinct_radius_squared: Option<f64>,
    cancel: impl Fn() -> bool,
) -> Result<(Vec<crate::geometry::Correspondence>,Vec<(u64,u64,usize)>,Vec<(u64,u64,usize)>),LocalError> {
    if !policy.max_squared_distance.is_finite()
        || !(0. ..=4.).contains(&policy.max_squared_distance)
        || !policy.squared_ratio.is_finite()
        || policy.squared_ratio <= 0.
        || policy.squared_ratio >= 1.
    {
        return Err(LocalError::Invalid);
    }
    if cancel() {
        return Err(LocalError::Cancelled);
    }
    let work = u64::try_from(left.len())
        .ok()
        .and_then(|a| u64::try_from(right.len()).ok().and_then(|b| a.checked_mul(b)))
        .ok_or(LocalError::Budget)?;
    let work = work
        .checked_mul(if distinct_radius_squared.is_some() { 2 } else { 1 })
        .ok_or(LocalError::Budget)?;
    if work > policy.max_comparisons {
        return Err(LocalError::Budget);
    }
    for feature in left.iter().chain(right) {
        if cancel() {
            return Err(LocalError::Cancelled);
        }
        let d = &feature.descriptor.0;
        if feature.position.iter().any(|v| !v.is_finite())
            || d.iter().any(|v| !v.is_finite() || *v < 0.)
            || (d.iter().map(|v| v * v).sum::<f64>() - 1.).abs() > 1e-6
        {
            return Err(LocalError::Invalid);
        }
    }
    if left.is_empty() || right.is_empty() {
        if cancel() {
            return Err(LocalError::Cancelled);
        }
        return Ok((Vec::new(),Vec::new(),Vec::new()));
    }
    let allocate = |len| {
        let mut v = Vec::new();
        v.try_reserve_exact(len).map_err(|_| LocalError::Budget)?;
        v.resize(len, (f64::INFINITY, f64::INFINITY, usize::MAX));
        Ok::<_, LocalError>(v)
    };
    let mut forward = allocate(left.len())?;
    let mut reverse = allocate(right.len())?;
 let mut forward_runner=vec![usize::MAX;left.len()];let mut reverse_runner=vec![usize::MAX;right.len()];
 let mut forward_resolved=vec![false;left.len()];let mut reverse_resolved=vec![false;right.len()];

    let update=|best:&mut(f64,f64,usize),runner:&mut usize,distance:f64,index:usize|{
 if distance<best.0{*runner=best.2;*best=(distance,best.0,index);}else if distance<best.1{best.1=distance;*runner=index;}
 };
    for (i, a) in left.iter().enumerate() {
        for (block, chunk) in right.chunks(4).enumerate() {
            for _ in chunk {
                if cancel() {
                    return Err(LocalError::Cancelled);
                }
            }
            let distances = baseline_distances(&a.descriptor, chunk);
            for (lane, _b) in chunk.iter().enumerate() {
                let j = block * 4 + lane;

                let distance = distances[lane];
                update(&mut forward[i], &mut forward_runner[i],distance, j);
                update(&mut reverse[j], &mut reverse_runner[j],distance, i);
            }
        }
    }
    if let Some(radius) = distinct_radius_squared {
        for i in 0..forward.len(){let runner=forward_runner[i];let best=&mut forward[i];if runner!=usize::MAX{let a=right[best.2].position;let b=right[runner].position;forward_resolved[i]=(a[0]-b[0]).powi(2)+(a[1]-b[1]).powi(2)>radius;}if !forward_resolved[i]{best.1=f64::INFINITY;}}
        for j in 0..reverse.len(){let runner=reverse_runner[j];let best=&mut reverse[j];if runner!=usize::MAX{let a=left[best.2].position;let b=left[runner].position;reverse_resolved[j]=(a[0]-b[0]).powi(2)+(a[1]-b[1]).powi(2)>radius;}if !reverse_resolved[j]{best.1=f64::INFINITY;}}
        for (i, a) in left.iter().enumerate() {
            for (block, chunk) in right.chunks(4).enumerate() {
                for _ in chunk {
                    if cancel() {
                        return Err(LocalError::Cancelled);
                    }
                }
                if forward_resolved[i] && (block*4..block*4+chunk.len()).all(|j|reverse_resolved[j]){continue;}
                let distances = baseline_distances(&a.descriptor, chunk);
                for (lane, b) in chunk.iter().enumerate() {
                    let j = block * 4 + lane;

                    let distance = distances[lane];
                    let nearest = right[forward[i].2].position;
                    if !forward_resolved[i] && ((b.position[0] - nearest[0]).powi(2) + (b.position[1] - nearest[1]).powi(2) > radius) {
                        forward[i].1 = forward[i].1.min(distance);
                    }
                    let nearest = left[reverse[j].2].position;
                    if !reverse_resolved[j] && ((a.position[0] - nearest[0]).powi(2) + (a.position[1] - nearest[1]).powi(2) > radius) {
                        reverse[j].1 = reverse[j].1.min(distance);
                    }
                }
            }
        }
    }
    let mut output = Vec::new();
    output
        .try_reserve_exact(left.len().min(right.len()))
        .map_err(|_| LocalError::Budget)?;
    for (i, &(distance, second, j)) in forward.iter().enumerate() {
        if cancel() {
            return Err(LocalError::Cancelled);
        }
        if (distinct_radius_squared.is_some() && !second.is_finite())
            || distance > policy.max_squared_distance
            || distance >= second * policy.squared_ratio
        {
            continue;
        }
        let (back, next, index) = reverse[j];
        if index == i
            && back < next * policy.squared_ratio
            && (distinct_radius_squared.is_none() || next.is_finite())
        {
            output.push(crate::geometry::Correspondence {
                source: left[i].position,
                target: right[j].position,
            });
        }
    }
    if cancel() {
        return Err(LocalError::Cancelled);
    }
    Ok((output,forward.iter().map(|&(a,b,i)|(a.to_bits(),b.to_bits(),i)).collect(),reverse.iter().map(|&(a,b,i)|(a.to_bits(),b.to_bits(),i)).collect()))
}

fn candidate_trace(
    left: &[GradientFeature],
    right: &[GradientFeature],
    policy: GradientMatchPolicy,
    distinct_radius_squared: Option<f64>,
    cancel: impl Fn() -> bool,
) -> Result<(Vec<crate::geometry::Correspondence>,Vec<(u64,u64,usize)>,Vec<(u64,u64,usize)>),LocalError> {
    if !policy.max_squared_distance.is_finite()
        || !(0. ..=4.).contains(&policy.max_squared_distance)
        || !policy.squared_ratio.is_finite()
        || policy.squared_ratio <= 0.
        || policy.squared_ratio >= 1.
    {
        return Err(LocalError::Invalid);
    }
    if cancel() {
        return Err(LocalError::Cancelled);
    }
    let work = u64::try_from(left.len())
        .ok()
        .and_then(|a| u64::try_from(right.len()).ok().and_then(|b| a.checked_mul(b)))
        .ok_or(LocalError::Budget)?;
    let work = work
        .checked_mul(if distinct_radius_squared.is_some() { 2 } else { 1 })
        .ok_or(LocalError::Budget)?;
    if work > policy.max_comparisons {
        return Err(LocalError::Budget);
    }
    for feature in left.iter().chain(right) {
        if cancel() {
            return Err(LocalError::Cancelled);
        }
        let d = &feature.descriptor.0;
        if feature.position.iter().any(|v| !v.is_finite())
            || d.iter().any(|v| !v.is_finite() || *v < 0.)
            || (d.iter().map(|v| v * v).sum::<f64>() - 1.).abs() > 1e-6
        {
            return Err(LocalError::Invalid);
        }
    }
    if left.is_empty() || right.is_empty() {
        if cancel() {
            return Err(LocalError::Cancelled);
        }
        return Ok((Vec::new(),Vec::new(),Vec::new()));
    }
    let allocate = |len| {
        let mut v = Vec::new();
        v.try_reserve_exact(len).map_err(|_| LocalError::Budget)?;
        v.resize(len, (f64::INFINITY, f64::INFINITY, usize::MAX));
        Ok::<_, LocalError>(v)
    };
    let mut forward = allocate(left.len())?;
    let mut reverse = allocate(right.len())?;
 let mut forward_runner=vec![usize::MAX;left.len()];let mut reverse_runner=vec![usize::MAX;right.len()];
 let mut forward_resolved=vec![false;left.len()];let mut reverse_resolved=vec![false;right.len()];

    let update=|best:&mut(f64,f64,usize),runner:&mut usize,distance:f64,index:usize|{
 if distance<best.0{*runner=best.2;*best=(distance,best.0,index);}else if distance<best.1{best.1=distance;*runner=index;}
 };
    for (i, a) in left.iter().enumerate() {
        for (block, chunk) in right.chunks(4).enumerate() {
            for _ in chunk {
                if cancel() {
                    return Err(LocalError::Cancelled);
                }
            }
            let distances = baseline_distances(&a.descriptor, chunk);
            for (lane, _b) in chunk.iter().enumerate() {
                let j = block * 4 + lane;

                let distance = distances[lane];
                update(&mut forward[i], &mut forward_runner[i],distance, j);
                update(&mut reverse[j], &mut reverse_runner[j],distance, i);
            }
        }
    }
    if let Some(radius) = distinct_radius_squared {
        for i in 0..forward.len(){let runner=forward_runner[i];let best=&mut forward[i];if runner!=usize::MAX{let a=right[best.2].position;let b=right[runner].position;forward_resolved[i]=(a[0]-b[0]).powi(2)+(a[1]-b[1]).powi(2)>radius;}if !forward_resolved[i]{best.1=f64::INFINITY;}}
        for j in 0..reverse.len(){let runner=reverse_runner[j];let best=&mut reverse[j];if runner!=usize::MAX{let a=left[best.2].position;let b=left[runner].position;reverse_resolved[j]=(a[0]-b[0]).powi(2)+(a[1]-b[1]).powi(2)>radius;}if !reverse_resolved[j]{best.1=f64::INFINITY;}}
        for (i, a) in left.iter().enumerate() {
            for (block, chunk) in right.chunks(4).enumerate() {
                for _ in chunk {
                    if cancel() {
                        return Err(LocalError::Cancelled);
                    }
                }
                let distances = if forward_resolved[i] {
                    let mut needed = 0u8;
                    for lane in 0..chunk.len() {
                        if !reverse_resolved[block*4+lane] { needed |= 1 << lane; }
                    }
                    if needed == 0 { continue; }
                    if needed.count_ones() <= 2 {
                        let mut values = [0.;4];
                        for lane in 0..chunk.len() {
                            if needed & (1 << lane) != 0 {
                                values[lane] = a.descriptor.0.iter().zip(&chunk[lane].descriptor.0).map(|(x,y)|(x-y)*(x-y)).sum();
                            }
                        }
                        values
                    } else { baseline_distances(&a.descriptor,chunk) }
                } else { baseline_distances(&a.descriptor,chunk) };
                for (lane, b) in chunk.iter().enumerate() {
                    let j = block * 4 + lane;

                    let distance = distances[lane];
                    let nearest = right[forward[i].2].position;
                    if !forward_resolved[i] && ((b.position[0] - nearest[0]).powi(2) + (b.position[1] - nearest[1]).powi(2) > radius) {
                        forward[i].1 = forward[i].1.min(distance);
                    }
                    let nearest = left[reverse[j].2].position;
                    if !reverse_resolved[j] && ((a.position[0] - nearest[0]).powi(2) + (a.position[1] - nearest[1]).powi(2) > radius) {
                        reverse[j].1 = reverse[j].1.min(distance);
                    }
                }
            }
        }
    }
    let mut output = Vec::new();
    output
        .try_reserve_exact(left.len().min(right.len()))
        .map_err(|_| LocalError::Budget)?;
    for (i, &(distance, second, j)) in forward.iter().enumerate() {
        if cancel() {
            return Err(LocalError::Cancelled);
        }
        if (distinct_radius_squared.is_some() && !second.is_finite())
            || distance > policy.max_squared_distance
            || distance >= second * policy.squared_ratio
        {
            continue;
        }
        let (back, next, index) = reverse[j];
        if index == i
            && back < next * policy.squared_ratio
            && (distinct_radius_squared.is_none() || next.is_finite())
        {
            output.push(crate::geometry::Correspondence {
                source: left[i].position,
                target: right[j].position,
            });
        }
    }
    if cancel() {
        return Err(LocalError::Cancelled);
    }
    Ok((output,forward.iter().map(|&(a,b,i)|(a.to_bits(),b.to_bits(),i)).collect(),reverse.iter().map(|&(a,b,i)|(a.to_bits(),b.to_bits(),i)).collect()))
}

fn random(seed:&mut u64)->f64{*seed=seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);((*seed>>11) as f64)/((1u64<<53)as f64)}
fn fixture(n:usize,mode:usize,seed:u64)->Vec<GradientFeature>{let mut seed=seed;(0..n).map(|i|{let mut d=[0.;128];match mode{0=>for v in &mut d{*v=random(&mut seed);},1=>{d[(i*17)%128]=1.;},2=>{for v in &mut d{*v=random(&mut seed)*0.01;}d[i%16]=1.;},_=>{d[i%8]=1.;d[(i%8)+32]=0.1;}}let norm=d.iter().map(|v|v*v).sum::<f64>().sqrt();for v in &mut d{*v/=norm;}GradientFeature{position:[(i%31)as f64*3.,(i/31)as f64*3.],descriptor:GradientDescriptor(d)}}).collect()}

fn read_dump(path:&str)->Vec<GradientFeature>{let bytes=std::fs::read(path).unwrap();assert!(bytes.len()>=8);let n=u64::from_le_bytes(bytes[..8].try_into().unwrap())as usize;assert!(n<100000&&bytes.len()==8+n*130*8);let values:Vec<_>=bytes[8..].chunks_exact(8).map(|v|f64::from_bits(u64::from_le_bytes(v.try_into().unwrap()))).collect();values.chunks_exact(130).map(|v|GradientFeature{position:[v[0],v[1]],descriptor:GradientDescriptor(v[2..].try_into().unwrap())}).collect()}
fn run_natural(args:&[String],policy:GradientMatchPolicy){let a=read_dump(&args[0]);let b=read_dump(&args[1]);for radius in [None,Some(4.)]{let expected=baseline(&a,&b,policy,radius,||false).unwrap();assert_eq!(expected,candidate(&a,&b,policy,radius,||false).unwrap());assert_eq!(baseline_trace(&a,&b,policy,radius,||false),candidate_trace(&a,&b,policy,radius,||false));for round in 0..9{let mut ns=[0u128;2];for which in [round%2,1-round%2]{let start=Instant::now();let output=if which==0{baseline(black_box(&a),black_box(&b),policy,radius,||false)}else{candidate(black_box(&a),black_box(&b),policy,radius,||false)}.unwrap();black_box(&output);assert_eq!(output,expected);ns[which]=start.elapsed().as_nanos();}println!("{{\"kind\":\"natural_timing\",\"case_id\":{},\"distinct\":{},\"round\":{},\"left_features\":{},\"right_features\":{},\"baseline_ns\":{},\"candidate_ns\":{},\"matches\":{},\"nearest_tables_bit_identical\":true}}",args[2],radius.is_some(),round,a.len(),b.len(),ns[0],ns[1],expected.len());}}}


fn check(left:&[GradientFeature],right:&[GradientFeature],radius:Option<f64>,exhaustive:bool)->usize {
    let policy=GradientMatchPolicy{max_comparisons:10000000,max_squared_distance:2.,squared_ratio:0.8};
    let count=Cell::new(0usize);
    let expected=baseline_trace(left,right,policy,radius,||{count.set(count.get()+1);false});
    let calls=count.get();count.set(0);
    let actual=candidate_trace(left,right,policy,radius,||{count.set(count.get()+1);false});
    assert_eq!(expected,actual,"nearest/correspondence mismatch: n={} m={} radius={:?}",left.len(),right.len(),radius);
    assert_eq!(calls,count.get());
    let stops:Vec<_>=if exhaustive {(1..=calls).collect()} else {vec![1,calls/2+1,calls]};
    for stop in &stops {
        count.set(0);
        let expected=baseline_trace(left,right,policy,radius,||{count.set(count.get()+1);count.get()==*stop});
        let before=count.get();count.set(0);
        let actual=candidate_trace(left,right,policy,radius,||{count.set(count.get()+1);count.get()==*stop});
        assert_eq!(expected,actual);assert_eq!(before,count.get());
    }
    stops.len()
}
fn cluster_bank()->Vec<GradientFeature>{
    (0..512).map(|i|{let cluster=i/8;let variant=i%8;let mut d=[0.;128];d[cluster]=1.;d[64+variant]=0.01;
        let norm=d.iter().map(|v|v*v).sum::<f64>().sqrt();for v in &mut d{*v/=norm;}
        GradientFeature{position:[cluster as f64*10.+variant as f64*0.01,0.],descriptor:GradientDescriptor(d)}
    }).collect()
}

fn main(){let args:Vec<_>=std::env::args().skip(1).collect();assert!(args.len()==1||args.len()==3);
    let (left,right)=if args.len()==3{(read_dump(&args[1]),read_dump(&args[2]))}else{let b=cluster_bank();(b.clone(),b)};
    let policy=GradientMatchPolicy{max_comparisons:10000000,max_squared_distance:2.,squared_ratio:0.8};
    let mut retained=Vec::new();
    for _ in 0..4 {retained=if args[0]=="runner"{baseline(black_box(&left),black_box(&right),policy,Some(4.),||false)}else{assert_eq!(args[0],"masked");candidate(black_box(&left),black_box(&right),policy,Some(4.),||false)}.unwrap();black_box(&retained);}
    println!("{}",retained.len());for p in retained{println!("{} {} {} {}",p.source[0].to_bits(),p.source[1].to_bits(),p.target[0].to_bits(),p.target[1].to_bits());}
}
