#!/usr/bin/env python3
"""Freeze only completed noncopy pairs, retaining both directional traces."""
import hashlib,json
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 rp=D/'dedup-disc21-bidirectional-noncopy-20261012.json';raw=rp.read_bytes();r=json.loads(raw);n=r['completed'];assert 0<n<=3 and len(r['results'])==n
 out=D/f'dedup-disc21-bidirectional-noncopy-prefix-{n:04d}-audit-20261012.json';capture=out.with_name(out.stem+'-capture.json');assert not out.exists()and not capture.exists()
 assert all(sha(p)==v for p,v in r['pins'].items());prep=json.loads(Path(r['prepared']).read_text());assert r['plan']==prep['plan']and len(r['plan'])==3;assert all(sha(p)==v for p,v in prep['pins'].items())
 parent=next(Path(p)for p in prep['pins']if p.endswith('/dedup-disc21-architecture-controls-prepared-20261012.json'));images={x['id']:x['path']for x in json.loads(parent.read_text())['images']}
 for i,row in enumerate(r['results']):
  assert row['command'][5:7]==[images[r['plan'][i]['source']],images[r['plan'][i]['target']]];assert row['command'][-2:]==['640','2.0']
  assert row['index']==i and row['returncode']==0;assert sha(row['log'])==row['log_sha256']and Path(row['log']).read_text()==row['stdout']+row['stderr'];v=json.loads(row['stdout']);assert v['managed_used']==0 and v['managed_report_used']>0;assert v['evidence']['file_ids']==[1,2]and len(v['evidence']['pairs'])==1;p=v['evidence']['pairs'][0];assert (p['left'],p['right'])==(1,2)and not p['supported']and p['reverse']is not None
  for direction in ['forward','reverse']:
   e=p[direction];assert not e['supported']and e['primary']['attempts']==25 and e['primary']['transform']is None;recovery=e['recovery'];assert recovery is not None and recovery['search']['attempts']==9 and not recovery['search']['supported']and recovery['search']['transform']is None and recovery['exposures']=='None'
 if n==3:assert r['status']=='complete_three_unordered_noncopy_pairs'
 capture.write_bytes(raw);out.write_text(json.dumps({'status':'verified_completed_bidirectional_noncopy_prefix','completed_pairs':n,'required_pairs':3,'completed_directions':2*n,'false_supports':0,'execution_failures':0,'pins':{str(p):sha(p)for p in [capture,Path(__file__)]},'scope':'Original JPEGs, fixed manually inspected development noncopy plan, actual cached CPU collection both directions with full primary/recovery traces and released managed credit. Not independent heldout semantic/burst precision, full-corpus/platform/formats/RSS/full500 completion.'},indent=2)+'\n');print(f'Verified{n}/3 noncopy pairs /{2*n}directions')
if __name__=='__main__':main()
