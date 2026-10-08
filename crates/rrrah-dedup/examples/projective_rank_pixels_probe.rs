use rrrah_core::{MemoryBudget, SharedBuffer};
use rrrah_dedup::{
    exact::ContentSnapshot,
    geometry::ProjectiveTransform,
    linear::LinearRgbaView,
    mesh_grid::{ProjectiveGridPolicy, build_projective_grid},
    mesh_rank::compare_mesh_rank,
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
        return Err("source pixels, target pixels, nine-coefficient model required".into());
    }
    let values: Vec<f64> = std::fs::read_to_string(&args[2])?
        .split_whitespace()
        .map(str::parse)
        .collect::<Result<_, _>>()?;
    if values.len() != 9 {
        return Err("nine model coefficients required".into());
    }
    let model = ProjectiveTransform {
        matrix: std::array::from_fn(|i| std::array::from_fn(|j| values[i * 3 + j])),
    };
    let budget = MemoryBudget::new(512 * 1024 * 1024);
    let first = ContentSnapshot::read(Path::new(&args[0]), 110000000, &|| false)?;
    let second = ContentSnapshot::read(Path::new(&args[1]), 110000000, &|| false)?;
    let (sw, sh, a) = pixels(Path::new(&args[0]), &budget)?;
    let (tw, th, b) = pixels(Path::new(&args[1]), &budget)?;
    let av = LinearRgbaView::new(sw, sh, &a, 6400000, || false)?;
    let bv = LinearRgbaView::new(tw, th, &b, 6400000, || false)?;
    for (name, model, src, dst, sd, td) in [
        ("forward", model, &av, &bv, (sw, sh), (tw, th)),
        ("reverse", model.inverse()?, &bv, &av, (tw, th), (sw, sh)),
    ] {
        let grid = build_projective_grid(
            model,
            sd,
            td,
            [0, 0, td.0, td.1],
            ProjectiveGridPolicy {
                maximum_sites: 6400000,
            },
            &budget,
            || false,
        )?;
        for radius in [0, 3, 8] {
            let e = compare_mesh_rank(
                src,
                dst,
                &grid,
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
