//! One-shot CPU inference worker. Binary stdout is reserved for the protocol.
use std::{error::Error, path::PathBuf};
fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args_os().skip(1);
    let weights = PathBuf::from(args.next().ok_or("weights path required")?);
    let mut number = || -> Result<String, Box<dyn Error>> {
        Ok(args
            .next()
            .ok_or("worker numeric argument required")?
            .into_string()
            .map_err(|_| "non-UTF8 numeric argument")?)
    };
    let pixels: usize = number()?.parse()?;
    let proposals: usize = number()?.parse()?;
    let threads: i32 = number()?.parse()?;
    let seed: i64 = number()?.parse()?;
    if args.next().is_some() || pixels == 0 || threads <= 0 {
        return Err("invalid worker arguments".into());
    }
    let bytes = pixels
        .checked_mul(2)
        .and_then(|n| {
            proposals
                .checked_mul(std::mem::size_of::<rrrah_dedup::proposals::ScoredProposal>())
                .and_then(|p| n.checked_add(p))
        })
        .ok_or("worker budget overflow")?;
    let budget = rrrah_core::MemoryBudget::new(u64::try_from(bytes)?);
    tch::set_num_threads(threads);
    tch::manual_seed(seed);
    rrrah_dedup::model_worker::serve_loftr_worker_once(
        &mut std::io::stdin().lock(),
        &mut std::io::stdout().lock(),
        &weights,
        pixels,
        proposals,
        &budget,
        &|| false,
    )?;
    assert_eq!(budget.used(), 0);
    Ok(())
}
