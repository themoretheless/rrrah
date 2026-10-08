use rrrah_core::{MemoryBudget, SharedBuffer};
use rrrah_dedup::{
    exact::ContentSnapshot,
    geometry::ProjectiveTransform,
    linear::LinearRgbaView,
    mesh_grid::{ProjectiveGridPolicy, build_projective_grid},
    mesh_rank::compare_mesh_rank_region,
    rank_region::RankRegionPolicy,
};
use std::{
    io::{BufReader, Read},
    path::Path,
};
fn pixels(
    path: &Path,
    budget: &MemoryBudget,
) -> Result<(u32, u32, SharedBuffer<f32>), Box<dyn std::error::Error>> {
    let scratch = budget.try_reserve(65536)?;
    let mut file = BufReader::with_capacity(65536, std::fs::File::open(path)?);
    let mut magic = [0; b"RRRAH-RANK-RGBA32-V1\n".len()];
    file.read_exact(&mut magic)?;
    if &magic != b"RRRAH-RANK-RGBA32-V1\n" {
        return Err("invalid pixel header".into());
    }
    let mut b = [0; 4];
    file.read_exact(&mut b)?;
    let w = u32::from_le_bytes(b);
    file.read_exact(&mut b)?;
    let h = u32::from_le_bytes(b);
    let n = u64::from(w) * u64::from(h);
    if n == 0 || n > 6400000 {
        return Err("pixel limit".into());
    }
    let credit = budget.try_reserve(n * 16)?;
    let mut samples = Vec::new();
    samples.try_reserve_exact(usize::try_from(n * 4)?)?;
    for _ in 0..n * 4 {
        file.read_exact(&mut b)?;
        samples.push(f32::from_le_bytes(b));
    }
    if file.read(&mut b)? != 0 {
        return Err("trailing pixels".into());
    }
    drop(file);
    drop(scratch);
    Ok((w, h, credit.try_adopt(samples)?))
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 3 {
        return Err("source pixels, target pixels, seventeen-value model/regions required".into());
    }
    let values: Vec<f64> = std::fs::read_to_string(&args[2])?
        .split_whitespace()
        .map(str::parse)
        .collect::<Result<_, _>>()?;
    if values.len() != 17 {
        return Err("nine coefficients and eight integer region values required".into());
    }
    let model = ProjectiveTransform {
        matrix: std::array::from_fn(|i| std::array::from_fn(|j| values[i * 3 + j])),
    };
    if values[9..]
        .iter()
        .any(|v| !v.is_finite() || *v < 0. || *v > f64::from(u32::MAX) || v.fract() != 0.)
    {
        return Err("invalid integer regions".into());
    }
    let source_region: [u32; 4] = std::array::from_fn(|i| values[9 + i] as u32);
    let target_region: [u32; 4] = std::array::from_fn(|i| values[13 + i] as u32);
    let budget = MemoryBudget::new(512 * 1024 * 1024);
    let first = ContentSnapshot::read(Path::new(&args[0]), 110000000, &|| false)?;
    let second = ContentSnapshot::read(Path::new(&args[1]), 110000000, &|| false)?;
    let (sw, sh, a) = pixels(Path::new(&args[0]), &budget)?;
    let (tw, th, b) = pixels(Path::new(&args[1]), &budget)?;
    let av = LinearRgbaView::new(sw, sh, &a, 6400000, || false)?;
    let bv = LinearRgbaView::new(tw, th, &b, 6400000, || false)?;
    for (name, model, src, dst, sd, td, sr, tr) in [
        (
            "forward",
            model,
            &av,
            &bv,
            (sw, sh),
            (tw, th),
            source_region,
            target_region,
        ),
        (
            "reverse",
            model.inverse()?,
            &bv,
            &av,
            (tw, th),
            (sw, sh),
            target_region,
            source_region,
        ),
    ] {
        let x0 = tr[0].saturating_sub(16);
        let y0 = tr[1].saturating_sub(16);
        let x1 = tr[0]
            .checked_add(tr[2])
            .and_then(|v| v.checked_add(16))
            .ok_or("region overflow")?
            .min(td.0);
        let y1 = tr[1]
            .checked_add(tr[3])
            .and_then(|v| v.checked_add(16))
            .ok_or("region overflow")?
            .min(td.1);
        if x1 <= x0 || y1 <= y0 {
            return Err("invalid context".into());
        }
        let grid = build_projective_grid(
            model,
            sd,
            td,
            [x0, y0, x1 - x0, y1 - y0],
            ProjectiveGridPolicy {
                maximum_sites: 6400000,
            },
            &budget,
            || false,
        )?;
        for radius in [0, 3, 8] {
            let e = compare_mesh_rank_region(
                src,
                dst,
                &grid,
                sr,
                tr,
                RankRegionPolicy {
                    radius: 8,
                    minimum_contrast: 0.005,
                    minimum_pairs: 1000,
                    maximum_sites: 6400000,
                    maximum_pixel_reads: 32000000,
                },
                radius,
                &budget,
                || false,
            )?;
            println!(
                "{{\"direction\":\"{name}\",\"filter_radius\":{radius},\"sites\":{},\"covered\":{},\"valid_sites\":{},\"informative_pairs\":{},\"agreeing_pairs\":{},\"pixel_reads\":{}}}",
                e.sites,
                grid.covered_sites(),
                e.valid_sites,
                e.informative_pairs,
                e.agreeing_pairs,
                e.pixel_reads
            );
        }
    }
    first.verify(&|| false)?;
    second.verify(&|| false)?;
    drop(a);
    drop(b);
    assert_eq!(budget.used(), 0);
    Ok(())
}
