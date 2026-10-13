#!/usr/bin/env python3
"""Verify current dependency-only observations against native controls and generic evidence."""
import hashlib,json
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1];D=ROOT/'docs/research'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 rp=D/'dedup-dependency-observation-native-20261011.json';mp=D/'dedup-dependency-observation-snapshot-20261011.json'
 r=json.loads(rp.read_text());m=json.loads(mp.read_text())
 assert r['status']=='complete_bounded_frozen_dependency_observation_native_controls' and r['invocations']==9 and r['all_expected_outcomes_match']
 for p,h in r['pins'].items():assert sha(p)==h,p
 lineage=[mp]
 assert m['changes']==['crates/rrrah-dedup/src/collection.rs','crates/rrrah-dedup/src/decode.rs','crates/rrrah-dedup/src/exact.rs']
 for p,h in m['file_hashes'].items():assert sha(Path(m['snapshot'])/p)==h,p
 parent_path=Path(m['parent_manifest']);assert sha(parent_path)==m['parent_manifest_sha256'];lineage.append(parent_path);parent=json.loads(parent_path.read_text())
 assert set(m['file_hashes'])==set(parent['file_hashes'])
 assert sorted(p for p,h in m['file_hashes'].items() if parent['file_hashes'][p]!=h)==m['changes']
 final=m
 for p in m['changes']+['crates/rrrah-dedup/src/model_loftr.rs','scripts/dedup-loftr-native-probe/src/bin/exact_collection_files.rs']:
  assert sha(ROOT/p)==m['file_hashes'][p],p
 prior_path=D/'dedup-exact-loftr-native-20261011.json'
 prior={x['case']:x['evidence']['result'] for x in json.loads(prior_path.read_text())['results'] if 'evidence' in x}
 counts={'exact_bytes':0,'complete':0,'inconclusive':0};errors=0
 for row in r['results']:
  assert row['status']=='passed' and sha(row['log'])==row['log_sha256']
  if row['expected'] in ['missing','files']:assert row['returncode']==1;errors+=1
  else:
   assert row['returncode']==0 and row['evidence']['managed_used']==0
   for pair in row['evidence']['result']['pairs']:counts[pair['evidence']['outcome']]+=1
   if row['expected'] in ['visual','unknown']:
    assert row['independent_pair_evidence_exact']
    control=prior['visual_alpha_'+row['case'].removeprefix('mixed_visual_')] if row['expected']=='visual' else prior['collision_alpha_native-first']
    assert all(pair['evidence']==control for pair in row['evidence']['result']['pairs'][1:])
   assert json.loads(row['stdout'])==row['evidence']
 assert counts=={'exact_bytes':9,'complete':4,'inconclusive':2} and errors==2
 gp=D/'dedup-dependency-observation-generic-20261011.json';g=json.loads(gp.read_text());assert g['tests']==200 and len(g['results'])==24
 assert g['status']=='complete_bounded_frozen_dependency_observation_generic_execution'
 for p,h in g['pins'].items():assert sha(p)==h,p
 for row in g['results']:assert sha(row['binary'])==row['binary_sha256'] and sha(row['log'])==row['log_sha256'] and row['status']=='passed'
 out={'status':'complete_bounded_dependency_observation_native_qualification','native_invocations':9,'pair_records':15,'pair_outcomes':counts,'error_controls':errors,'generic_tests':200,'frozen_files':len(final['file_hashes']),'current_changed_sources_match':True,'pins':{str(p):sha(p) for p in [rp,*lineage,gp,prior_path,Path(__file__)]},'limits':['macOS local CPU Torch only','Exhaustive enumeration, not indexed scaling or cross-pair decoded-image reuse','Provider RSS, full corpus precision and all-format qualification remain open']}
 (D/'dedup-dependency-observation-native-audit-20261011.json').write_text(json.dumps(out,indent=2)+'\n');print(out['status'])
if __name__=='__main__':main()
