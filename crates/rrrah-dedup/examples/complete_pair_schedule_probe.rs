//! Verify complete enumeration after proposal prioritization; no image decision.
use rrrah_dedup::pair_schedule::CompletePairSchedule;
use std::{error::Error, io::Write};
fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 3 {
        return Err("usage: proposal.log file_count output.tsv".into());
    }
    let n: usize = args[1].parse()?;
    if n > 386 {
        return Err("file cap".into());
    }
    let text = std::fs::read_to_string(&args[0])?;
    let mut hints = Vec::new();
    for line in text.lines().filter(|l| l.starts_with("P\t")) {
        let columns: Vec<_> = line.split('\t').collect();
        if columns.len() != 3 {
            return Err("proposal shape".into());
        }
        hints.push((columns[1].parse()?, columns[2].parse()?));
    }
    let ids: Vec<_> = (0..n as u64).collect();
    let pairs = n.saturating_mul(n.saturating_sub(1)) / 2;
    let budget = rrrah_core::MemoryBudget::new(16 * 1024 * 1024);
    let schedule = CompletePairSchedule::new(&ids, &hints, 386, pairs as u64, pairs, &budget, || false)?;
    let mut output = std::io::BufWriter::new(std::fs::File::create(&args[2])?);
    let mut proposed = 0usize;
    let mut fallback = 0usize;
    for pair in schedule.pairs(|| false) {
        let pair = pair?;
        if pair.proposed {
            proposed += 1;
        } else {
            fallback += 1;
        }
        writeln!(
            output,
            "{}\t{}\t{}",
            pair.left,
            pair.right,
            u8::from(pair.proposed)
        )?;
    }
    output.flush()?;
    assert_eq!((proposed + fallback) as u64, schedule.total_pairs());
    drop(schedule);
    assert_eq!(budget.used(), 0);
    println!(
        "files={n} proposed={proposed} fallback={fallback} managed_used={} managed_peak={}",
        budget.used(),
        budget.peak()
    );
    Ok(())
}
