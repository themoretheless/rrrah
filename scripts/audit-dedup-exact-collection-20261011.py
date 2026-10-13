#!/usr/bin/env python3
"""Close complete exact-first collection and shared visual regression evidence."""
import hashlib,json
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1];D=ROOT/'docs/research'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 manifest_path=D/'dedup-exact-collection-snapshot-20261011.json';gate_path=D/'dedup-exact-collection-generic-20261011.json'
 m=json.loads(manifest_path.read_text());g=json.loads(gate_path.read_text())
 assert g['status']=='complete_bounded_frozen_projected_portfolio_generic_execution'
 assert g['tests']==159 and g['build_returncode']==0 and len(g['results'])==19
 assert g['source_manifest_sha256']==sha(manifest_path)
 for p,h in g['pins'].items():assert sha(p)==h,p
 for p,h in m['file_hashes'].items():assert sha(Path(m['snapshot'])/p)==h,p
 for p in m['changes']:assert sha(ROOT/p)==m['file_hashes'][p],p
 parent_path=Path(m['parent_manifest']);assert sha(parent_path)==m['parent_manifest_sha256'];parent=json.loads(parent_path.read_text())
 assert all(m['file_hashes'][p]==h for p,h in parent['file_hashes'].items() if p not in m['changes'])
 total=0
 for r in g['results']:
  assert r['status']=='passed' and r['returncode']==0
  assert sha(r['binary'])==r['binary_sha256'] and sha(r['log'])==r['log_sha256']
  total+=r['tests']
 assert total==159
 log=D/'dedup-exact-collection-byte-collision-20261011.log'
 assert 'test result: ok. 7 passed; 0 failed;' in log.read_text()
 out={'status':'complete_bounded_frozen_exact_first_collection_generic_qualification','generic_tests':159,'generic_programs':19,'root_exact_collision_tests':7,'frozen_files':len(m['file_hashes']),'current_changed_sources_match':True,'pins':{str(p):sha(p) for p in [manifest_path,gate_path,parent_path,log,Path(__file__)]},'limits':['macOS generic providers; no current native LoFTR collection adapter qualification','Exhaustive enumeration, not indexed collection scaling or shared decoded-image cache','Provider RSS and full corpus/all-format coverage remain open']}
 (D/'dedup-exact-collection-audit-20261011.json').write_text(json.dumps(out,indent=2)+'\n');print(out['status'],total)
if __name__=='__main__':main()
