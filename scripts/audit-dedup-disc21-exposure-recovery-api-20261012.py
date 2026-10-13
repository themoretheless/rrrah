#!/usr/bin/env python3
import hashlib,json,re
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def checked(p):
 d=json.loads(p.read_text());assert all(sha(k)==v for k,v in d['pins'].items());return d

def main():
 out=D/'dedup-disc21-exposure-recovery-api-audit-20261012.json';assert not out.exists()
 rp=D/'dedup-disc21-exposure-recovery-api-20261012.json';r=checked(rp)
 assert r['status']=='complete_three_development_controls' and r['build_returncode']==0
 assert sha(r['build_log'])==r['build_log_sha256']
 gate=D/'dedup-exposure-recovery-search-verification-20261012.json';g=checked(gate);assert g['tests']==184
 mp=D/'dedup-exposure-recovery-search-snapshot-20261012.json';m=json.loads(mp.read_text())
 snapshot=next(Path(p).parents[2] for p in r['pins'] if p.endswith('/crates/rrrah-dedup/Cargo.toml'))
 assert all(sha(snapshot/f)==h for f,h in m['file_hashes'].items())
 prior=D/'dedup-disc21-pixel-context-native-audit-20261011.json';a=checked(prior)['evidence']
 overlap_audit=D/'dedup-disc21-overlap-cached-exposure-audit-20261012.json';checked(overlap_audit)
 overlap_report=D/'dedup-disc21-overlap-cached-exposure-20261012.json';b=checked(overlap_report)
 assert [(x['query'],x['reference']) for x in r['results']]==[('Q00061','R020965'),('Q00906','R014120'),('Q00000','R020965')]
 summaries=[]
 for row in r['results']:
  assert row['status']=='complete' and row['returncode']==0
  v=json.loads(row['stdout']);assert v['managed_used']==0;e=v['evidence'];p=e['primary'];assert p['attempts']==25
  if row['query']=='Q00061':
   assert e['supported'] and p['supported'] and e['recovery'] is None
   for k in ['transform','original_dimensions','raw_proposals','selected_points','geometry_inliers','pixel_rank_radius','pixel_window_radius']:assert p[k]==a[k]
   assert p['selected_bank']==a['hypothesis_bank'] and p['pixel_attempts']==2
   strip=lambda s:re.sub(r'source_window_bounds: \[[^]]+\], target_window_bounds: \[[^]]+\], ','',s)
   assert strip(p['pixels'])==a['pixels']
  else:
   assert not p['supported'] and p['transform'] is None and p['pixels']=='None'
   previous=next(x for x in b['results'] if x['query']==row['query']);old=json.loads(previous['stdout'])['evidence'];recovery=e['recovery'];assert recovery is not None
   search=recovery['search'];assert search['attempts']==9 and e['supported']==search['supported']==old['supported']
   assert recovery['exposures']==old['exposures']
   for k in ['transform','pixels','selected_bank','original_dimensions','raw_proposals','selected_points','geometry_inliers','pixel_attempts','pixel_rank_radius','pixel_window_radius']:assert search[k]==old[k]
  summaries.append({'query':row['query'],'reference':row['reference'],'supported':e['supported'],'primary_supported':p['supported'],'recovery_attempted':e['recovery'] is not None,'managed_peak':v['managed_peak'],'managed_used':v['managed_used']})
 source=next(Path(p) for p in r['pins'] if p.endswith('/src/bin/disc21_exposure_recovery_files.rs'))
 assert 'search_loftr_model_banks_exposure_recovery_files' in source.read_text()
 pins={str(p):sha(p) for p in [rp,gate,mp,prior,overlap_audit,overlap_report,source,Path(__file__)]}
 report={'status':'verified_actual_cpu_guarded_primary_then_exposure_recovery_controls','pins':pins,'summaries':summaries,'scope':'Three development controls through actual public composed CPU API. Primary collage support preserves previous geometry/pixel evidence and skips recovery. Dark primary miss advances to recovery with exact prior cached-overlap evidence. Negative remains inconclusive through both stages. Generic184 gate covers policy/source/owner atomicity. No complete cohort, held-out precision, default adoption, performance/RSS, full-format/platform/all500 qualification.'}
 out.write_text(json.dumps(report,indent=2)+'\n');print(report['status']);print(summaries)
if __name__=='__main__':main()
