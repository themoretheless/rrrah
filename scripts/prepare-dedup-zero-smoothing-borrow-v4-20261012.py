#!/usr/bin/env python3
import hashlib,json,shutil,subprocess
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 mp=D/'dedup-zero-smoothing-borrow-source-v3-20261012.json';m=json.loads(mp.read_text());parent=Path(m['snapshot']);s=T/'rrrah-zero-smoothing-borrow-candidate-v4-20261012';assert not s.exists();s.mkdir()
 for k,v in m['file_hashes'].items():
  assert sha(parent/k)==v,k
  p=s/k;p.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(parent/k,p)
 key='crates/rrrah-dedup/src/local_collection.rs';p=s/key;text=p.read_text();start=text.index('mod zero_collection_candidate_tests {');prefix=text[:start];suffix=text[start:];suffix=suffix.replace('    LocalFileError, ProjectiveCandidateUnionRegionsFilePolicy, ProjectiveDistinctScaleRegionsFilePolicy,','    ProjectiveCandidateUnionRegionsFilePolicy, ProjectiveDistinctScaleRegionsFilePolicy,').replace('    SpatialFeaturePolicy, compare_local_files_projective_candidate_union_regions_managed as compare,','    SpatialFeaturePolicy,');p.write_text(prefix+suffix)
 key='crates/rrrah-dedup/tests/candidate_union_files.rs';p=s/key
 with p.open('a') as f:f.write('''
#[test]
fn zero_radius_collection_matches_fresh_pair_evidence_both_orders() {
    use rrrah_dedup::local_collection::scan_projective_local_collection_candidate_union_regions_managed as scan;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rotation");
    let files = [
        (1, rrrah_decode::DecodeRequest::new(root.join("base.png"))),
        (2, rrrah_decode::DecodeRequest::new(root.join("scale-117-angle-0.png"))),
        (3, rrrah_decode::DecodeRequest::new(root.join("unrelated.png"))),
    ];
    let mut p = collection_config();
    p.search.smoothing_radius = 0;
    for reverse in [false, true] {
        let input: Vec<_> = if reverse { files.iter().rev().cloned().collect() } else { files.to_vec() };
        let budget = rrrah_core::MemoryBudget::new(64 * 1024 * 1024);
        let report = scan(input, p, &factors(), &budget, || false).unwrap();
        assert_eq!(report.analysed, [1,2,3]);
        assert!(report.file_issues.is_empty() && report.local.issues.is_empty() && report.local.source_issues.is_empty());
        for i in 0..3 {
            for j in i+1..3 {
                let fresh = compare(&files[i].1, &files[j].1, p.search, &factors(), &budget, || false).unwrap();
                let supported = fresh.regions.as_ref().is_some_and(|r| r.region_support_count() > 0);
                assert_eq!(supported, (i,j) == (0,1));
                let found = report.local.pairs.iter().find(|pair| (pair.left,pair.right) == (files[i].0,files[j].0));
                if supported { assert!(found.is_some()); }
                if let Some(pair) = found { assert_eq!(format!("{:?}",pair.evidence), format!("{fresh:?}")); }
                drop(fresh);
            }
        }
        drop(report);assert_eq!(budget.used(),0);
        assert!(scan(files.clone(), p, &factors(), &budget, || true).is_err());
        assert_eq!(budget.used(),0);
    }
}
''')
 keys=['crates/rrrah-dedup/src/gradient_scale.rs','crates/rrrah-dedup/src/local_scan.rs','crates/rrrah-dedup/src/local_collection.rs',key]
 subprocess.run(['rustfmt','--edition','2024','--config-path',str(R/'rustfmt.toml'),*[str(s/k)for k in keys]],check=True)
 hashes={k:sha(s/k)for k in m['file_hashes']};out=D/'dedup-zero-smoothing-borrow-source-v4-20261012.json';assert not out.exists();out.write_text(json.dumps({'status':'isolated_formatted_zero_borrow_candidate_with_zero_pair_collection_regression','snapshot':str(s),'parent_manifest':str(mp),'parent_sha256':sha(mp),'file_hashes':hashes,'deltas_from_v3':[k for k in hashes if hashes[k]!=m['file_hashes'][k]],'scope':'Same v3 production algorithms formatted using repository rustfmt config; unused isolated reference imports removed. New public integration regression covers radiuszero freshpositive/unrelatedpairs and collection equality in both input orders, cancellation and release. Not adopted; tests pending.'},indent=2)+'\n');print(s)
if __name__=='__main__':main()
