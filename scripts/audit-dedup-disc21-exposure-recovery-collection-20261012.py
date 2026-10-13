#!/usr/bin/env python3
import hashlib,json
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 out=D/'dedup-disc21-exposure-recovery-collection-audit-20261012.json';assert not out.exists();rp=D/'dedup-disc21-exposure-recovery-collection-20261012.json';r=json.loads(rp.read_text());assert r['status']=='complete_three_source_collection_control' and r['build_returncode']==r['returncode']==0;assert all(sha(p)==v for p,v in r['pins'].items());assert sha(r['build_log'])==r['build_log_sha256']
 gate=D/'dedup-exposure-recovery-collection-verification-20261012.json';g=json.loads(gate.read_text());assert g['tests']==223 and all(sha(p)==v for p,v in g['pins'].items());parent=D/'dedup-exposure-recovery-collection-snapshot-20261012.json';m=json.loads(parent.read_text());snapshot=next(Path(p).parents[2]for p in r['pins']if p.endswith('/crates/rrrah-dedup/Cargo.toml') and 'exposure-recovery-collection-source' in p);assert all(sha(snapshot/k)==v for k,v in m['file_hashes'].items())
 v=json.loads(r['stdout']);assert v['managed_used']==0 and v['managed_report_used']>0;e=v['evidence'];assert e['file_ids']==[1,2,3];pairs=e['pairs'];assert [(p['left'],p['right'])for p in pairs]==[(1,2),(1,3),(2,3)] and [p['supported']for p in pairs]==[True,False,False]
 old=D/'dedup-disc21-exposure-recovery-api-20261012.json';b=json.loads(old.read_text());prior=next(p for p in b['results']if p['query']=='Q00906');expected=json.loads(prior['stdout'])['evidence'];assert pairs[0]['primary']==expected['primary'] and pairs[0]['recovery']==expected['recovery']
 for p in pairs:
  assert p['primary']['attempts']==25
  if p['primary']['supported']:assert p['recovery']is None
  else:assert p['recovery']is not None and p['recovery']['search']['attempts']==9
  if not p['supported']:assert p['primary']['transform']is None and p['recovery']['search']['transform']is None and p['recovery']['exposures']=='None'
 source=next(Path(p)for p in r['pins']if p.endswith('/src/bin/disc21_exposure_recovery_collection.rs'));assert 'scan_loftr_model_banks_exposure_recovery_collection'in source.read_text();wrapper=(snapshot/'crates/rrrah-dedup/src/model_loftr.rs').read_text();assert 'let mut model=None;'in wrapper and 'proposals_with_model(&mut model,weights,left,right,cap,budget,cancel)'in wrapper
 paths=[rp,gate,parent,old,source,Path(__file__)];out.write_text(json.dumps({'status':'verified_actual_cpu_three_source_exposure_recovery_collection','pins':{str(p):sha(p)for p in paths},'pair_support':[[1,2,True],[1,3,False],[2,3,False]],'managed_report_used':v['managed_report_used'],'managed_peak':v['managed_peak'],'scope':'Actual public three-distinct-source collection and cached CPU wrapper. Positive primary/recovery evidence exactly matches separately verified pair operation; two cross-controls remain inconclusive. Report retains managed ownership until drop; used0 after drop. Shared model Option source inspected, no instrumented load counter/performance claim. Canonical-ID direction only, no opposite search/transitive grouping/default portfolio/heldout precision/full458/formats/platforms/RSS/full500 qualification.'},indent=2)+'\n');print('Verified actual CPU three-source guarded exposure recovery collection')
if __name__=='__main__':main()
