#!/usr/bin/env python3
import hashlib,json,re
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 p=D/'dedup-disc21-pilot-native-20261011.json';r=json.loads(p.read_text());assert r['status']=='complete_bounded_disc21_pilot_execution' and len(r['results'])==r['required']==2
 pins=dict(r['pins']);assert all(sha(k)==v for k,v in pins.items());prep=D/'dedup-disc21-pilot-prepared-20261011.json';data=json.loads(prep.read_text());pa=D/'dedup-disc21-pilot-prepared-audit-20261011.json';a=json.loads(pa.read_text());assert a['status']=='verified_disc21_published_labels_and_range_extraction' and all(sha(k)==v for k,v in a['pins'].items());assert r['plan']==data['plan'];images={x['id']:x for x in data['images']}
 mp=D/'dedup-native-bank-outcome-native-snapshot-20261011.json';m=json.loads(mp.read_text());assert all(sha(Path(m['snapshot'])/k)==v for k,v in m['file_hashes'].items());rows=[]
 for i,row in enumerate(r['results']):
  expected=data['plan'][i];assert row['index']==i and all(row[k]==expected[k] for k in ['query','reference','published_label']);assert row['command'][5:7]==[images[expected['query']]['path'],images[expected['reference']]['path']] and row['command'][-3:]==['640','2.0','1000000'];assert sha(row['log'])==row['log_sha256'] and Path(row['log']).read_text()==row['stdout']+row['stderr'];pins[row['log']]=row['log_sha256']
  if row['returncode']:
   assert row['status']=='execution_failed';rows.append({'query':row['query'],'published_label':row['published_label'],'status':'execution_failed'});continue
  v=json.loads(row['stdout']);assert v['files']==[2,9] and len(v['pairs'])==1 and v['managed_used']==0;e=v['pairs'][0];assert e==row['evidence'] and (e['left'],e['right'])==(2,9);assert e['support'] is None or type(e['support']) is bool;status='supported' if e['support'] is True else ('rejected' if e['support'] is False else 'inconclusive');assert status==row['status'];rss=int(re.search(r'(\d+)\s+maximum resident set size',row['stderr'])[1]);assert rss==row['max_rss_bytes'];rows.append({'query':row['query'],'reference':row['reference'],'published_label':row['published_label'],'status':status,'model_attempted':e['model_attempted'],'managed_used':0,'max_rss_bytes':rss,'elapsed_seconds':row['elapsed_seconds']})
 gaps=[x for x in rows if x['status']=='execution_failed' or (x['published_label']=='copy' and x['status']!='supported') or (x['published_label']=='no_reference_match' and x['status']=='supported')];out=D/'dedup-disc21-pilot-native-audit-20261011.json';assert not out.exists();pins.update({str(x):sha(x) for x in [p,pa,Path(__file__)]});out.write_text(json.dumps({'status':'verified_bounded_disc21_pilot_observations','pins':pins,'rows':rows,'observed_acceptance_gaps':gaps,'scope':'Published-label pilot only, one collage copy and one unrelated distractor against the same reference. Unknown is not definitive negative. Positive miss/false support/execution errors retained as gaps; no semantic/burst/general precision/full DISC21/all-case completion claim.'},indent=2)+'\n');print(rows,'gaps',gaps)
if __name__=='__main__':main()
