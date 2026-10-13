#!/usr/bin/env python3
import hashlib,json
from pathlib import Path
D=Path(__file__).resolve().parents[1]/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 p=D/'dedup-disc21-pixel-context-controls-20261011.json';raw=p.read_bytes();r=json.loads(raw);rows=[x for x in r['results'] if x['status']!='running'];n=len(rows);assert n and rows==r['results'][:n];capture=T/f'dedup-disc21-pixel-context-controls-prefix-{n:04d}-20261011.json';out=D/f'dedup-disc21-pixel-context-controls-prefix-{n:04d}-audit-20261011.json';assert not capture.exists() and not out.exists();capture.write_bytes(raw)
 pins=dict(r['pins']);assert all(sha(k)==v for k,v in pins.items());parent=D/'dedup-disc21-pixel-context-native-audit-20261011.json';a=json.loads(parent.read_text());assert a['status']=='verified_actual_cpu_four_context_positive_recovery' and all(sha(k)==v for k,v in a['pins'].items());pins.update(a['pins']);prep=json.loads((D/'dedup-disc21-cohort-prepared-20261011.json').read_text());images={x['id']:x['path'] for x in prep['images']};assert r['plan']==[prep['plan'][8],prep['plan'][6]] and r['required']==2
 for i,row in enumerate(rows):
  case=r['plan'][i];assert row['index']==i and all(row[k]==case[k] for k in ['query','reference','published_label']);assert row['command'][5:7]==[images[case['query']],images[case['reference']]] and row['command'][-2:]==['640','2.0'];assert sha(row['log'])==row['log_sha256'] and Path(row['log']).read_text()==row['stdout']+row['stderr'];pins[row['log']]=row['log_sha256']
  if row['returncode']:assert row['status']=='execution_failed';continue
  e=json.loads(row['stdout']);assert e==row['evidence'] and e['attempts']==25 and e['managed_used']==0 and type(e['supported']) is bool;assert row['status']==('supported' if e['supported'] else 'inconclusive')
 gaps=[x['query'] for x in rows if x['status']=='execution_failed' or (x['published_label']=='copy' and x['status']!='supported') or (x['published_label']=='no_reference_match' and x['status']=='supported')];pins.update({str(x):sha(x) for x in [capture,parent,Path(__file__)]});out.write_text(json.dumps({'status':'verified_four_context_controls_completed_prefix','completed':n,'required':2,'rows':[{'query':x['query'],'reference':x['reference'],'published_label':x['published_label'],'status':x['status']} for x in rows],'observed_acceptance_gaps':gaps,'pins':pins,'scope':'Bounded actual CPU controls only; unknown distractor is no observed false support, not definitive negative. Not general precision/all-case qualification.'},indent=2)+'\n');print(n,[(x['query'],x['status']) for x in rows])
if __name__=='__main__':main()
