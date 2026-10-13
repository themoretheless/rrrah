#!/usr/bin/env python3
"""Freeze and independently verify a completed prefix; never claim all458."""
import hashlib,json,re
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 report=D/'dedup-model-bank-all-positives-20261011.json';raw=report.read_bytes();r=json.loads(raw);n=r['completed'];assert 0<n<=458 and len(r['plan'])==458 and r['required']==458
 out=D/f'dedup-model-bank-all-positives-prefix-{n:04d}-audit-20261011.json';capture=T/f'dedup-model-bank-all-positives-prefix-{n:04d}-20261011.json';assert not out.exists() and not capture.exists();capture.write_bytes(raw)
 pins=dict(r['pins']);assert all(sha(k)==v for k,v in pins.items())
 bp=T/'rrrah-loftr-publisher-positives-qualified-20261011/audits/ordering-prefix-0458.json';b=json.loads(bp.read_text());assert sorted(x['baseline_index'] for x in r['plan'])==list(range(458));assert all(row==dict(b['plan'][row['baseline_index']],baseline_index=row['baseline_index']) for row in r['plan'])
 mp=D/'dedup-model-bank-collection-native-snapshot-20261011.json';m=json.loads(mp.read_text());assert all(sha(Path(m['snapshot'])/k)==v for k,v in m['file_hashes'].items());counts={'supported':0,'inconclusive':0,'rejected':0,'execution_failed':0};rows=[]
 for i,v in enumerate(r['results'][:n]):
  assert v['index']==i and v['baseline_index']==r['plan'][i]['baseline_index'];assert sha(v['log'])==v['log_sha256'];assert Path(v['log']).read_text()==v['stdout']+v['stderr'];pins[v['log']]=v['log_sha256']
  if v['returncode']!=0:assert v['status']=='execution_failed';kind='execution_failed'
  else:
   value=json.loads(v['stdout']);assert value['files']==[2,9] and value['managed_used']==0 and len(value['pairs'])==1;pair=value['pairs'][0];assert (pair['left'],pair['right'])==(2,9) and pair==v['evidence'];support=pair['support'];kind='supported' if support is True else ('rejected' if support is False else 'inconclusive');assert v['status']=='publisher_positive_'+kind;assert int(re.search(r'(\d+)\s+maximum resident set size',v['stderr'])[1])==v['rss_bytes']
  counts[kind]+=1;rows.append({'index':i,'baseline_index':v['baseline_index'],'status':kind})
 pins.update({str(p):sha(p) for p in [capture,Path(__file__)]});out.write_text(json.dumps({'status':'verified_completed_prefix_only','completed':n,'required':458,'counts':counts,'rows':rows,'captured_report':str(capture),'pins':pins,'scope':'Immutable completed prefix of full frozen bank positive execution. Full458 run may still be live; no aggregate recall/semantic negatives/formats/platforms/all500 completion claim.'},indent=2)+'\n');print(n,counts)
if __name__=='__main__':main()
