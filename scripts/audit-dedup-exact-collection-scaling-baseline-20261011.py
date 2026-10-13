#!/usr/bin/env python3
"""Verify frozen production/measurement identity and bounded scaling samples."""
import hashlib,json,statistics
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1];D=ROOT/'docs/research'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 rp=D/'dedup-exact-collection-scaling-baseline-20261011.json';mp=D/'dedup-exact-collection-scaling-baseline-snapshot-20261011.json'
 r=json.loads(rp.read_text());m=json.loads(mp.read_text());assert r['status']=='complete_frozen_all_copy_collection_scaling_baseline' and r['returncode']==r['build_returncode']==0
 assert r['controls']==3 and r['samples']==9 and r['source_manifest_sha256']==sha(mp)
 for p,h in r['pins'].items():assert sha(p)==h,p
 for p,h in m['file_hashes'].items():assert sha(Path(m['snapshot'])/p)==h,p
 pp=Path(m['parent_manifest']);assert sha(pp)==m['parent_manifest_sha256'];parent=json.loads(pp.read_text())
 assert m['changes']==['crates/rrrah-dedup/tests/projected_portfolio.rs']
 assert all(m['file_hashes'][p]==h for p,h in parent['file_hashes'].items() if p not in m['changes'])
 for p,h in m['file_hashes'].items():
  if p.startswith('crates/rrrah-dedup/src/') or p in m['changes']:assert sha(ROOT/p)==h,p
 assert [x['files'] for x in r['rows']]==[8,32,64]
 for row in r['rows']:
  n=row['files'];pairs=n*(n-1)//2;assert row['pairs']==row['exact_pairs']==pairs
  assert row['families']==1 and row['file_bytes']==1048576 and len(row['elapsed_ns'])==3
  assert all(v>0 for v in row['elapsed_ns']) and row['median_seconds']==statistics.median(row['elapsed_ns'])/1e9
  assert row['analytical_source_bytes']==(4*n+6*pairs)*row['file_bytes']
 out={'status':'complete_bounded_all_copy_collection_scaling_baseline_audit','controls':3,'samples':9,'frozen_files':len(m['file_hashes']),'production_unchanged':True,'rows':r['rows'],'next_action':'Evaluate managed operation-local exact equivalence certificates to avoid repeated byte confirmation while retaining every existing per-pair and final source guard; qualify collision partitioning, cancellation, memory admission and mixed collections before adoption.','pins':{str(p):sha(p) for p in [rp,mp,pp,Path(__file__)]},'limits':['Elapsed times from a shared macOS host; not a universal benchmark','Payload byte counts are analytical, not measured physical I/O','All-copy collections only; no broad visual scaling or RSS qualification']}
 (D/'dedup-exact-collection-scaling-baseline-audit-20261011.json').write_text(json.dumps(out,indent=2)+'\n');print(out['status'])
if __name__=='__main__':main()
