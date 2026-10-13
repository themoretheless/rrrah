#!/usr/bin/env python3
"""Verify dependency-only collection source guards and header-binding regressions."""
import hashlib,json
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1];D=ROOT/'docs/research'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 mp=D/'dedup-dependency-observation-snapshot-20261011.json';gp=D/'dedup-dependency-observation-generic-20261011.json'
 m=json.loads(mp.read_text());g=json.loads(gp.read_text())
 assert g['status']=='complete_bounded_frozen_dependency_observation_generic_execution' and g['tests']==200 and len(g['results'])==24 and g['build_returncode']==0
 assert g['source_manifest_sha256']==sha(mp)
 for p,h in g['pins'].items():assert sha(p)==h,p
 for p,h in m['file_hashes'].items():assert sha(Path(m['snapshot'])/p)==h,p
 assert m['changes']==['crates/rrrah-dedup/src/collection.rs','crates/rrrah-dedup/src/decode.rs','crates/rrrah-dedup/src/exact.rs']
 for p in m['changes']:assert sha(ROOT/p)==m['file_hashes'][p],p
 pp=Path(m['parent_manifest']);assert sha(pp)==m['parent_manifest_sha256'];parent=json.loads(pp.read_text())
 assert all(m['file_hashes'][p]==h for p,h in parent['file_hashes'].items() if p not in m['changes'])
 total=0
 for r in g['results']:
  assert r['status']=='passed' and r['returncode']==0 and sha(r['binary'])==r['binary_sha256'] and sha(r['log'])==r['log_sha256']
  total+=r['tests']
 assert total==200
 suites={r['suite']:r['tests'] for r in g['results']}
 assert all(suites[k]==v for k,v in {'decode':14,'wal_dependency':4,'scan':7,'raster_equality':7,'rrrah_dedup':9,'projected_portfolio':24}.items())
 out={'status':'complete_bounded_frozen_dependency_only_observation_qualification','generic_tests':200,'programs':24,'frozen_files':len(m['file_hashes']),'current_changed_sources_match':True,'pins':{str(p):sha(p) for p in [mp,gp,pp,Path(__file__)]},'scope':'Exact collections observe original file once per guard plus independent external dependencies. Activation binds header to original full digest; ordinary full visual source snapshots retain their existing checks and cache recipe data.','limits':['macOS generic provider qualification; current native Torch and Linux source not yet qualified','Latency benefit is unmeasured; no speedup claim','Intermediate restored concurrent rewrites and atomic filesystem snapshots remain outside existing observation contract','Full corpus precision, indexed scaling, provider RSS and all-format qualification remain open']}
 (D/'dedup-dependency-observation-audit-20261011.json').write_text(json.dumps(out,indent=2)+'\n');print(out['status'],total)
if __name__=='__main__':main()
