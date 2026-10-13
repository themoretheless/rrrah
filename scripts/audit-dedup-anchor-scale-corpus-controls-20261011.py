#!/usr/bin/env python3
import hashlib,json,re
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 report=D/'dedup-anchor-scale-corpus-controls-20261011.json';raw=report.read_bytes();r=json.loads(raw);closed=[]
 for row in r['results']:
  if row['status']=='running':break
  closed.append(row)
 assert closed;out=D/f'dedup-anchor-scale-corpus-controls-prefix-{len(closed):04d}-audit-20261011.json';assert not out.exists();capture=Path(r['plan'][0]['input']).parent/f'prefix-{len(closed):04d}.json';assert not capture.exists();capture.write_bytes(raw);assert all(sha(p)==h for p,h in r['pins'].items());pins=dict(r['pins']);pins.update({str(capture):sha(capture),str(Path(__file__)):sha(__file__)});decisions=[]
 baseline=json.loads((D/'dedup-model-joint-refit-20261011.json').read_text());best=json.loads(baseline['results'][1]['stdout'])['best'];inventory={x['target']:x for x in r['inventory']};expected=[]
 for item in r['inventory']:
  if item['role']=='excluded_incompatible_domain':continue
  assert item['dimensions']==[2560,1920]
  expected.extend((item['target'],radius) for radius in [8,12,16])
 assert [(p['target'],p['filter_radius']) for p in r['plan']]==expected
 for index,row in enumerate(closed):
  assert row['index']==index and row['returncode']==0;plan=r['plan'][index];assert all(row[k]==value for k,value in plan.items());assert sha(row['log'])==row['log_sha256'];assert Path(row['log']).read_text()==row['stdout']+row['stderr'];pins[row['log']]=row['log_sha256'];v=json.loads(row['stdout']);assert v['managed_used']==0 and v['filter_radius']==row['filter_radius'];assert v['best']==best and v['source_tolerance']==0.5414368903667351 and v['target_tolerance']==2 and v['trials']==1000000 and v['seed']==173;e=v['anchor_result'];counts=[]
  for line in row['stderr'].splitlines():
   assert line.startswith('RRRAH_ANCHOR_WINDOW\t');c=dict((k,int(n)) for k,n in re.findall(r'(sites|valid_sites|informative_pairs|agreeing_pairs|pixel_reads): (\d+)',line));assert len(c)==5;counts.append(c)
  assert e['anchors']==[11,11] and len(counts)==22
  for windows,aggregate in zip([counts[:11],counts[11:]],e['directions']):assert all(sum(c[k] for c in windows)==value for k,value in aggregate.items())
  supported=all(x['informative_pairs']>=1000 and x['valid_sites']/x['sites']>=.3 and x['agreeing_pairs']/x['informative_pairs']>=.9 for x in e['directions']);assert supported==e['supported'];assert row['status']==('supported' if supported else 'not_supported');decisions.append({'target':row['target'],'filter_radius':row['filter_radius'],'role':row['role'],'supported':supported,'directions':e['directions']})
 assert all(sha(p)==h for p,h in pins.items());out.write_text(json.dumps({'status':'verified_closed_corpus_scale_control_prefix','pins':pins,'closed':len(closed),'planned':len(r['plan']),'inventory':len(inventory),'excluded':sum(x['role']=='excluded_incompatible_domain' for x in inventory.values()),'producer_status_at_capture':r['status'],'results':decisions,'scope':'Immutable closed prefix with exact logs and all per-window count sums checked. Different-origin controls are not independent semantic negative annotations. No unfinished rows, complete corpus, precision or production claim.'},indent=2)+'\n');print(len(closed),'of',len(r['plan']),'verified')
if __name__=='__main__':main()
