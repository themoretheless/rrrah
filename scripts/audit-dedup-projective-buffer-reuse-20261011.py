#!/usr/bin/env python3
import hashlib,json
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 out=D/'dedup-projective-buffer-reuse-verification-20261011.json';assert not out.exists();rp=D/'dedup-projective-buffer-reuse-parity-20261011.json';r=json.loads(rp.read_text());assert r['status']=='complete_four_real_point_buffer_parity' and len(r['results'])==len(r['plan'])==4;assert all(sha(p)==h for p,h in r['pins'].items());pins=dict(r['pins']);pins.update({str(rp):sha(rp),str(Path(__file__)):sha(__file__)})
 for plan,row in zip(r['plan'],r['results']):
  reference=json.loads(Path(plan['reference_report']).read_text());assert all(sha(p)==h for p,h in reference['pins'].items());expected=json.loads(reference['results'][plan['reference_index']]['stdout']);assert plan['expected']==expected;assert row['returncode']==0 and row['status']=='exact_whole_json_parity';assert sha(row['log'])==row['log_sha256'];assert Path(row['log']).read_text()==row['stdout']+row['stderr'];assert json.loads(row['stdout'])==expected;assert expected['managed_used']==0;pins[row['log']]=row['log_sha256']
 gp=D/'dedup-projective-buffer-reuse-countfix-audit-20261011.json';g=json.loads(gp.read_text());assert g['status']=='complete_bounded_frozen_joint_model_tests' and g['tests']==17;assert all(sha(p)==h for p,h in g['pins'].items());m=json.loads(Path(g['source_manifest']).read_text());assert all(sha(Path(m['snapshot'])/f)==h for f,h in m['file_hashes'].items());assert sha(R/'crates/rrrah-dedup/src/geometry.rs')==m['file_hashes']['crates/rrrah-dedup/src/geometry.rs'];pins[str(gp)]=sha(gp)
 for row in g['results']:
  assert sha(row['binary'])==row['binary_sha256'] and sha(row['log'])==row['log_sha256'];pins[row['binary']]=row['binary_sha256'];pins[row['log']]=row['log_sha256']
 assert all(sha(p)==h for p,h in pins.items());out.write_text(json.dumps({'status':'verified_17_tests_four_exact_real_controls_buffer_reuse','pins':pins,'tests':17,'real_controls':4,'scope':'Inlier vector reuse in shared sampled target/joint fitter. Four complete JSON results exactly match old allocating fitter, including H, indices, normalized error, original pixels and managed fields. Geometry/domain/joint cancellation/regression tests pass. No total RSS or speed improvement quantified; CPU collection gate used previous frozen geometry.'},indent=2)+'\n');print('Verified17 tests and4 exact real control outputs')
if __name__=='__main__':main()
