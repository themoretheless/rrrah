#!/usr/bin/env python3
"""Close frozen exact-first native LoFTR controls and inherited generic guards."""
import hashlib,json
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1];D=ROOT/'docs/research'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 report_path=D/'dedup-exact-loftr-native-20261011.json';manifest_path=D/'dedup-exact-loftr-snapshot-20261011.json'
 r=json.loads(report_path.read_text());m=json.loads(manifest_path.read_text())
 assert r['status']=='complete_bounded_frozen_exact_first_loftr_native_controls'
 assert r['invocations']==11 and r['all_expected_outcomes_match']
 assert [r[k] for k in ['exact_controls','visual_controls','unknown_controls','error_controls']]==[5,3,2,1]
 for p,h in r['pins'].items():assert sha(p)==h,p
 for p,h in m['file_hashes'].items():assert sha(Path(m['snapshot'])/p)==h,p
 parent_path=Path(m['parent_manifest']);assert sha(parent_path)==m['parent_manifest_sha256']
 parent=json.loads(parent_path.read_text());assert m['changes']==['crates/rrrah-dedup/src/model_loftr.rs','scripts/dedup-loftr-native-probe/src/bin/exact_portfolio_files.rs']
 assert all(m['file_hashes'][p]==h for p,h in parent['file_hashes'].items() if p not in m['changes'])
 current=['crates/rrrah-dedup/src/exact.rs','crates/rrrah-dedup/src/exact_portfolio.rs','crates/rrrah-dedup/src/lib.rs','crates/rrrah-dedup/tests/projected_portfolio.rs',*m['changes']]
 for p in current:assert sha(ROOT/p)==m['file_hashes'][p],p
 for row in r['results']:
  assert row['status']=='passed' and sha(row['log'])==row['log_sha256']
  assert row['returncode']==(1 if row['expected']=='error' else 0)
  if row['expected'] in ['visual','unknown']:assert row['independent_visual_evidence_exact']
 generic_path=D/'dedup-exact-portfolio-fixed-generic-20261011.json';g=json.loads(generic_path.read_text())
 assert g['status']=='complete_bounded_frozen_exact_first_portfolio_execution' and g['tests']==33
 for p,h in g['pins'].items():assert sha(p)==h,p
 for row in g['results']:
  assert sha(row['binary'])==row['binary_sha256'] and sha(row['log'])==row['log_sha256']
 out={'status':'complete_bounded_exact_first_loftr_native_and_generic_qualification','native_invocations':11,'generic_tests':33,'frozen_files':len(m['file_hashes']),'current_changed_sources_match':True,'pins':{str(p):sha(p) for p in [report_path,manifest_path,parent_path,generic_path,Path(__file__)]},'limits':['macOS CPU local Torch only; no cross-platform extension','No collection exact-first adapter yet','Provider RSS, large corpus precision and full format coverage remain open']}
 (D/'dedup-exact-loftr-native-audit-20261011.json').write_text(json.dumps(out,indent=2)+'\n');print(out['status'])
if __name__=='__main__':main()
