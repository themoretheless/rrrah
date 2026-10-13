#!/usr/bin/env python3
import hashlib,json
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 out=D/'dedup-opposite-native-prefix0200-audit-20261011.json';assert not out.exists();rp=D/'dedup-opposite-native-prefix0200-20261011.json';r=json.loads(rp.read_text());assert r['status']=='complete_bounded_opposite_native_controls' and len(r['results'])==r['required']==2;assert all(sha(p)==h for p,h in r['pins'].items());m=json.loads(Path(r['source_manifest']).read_text());assert all(sha(Path(m['snapshot'])/f)==h for f,h in m['file_hashes'].items());previous=D/'dedup-opposite-native-20261011.json';old=json.loads(previous.read_text());oldbin=next(p for p in old['pins'] if p.endswith('/opposite_native_files'));newbin=next(p for p in r['pins'] if p.endswith('/opposite_native_files'));assert old['pins'][oldbin]==sha(oldbin)==sha(newbin)
 ap=D/'dedup-loftr-publisher-positives-ordering-prefix-0200-audit-20261011.json';a=json.loads(ap.read_text());b=json.loads(Path(a['captured_report']).read_text());pins={str(rp):sha(rp),str(previous):sha(previous),str(ap):sha(ap),str(Path(__file__)):sha(Path(__file__))};decisions=[]
 for index,plan,result in zip([57,153],r['plan'],r['results']):
  assert plan==b['plan'][index] and b['results'][index]['status']=='publisher_positive_inconclusive';assert sha(result['log'])==result['log_sha256'];assert Path(result['log']).read_text()==result['stdout']+result['stderr'];pins[result['log']]=result['log_sha256'];status=result['status'];assert status in ['supported','not_supported','inconclusive'];value=None
  if result['returncode']==0:
   value=json.loads(result['stdout']);assert value['managed_used']==0 and value['source_tolerance']==plan['source_tolerance'] and value['target_tolerance']==2;assert value['source_paths']==[plan['left'],plan['right']]
   if status=='supported':
    p=value['pixels'];assert min(p['anchors'])>=10 and p['supported'];assert value['transform'] and value['points']
    for d in p['directions']:assert d['informative_pairs']>=1000 and d['valid_sites']/d['sites']>=.3 and d['agreeing_pairs']/d['informative_pairs']>=.9
  else:assert status=='inconclusive' and not result['stdout'] and 'Error: Rank(InsufficientGeometry)' in result['stderr']
  decisions.append({'group_id':plan['group_id'],'direction':plan['direction'],'status':status,'elapsed_seconds':result['elapsed_seconds'],'anchors':value['pixels']['anchors'] if value and value['pixels'] else None,'attempted_recipes':value['attempted_recipes'] if value else None})
 out.write_text(json.dumps({'status':'verified_bounded_additional_opposite_native_controls','pins':pins,'results':decisions,'scope':'Two further original reverse baseline unknowns, previously qualified explicit six-opposite API at unchanged caller thresholds. Not combined portfolio recovery runtime, full recall/precision/platform/RSS qualification. Unresolved outcomes stay separate from old baseline.'},indent=2)+'\n');print(json.dumps(decisions))
if __name__=='__main__':main()
