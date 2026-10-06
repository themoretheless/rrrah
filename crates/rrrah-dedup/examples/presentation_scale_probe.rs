//! Warm-cache complete-presentation baseline; known fixture labels verify every pair.
use rrrah_core::MemoryBudget;
use rrrah_decode::DecodeRequest;
use rrrah_dedup::{
    animated::{AnimationBudget, AnimationKind},
    container::{Presentation, PresentationPolicy, scan_indexed_presentation_groups, scan_presentations},
    pages::PageKind,
};
use std::{fs, path::PathBuf, time::Instant};

#[allow(clippy::too_many_lines)] // Keep labelled fixtures and both measured search modes together.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let indexed = std::env::args().any(|value| value == "--indexed-groups");
    let fixture_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    for pages in [false, true] {
        let fixtures = if pages {
            [
                "tiff/classic-little-none-same.tif",
                "tiff/bigtiff-big-lzw-same.tif",
                "tiff/bigtiff-big-lzw-changed.tif",
            ]
        } else {
            [
                "timeline/base.gif",
                "timeline/split.apng",
                "timeline/changed-pixels.apng",
            ]
        };
        for count in [8_usize, 32, 128] {
            let folder = tempfile::tempdir()?;
            let mut requests = Vec::new();
            let mut labels = Vec::new();
            let mut encoded_bytes = 0_u64;
            for id in 0..count {
                let variant = id % 3;
                let from = fixture_root.join(fixtures[variant]);
                let to = folder
                    .path()
                    .join(format!("{id:04}.{}", from.extension().unwrap().to_str().unwrap()));
                encoded_bytes += fs::copy(from, &to)?;
                let mode = if pages {
                    Presentation::Pages(PageKind::Tiff)
                } else if variant == 0 {
                    Presentation::Animation(AnimationKind::Gif)
                } else {
                    Presentation::Animation(AnimationKind::Apng)
                };
                requests.push((u64::try_from(id)?, DecodeRequest::new(to), mode));
                // Independently authored fixture semantics: base and differently
                // encoded/split presentation are equal; last-page/frame edit differs.
                labels.push(variant == 2);
            }
            let mut equal = Vec::new();
            let mut different = Vec::new();
            for a in 0..count {
                for b in a + 1..count {
                    let pair = (u64::try_from(a)?, u64::try_from(b)?);
                    if labels[a] == labels[b] {
                        equal.push(pair);
                    } else {
                        different.push(pair);
                    }
                }
            }
            let limits = AnimationBudget {
                max_frames: 10,
                max_pixels: 100,
                max_file_bytes: 100_000,
            };
            for repetition in 0..4 {
                let budget = MemoryBudget::new(16 * 1024 * 1024);
                let started = Instant::now();
                let (report, candidate_pairs) = if indexed {
                    let result = scan_indexed_presentation_groups(
                        requests.clone(),
                        PresentationPolicy {
                            max_files: count,
                            max_pairs: equal.len(),
                            limits,
                            grouping: rrrah_dedup::groups::GroupBudget {
                                max_entries: count,
                                max_pairs: equal.len(),
                                max_pair_checks: u64::try_from(count * count)?,
                            },
                        },
                        &budget,
                        || false,
                    )?;
                    assert!(result.preparation_issues.is_empty() && result.result.source_issues.is_empty());
                    let expected_groups = [false, true].map(|label| {
                        labels
                            .iter()
                            .enumerate()
                            .filter_map(|(id, &value)| (value == label).then_some(u64::try_from(id).unwrap()))
                            .collect::<Vec<_>>()
                    });
                    assert_eq!(result.result.grouping.groups, expected_groups);
                    (result.result.comparisons, result.candidate_pairs)
                } else {
                    (
                        scan_presentations(
                            requests.clone(),
                            count,
                            count * (count - 1) / 2,
                            limits,
                            &budget,
                            || false,
                        )?,
                        count * (count - 1) / 2,
                    )
                };
                let seconds = started.elapsed().as_secs_f64();
                assert!(report.issues.is_empty(), "{:?}", report.issues);
                assert_eq!(report.equal, equal);
                if indexed {
                    assert!(report.different.is_empty());
                } else {
                    assert_eq!(report.different, different);
                }
                assert_eq!(budget.used(), 0);
                if repetition != 0 {
                    println!(
                        "{{\"indexed\":{indexed},\"candidate_pairs\":{candidate_pairs},\"pages\":{pages},\"files\":{count},\"pairs\":{},\"encoded_bytes\":{encoded_bytes},\"repetition\":{repetition},\"seconds\":{seconds},\"equal\":{},\"different\":{},\"peak_managed_bytes\":{},\"retained_managed_bytes\":0}}",
                        count * (count - 1) / 2,
                        equal.len(),
                        report.different.len(),
                        budget.peak()
                    );
                }
            }
        }
    }
    Ok(())
}
