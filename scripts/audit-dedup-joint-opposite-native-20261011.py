#!/usr/bin/env python3
import argparse,hashlib,json
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 partial=argparse.ArgumentParser();partial.add_argument('--partial',action='store_true');partial=partial.parse_args().partial
 rp=D/'dedup-joint-opposite-native-20261011.json';raw=rp.read_bytes();r=json.loads(raw);closed=[]
 for result in r['results']:
  if result['status']=='running':break
  closed.append(result)
 if partial:
  assert closed and r['status'] in ['running','complete_bounded_joint_opposite_native_controls'];capture=Path('/Users/themoretheless/.codex/tmp')/f'dedup-joint-opposite-native-prefix-{len(closed):04d}-20261011.json';assert not capture.exists();capture.write_bytes(raw);rp=capture
 else:assert r['status']=='complete_bounded_joint_opposite_native_controls' and len(closed)==r['required']==3
 out=D/(f'dedup-joint-opposite-native-prefix-{len(closed):04d}-audit-20261011.json' if partial else 'dedup-joint-opposite-native-audit-20261011.json');assert not out.exists();r['results']=closed;r['plan']=r['plan'][:len(closed)];assert all(sha(p)==h for p,h in r['pins'].items());m=json.loads(Path(r['source_manifest']).read_text());assert all(sha(Path(m['snapshot'])/f)==h for f,h in m['file_hashes'].items());generic=D/'dedup-joint-opposite-files-audit-20261011.json';g=json.loads(generic.read_text());assert g['status']=='complete_bounded_frozen_joint_opposite_files_78_tests' and g['tests']==78;gm=json.loads(Path(g['source_manifest']).read_text());assert all(m['file_hashes'][f]==h for f,h in gm['file_hashes'].items());assert all(sha(p)==h for p,h in g['pins'].items())
 ap=D/'dedup-loftr-publisher-positives-ordering-prefix-0200-audit-20261011.json';a=json.loads(ap.read_text());b=json.loads(Path(a['captured_report']).read_text());pins={str(rp):sha(rp),str(generic):sha(generic),str(ap):sha(ap),str(Path(__file__)):sha(Path(__file__))};decisions=[]
 for index,plan,result in zip([57,3,153][:len(closed)],r['plan'],r['results']):
  assert plan==b['plan'][index] and b['results'][index]['status']=='publisher_positive_inconclusive';assert sha(result['log'])==result['log_sha256'];assert Path(result['log']).read_text()==result['stdout']+result['stderr'];pins[result['log']]=result['log_sha256'];status=result['status'];assert status in ['supported','not_supported','inconclusive'];value=None
  if result['returncode']==0:
   value=json.loads(result['stdout']);assert value['managed_used']==0 and value['source_tolerance']==plan['source_tolerance'] and value['target_tolerance']==2;assert value['source_paths']==[plan['left'],plan['right']]
   if status=='supported':
    p=value['pixels'];assert min(p['anchors'])>=10 and p['supported'];assert value['transform'] and value['points']
    for d in p['directions']:assert d['informative_pairs']>=1000 and d['valid_sites']/d['sites']>=.3 and d['agreeing_pairs']/d['informative_pairs']>=.9
  else:assert status=='inconclusive' and not result['stdout'] and 'Error: Rank(InsufficientGeometry)' in result['stderr']
  decisions.append({'group_id':plan['group_id'],'direction':plan['direction'],'status':status,'elapsed_seconds':result['elapsed_seconds'],'anchors':value['pixels']['anchors'] if value and value['pixels'] else None,'attempted_recipes':value['attempted_recipes'] if value else None})
 out.write_text(json.dumps({'status':'verified_bounded_joint_opposite_native_controls','pins':pins,'results':decisions,'scope':'Three original reverse baseline unknowns, new caller-coordinate joint fitting in explicit six-opposite file API at unchanged pixel thresholds. Same production source as frozen78-test gate. Not combined portfolio recovery runtime, full recall/precision/platform/RSS qualification. Unresolved outcomes stay separate from old baseline.'},indent=2)+'\n');print(json.dumps(decisions))
if __name__=='__main__':main()
