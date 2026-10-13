#!/usr/bin/env python3
import json,subprocess
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1];D=ROOT/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def main():
 m=json.loads((D/'dedup-gradient-index-row-major-formatted-snapshot-20261011.json').read_text());source=Path(m['snapshot']);s=(source/'crates/rrrah-dedup/src/gradient_index.rs').read_text()
 a=s.index('    pub fn search(\n');b=s.index('\n}\n\n/// Conservative',a);old=s[a:b]
 header=old[:old.index(' {\n')+3];body=old[old.index(' {\n')+3:]
 wrapper=header+'''        self.search_with_comparisons(query, recipe, radius, max_comparisons, max_results, cancel).map(|(hits, _)| hits)
    }
'''
 helper=header.replace('pub fn search(', 'fn search_with_comparisons(').replace('Result<Vec<GradientCandidate>, LocalError>', 'Result<(Vec<GradientCandidate>, u64), LocalError>')+body
 helper=helper.replace('''        if u64::try_from(end - start).map_err(|_| LocalError::Budget)? > max_comparisons {''','''        let comparisons = u64::try_from(end - start).map_err(|_| LocalError::Budget)?;
        if comparisons > max_comparisons {''').replace('        Ok(result)','        Ok((result, comparisons))')
 assert 'let comparisons =' in helper;s=s[:a]+wrapper+helper+s[b:]
 s=s.replace('''    pub descriptor_hits: usize,
}''','''    pub descriptor_hits: usize,
    /// Actual coordinate-window candidates evaluated, including rejected distances.
    /// Index construction, coordinate lookup and reflection have separate bounds.
    pub descriptor_comparisons: u64,
}''',1)
 s=s.replace('''        None,
        cancel,
    )
}''','''        None,
        true,
        cancel,
    )
}''',1).replace('''        Some(maximum_reflection_bins),
        cancel,
    )
}''','''        Some(maximum_reflection_bins),
        true,
        cancel,
    )
}''',1)
 a=s.index('\nfn gradient_file_pair_report_selected(')
 new='''
/// Exact descriptor-radius proposals with a cumulative actual-comparison cap.
/// Unlike the legacy worst-case preflight, a sparse coordinate window can admit
/// fewer than n*n comparisons. Every attempted descriptor distance consumes one
/// unit, including early rejected distances; self/same-file hits still count.
/// `reflection_bins` is None for ordinary search, or the prepaid transform-bin
/// cap for both ordinary and reflected queries. Index/scratch/pair memory, hit
/// and pair caps remain independent. Sources remain immutable borrowed data.
/// These proposals require fresh geometry/pixel confirmation, not file identity.
/// # Errors
/// Invalid data, cancellation or any cumulative limit refuses the whole report;
/// no partial pairs publish. Actual work refusal can occur after allocation or
/// earlier successful queries; all managed reservations are released.
#[allow(clippy::too_many_arguments)]
pub fn gradient_file_pair_report_with_actual_work(
    entries: &[(u64, GradientDescriptor)],
    owners: &[u64],
    recipe: GradientCellRecipe,
    radius: f64,
    max_comparisons: u64,
    max_hits: usize,
    max_pairs: usize,
    budget: &MemoryBudget,
    reflection_bins: Option<u64>,
    cancel: impl Fn() -> bool,
) -> Result<GradientFilePairReport, LocalError> {
    gradient_file_pair_report_selected(entries, owners, recipe, radius, max_comparisons,
        max_hits, max_pairs, budget, reflection_bins, false, cancel)
}
'''
 s=s[:a]+new+s[a:]
 a=s.index('\nfn gradient_file_pair_report_selected(');before=s[:a];body=s[a:]
 body=body.replace('''    reflection_bins: Option<u64>,
    cancel:''','''    reflection_bins: Option<u64>,
    prepay_worst_case: bool,
    cancel:''',1).replace('''    if n.checked_mul(n)''','''    if prepay_worst_case && n.checked_mul(n)''',1)
 body=body.replace('''    let mut hits = 0_usize;''','''    let mut hits = 0_usize;
    let mut comparisons = 0_u64;''',1)
 old='''            let results = index.search(query, recipe, radius, n, max_hits.saturating_sub(hits), &cancel)?;'''
 assert old in body
 body=body.replace(old,'''            let remaining = if prepay_worst_case { n } else { max_comparisons.saturating_sub(comparisons) };
            let (results, attempted) = index.search_with_comparisons(query, recipe, radius, remaining, max_hits.saturating_sub(hits), &cancel)?;
            comparisons = comparisons.checked_add(attempted).ok_or(LocalError::Budget)?;''',1)
 body=body.replace('''        descriptor_hits: hits,''','''        descriptor_hits: hits,
        descriptor_comparisons: comparisons,''',1)
 p=T/'rrrah-gradient-index-actual-work-source-20261011.rs';p.write_text(before+body);subprocess.run(['rustfmt','--edition','2024',str(p)],check=True);print(p)
if __name__=='__main__':main()
