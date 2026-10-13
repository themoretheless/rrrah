#!/usr/bin/env python3
import hashlib,json,re
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 out=D/'dedup-disc21-overlap-calibrated-strong-audit-20261011.json';assert not out.exists();rp=D/'dedup-disc21-overlap-calibrated-strong-20261011.json';r=json.loads(rp.read_text());assert r['status']=='complete_two_development_controls' and len(r['results'])==2 and r['build_returncode']==0;assert all(sha(p)==h for p,h in r['pins'].items());assert sha(r['build_log'])==r['build_log_sha256']
 gate=D/'dedup-exposure-anchor-files-verification-20261011.json';g=json.loads(gate.read_text());assert g['tests']==172 and all(sha(p)==h for p,h in g['pins'].items())
 parent=D/'dedup-exposure-anchor-files-snapshot-20261011.json';m=json.loads(parent.read_text());snap=Path(m['snapshot']);copied=next(Path(p).parents[2] for p in r['pins'] if p.endswith('/crates/rrrah-dedup/Cargo.toml'));assert all(sha(copied/f)==h for f,h in m['file_hashes'].items() if f!='crates/rrrah-dedup/src/model_loftr.rs');assert sha(copied/'crates/rrrah-dedup/src/model_loftr.rs')==sha(R/'crates/rrrah-dedup/src/model_loftr.rs')
 source=next(Path(p) for p in r['pins'] if p.endswith('/src/bin/disc21_overlap_calibrated_strong_files.rs'));text=source.read_text()
 for literal in ['minimum_contrast: 0.005','minimum_witnesses: 20','minimum_agreement: 0.9','minimum_pairs: 1000','compare_exposure_anchor_rank_files','6*policy.decode.max_pixels']:assert literal in text
 baseline=json.loads(json.loads((D/'dedup-disc21-overlap-regions-native-v2-20261011.json').read_text())['stdout'])['evidence'];summaries=[]
 for row in r['results']:
  assert row['status']=='complete' and row['returncode']==0;v=json.loads(row['stdout']);assert v['managed_used']==0;e=v['evidence'];assert e['bank_count']==9 and len(e['banks'])==9
  ld,rd=e['original_dimensions'];w,h=ld[0]//2,ld[1]//2;rectangles=[[x,y,w,h] for y in [0,ld[1]//4,ld[1]//2] for x in [0,ld[0]//4,ld[0]//2]];assert e['source_rectangles']==rectangles and e['target_rectangle']==[0,0,*rd]
  accepted=[]
  for attempt in e['pixel_attempts']:
   assert len(attempt['witnesses'])>=20
   if attempt['pixels']=='insufficient_geometry':assert not attempt['supported'];continue
   counts=[tuple(map(int,x)) for x in re.findall(r'informative_pairs: (\d+), agreeing_pairs: (\d+)',attempt['pixels'])];assert len(counts)==2;support=all(n>=1000 and a/n>=0.9 for n,a in counts);assert attempt['supported']==support
   assert 'exposures: [RankExposure' in attempt['pixels']
   if support:accepted.append(attempt)
  assert e['supported']==bool(accepted)
  if row['label']=='copy':
   assert e['banks']==baseline['banks'];assert e['supported'] and e['selected_bank']==7 and e['geometry_inliers']==24
   a=accepted[0];assert a['rank_radius']==2 and a['window_radius']==2;assert sum(199<=p['source'][0]<=373 and 694<=p['source'][1]<=867 for p in a['witnesses'])==23
  else:assert not e['supported']
  summaries.append({'query':row['query'],'reference':row['reference'],'label':row['label'],'supported':e['supported'],'bank':e['selected_bank'],'geometry_inliers':e['geometry_inliers'],'pixel_attempts':len(e['pixel_attempts']),'managed_peak':v['managed_peak']})
 pins={str(p):sha(p) for p in [rp,gate,parent,source,Path(__file__),D/'dedup-disc21-overlap-regions-native-v2-20261011.json']}
 report={'status':'verified_automatic_cpu_calibrated_strong_two_development_controls','pins':pins,'summaries':summaries,'scope':'Actual CPU9-region automatic extraction and explicit guarded calibrated pixel API; positive proposal banks exactly equal original overlap baseline. One dark positive recovered with24 witnesses,23 inside manual development annotation; one easy unrelated negative remains inconclusive. Min20 geometry was selected using this development case. No held-out negative precision, default adoption, full cohort/corpus/formats/platforms/RSS/all500 qualification. Public file API and unchanged legacy paths separately covered by172 tests.'};out.write_text(json.dumps(report,indent=2)+'\n');print(report['status']);print(summaries)
if __name__=='__main__':main()
