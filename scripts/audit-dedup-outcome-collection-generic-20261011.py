#!/usr/bin/env python3
"""Verify dependency-only collection source guards and header-binding regressions."""
import hashlib,json
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1];D=ROOT/'docs/research'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 mp=D/'dedup-outcome-collection-snapshot-20261011.json';gp=D/'dedup-outcome-collection-generic-20261011.json'
 m=json.loads(mp.read_text());g=json.loads(gp.read_text())
 assert g['status']=='complete_bounded_frozen_outcome_collection_generic_execution' and g['tests']==202 and len(g['results'])==24 and g['build_returncode']==0
 assert g['source_manifest_sha256']==sha(mp)
 for p,h in g['pins'].items():assert sha(p)==h,p
 for p,h in m['file_hashes'].items():assert sha(Path(m['snapshot'])/p)==h,p
 assert m['changes']==['crates/rrrah-dedup/src/portfolio_collection.rs','crates/rrrah-dedup/src/model_loftr.rs','crates/rrrah-dedup/tests/portfolio_collection.rs','scripts/dedup-loftr-native-probe/src/bin/tile_outcome_collection_files.rs']
 for p in m['changes']:assert sha(ROOT/p)==m['file_hashes'][p],p
 pp=Path(m['parent_manifest']);assert sha(pp)==m['parent_manifest_sha256'];parent=json.loads(pp.read_text())
 assert all(m['file_hashes'][p]==h for p,h in parent['file_hashes'].items() if p not in m['changes'])
 total=0
 for r in g['results']:
  assert r['status']=='passed' and r['returncode']==0 and sha(r['binary'])==r['binary_sha256'] and sha(r['log'])==r['log_sha256']
  total+=r['tests']
 assert total==202
 suites={r['suite']:r['tests'] for r in g['results']}
 assert all(suites[k]==v for k,v in {'decode':14,'wal_dependency':4,'scan':7,'raster_equality':7,'rrrah_dedup':9,'projected_portfolio':24,'portfolio_collection':12}.items())
 out={'status':'complete_bounded_frozen_explicit_unknown_collection_generic_qualification','generic_tests':202,'programs':24,'frozen_files':len(m['file_hashes']),'current_changed_sources_match':True,'pins':{str(p):sha(p) for p in [mp,gp,pp,Path(__file__)]},'scope':'Explicit unknown collection API retains typed insufficient geometry as records while sharing the legacy engine; fatal errors and all guards remain unchanged.','limits':['macOS generic provider qualification; current native Torch and Linux source not yet qualified','Latency benefit is unmeasured; no speedup claim','Intermediate restored concurrent rewrites and atomic filesystem snapshots remain outside existing observation contract','Full corpus precision, indexed scaling, provider RSS and all-format qualification remain open']}
 (D/'dedup-outcome-collection-generic-audit-20261011.json').write_text(json.dumps(out,indent=2)+'\n');print(out['status'],total)
if __name__=='__main__':main()
