#!/usr/bin/env python3
"""Audit actual CPU directions without assuming that forward must miss."""
import hashlib,json
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 out=D/'dedup-disc21-bidirectional-exposure-collection-audit-20261012.json';assert not out.exists()
 rp=D/'dedup-disc21-bidirectional-exposure-collection-20261012.json';r=json.loads(rp.read_text());assert r['status']=='complete_bidirectional_collection_control' and r['build_returncode']==r['returncode']==0
 assert all(sha(p)==v for p,v in r['pins'].items());assert sha(r['build_log'])==r['build_log_sha256']
 mp=D/'dedup-bidirectional-exposure-collection-snapshot-20261012.json';m=json.loads(mp.read_text());snapshot=next(Path(p).parents[2]for p in r['pins']if p.endswith('/crates/rrrah-dedup/Cargo.toml')and 'bidirectional-exposure-collection-source'in p);assert all(sha(snapshot/k)==v for k,v in m['file_hashes'].items())
 gp=D/'dedup-bidirectional-exposure-collection-verification-20261012.json';g=json.loads(gp.read_text());assert g['tests']==46 and all(sha(p)==v for p,v in g['pins'].items())
 v=json.loads(r['stdout']);assert v['managed_used']==0 and v['managed_report_used']>0;assert v['evidence']['file_ids']==[1,2];pairs=v['evidence']['pairs'];assert len(pairs)==1;p=pairs[0];assert (p['left'],p['right'])==(1,2)and p['supported']
 forward=p['forward'];reverse=p['reverse'];assert forward['primary']['attempts']==25
 if forward['supported']:
  assert reverse is None;direction='forward_support_reverse_skipped';scope='Reversed-ID pair supported in forward direction. This control does not establish reverse recovery execution.'
 else:
  assert forward['recovery']['search']['attempts']==9 and reverse is not None and reverse['supported'];direction='reverse_support_after_forward_miss'
  old=D/'dedup-disc21-exposure-recovery-api-20261012.json';b=json.loads(old.read_text());expected=json.loads(next(x for x in b['results']if x['query']=='Q00906')['stdout'])['evidence'];assert reverse['primary']==expected['primary']and reverse['recovery']==expected['recovery'];scope='Actual reversed-ID CPU control misses forward and supports reverse, with directed evidence identical to qualified Q00906 pair.'
 out.write_text(json.dumps({'status':'verified_actual_cpu_bidirectional_exposure_collection','direction':direction,'pins':{str(p):sha(p)for p in [rp,mp,gp,Path(__file__)]},'managed_report_used':v['managed_report_used'],'managed_peak':v['managed_peak'],'scope':scope+' Development control only; not default integration, heldout precision, total RSS, full corpus/formats/platforms or full500 completion.'},indent=2)+'\n');print(direction)
if __name__=='__main__':main()
