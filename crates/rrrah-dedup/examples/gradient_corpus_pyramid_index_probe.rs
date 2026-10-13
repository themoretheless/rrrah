//! Corpus descriptor-proposal diagnostic; no file-identity decision is implied.
use rrrah_core::MemoryBudget;
use rrrah_dedup::{decode::decode_selected_frame, gradient::{extract_gradient_pyramid_with_recipe_managed, GradientCellRecipe}, gradient_index::gradient_file_pair_report_with_actual_work, local::LocalPolicy, pyramid::PyramidPolicy};
use std::{collections::BTreeSet, error::Error, io::Write};
fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 3 { return Err("usage: gradient_corpus_pyramid_index_probe paths.txt radius descriptors.bin".into()); }
    let paths = std::fs::read_to_string(&args[0])?;
    let radius: f64 = args[1].parse()?;
    let recipe = GradientCellRecipe::Interpolated;
    let budget = MemoryBudget::new(256*1024*1024);
    let files: Vec<_> = paths.lines().collect();
    if files.len() > 386 { return Err("maximum 386 files".into()); }
    // Descriptor bank and owner arrays remain admitted through report and oracle.
    let bank_bytes = files.len().checked_mul(128).and_then(|n| n.checked_mul(std::mem::size_of::<(u64,rrrah_dedup::gradient::GradientDescriptor)>() + std::mem::size_of::<u64>())).ok_or("bank overflow")?;
    let _bank_credit = budget.try_reserve(bank_bytes as u64)?;
    let mut entries = Vec::new(); entries.try_reserve_exact(files.len()*128)?;
    let mut owners = Vec::new(); owners.try_reserve_exact(files.len()*128)?;
    for (owner,path) in files.iter().enumerate() {
        let mut request = rrrah_decode::DecodeRequest::new(path);
        request.assume_untagged_linear_srgb = true;
        let image = decode_selected_frame(&request, 200000, &budget, || false)?;
        let features = extract_gradient_pyramid_with_recipe_managed(&image.view(|| false)?, PyramidPolicy {
            local: LocalPolicy { max_pixels:200000,max_candidates:200000,max_features:32,minimum_corner_score:0.0001 },
            max_levels:4,max_total_pixels:200000,max_total_features:128,
        }, 128*512, recipe, &budget, || false)?;
        println!("F\t{owner}\t{}",features.len());
        for feature in features.iter() {
            entries.push((entries.len() as u64,feature.descriptor));owners.push(owner as u64);
        }
    }
    let mut output = std::io::BufWriter::new(std::fs::File::create(&args[2])?);
    for ((id,d),owner) in entries.iter().zip(&owners) {
        output.write_all(&id.to_le_bytes())?;output.write_all(&owner.to_le_bytes())?;
        for value in d.0 { output.write_all(&value.to_le_bytes())?; }
    }
    output.flush()?;
    let max_work=(entries.len() as u64).checked_mul(entries.len() as u64).ok_or("work overflow")?;
    let start=std::time::Instant::now();
    let report=gradient_file_pair_report_with_actual_work(&entries,&owners,recipe,radius,max_work,max_work.try_into()?,files.len()*files.len(),&budget,None,||false)?;
    let index_elapsed=start.elapsed();
    // Independent full-distance oracle: no index window or early distance exit.
    let start=std::time::Instant::now();let mut pairs=BTreeSet::new();let mut hits=0usize;
    for (i,(_,a)) in entries.iter().enumerate() {
        for (j,(_,b)) in entries.iter().enumerate() {
            let mut distance=0.;for k in 0..128 { let delta=a.0[k]-b.0[k];distance+=delta*delta; }
            if distance<=radius { hits+=1;if owners[i]!=owners[j] { pairs.insert((owners[i].min(owners[j]),owners[i].max(owners[j]))); } }
        }
    }
    let oracle_elapsed=start.elapsed();
    assert_eq!(report.descriptor_hits,hits);
    assert_eq!(report.pairs,pairs.into_iter().collect::<Vec<_>>());
    for &(a,b) in &report.pairs { println!("P\t{a}\t{b}"); }
    println!("S\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",files.len(),entries.len(),report.pairs.len(),hits,report.descriptor_comparisons,max_work,index_elapsed.as_secs_f64(),oracle_elapsed.as_secs_f64());
    drop(report);drop(entries);drop(owners);drop(_bank_credit);
    assert_eq!(budget.used(),0);println!("M\t{}\t{}",budget.used(),budget.peak());
    Ok(())
}
