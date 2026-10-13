#!/usr/bin/env python3
import json,hashlib,re
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
P=D/'dedup-copydays-nearest-wrong-native-20261013.json';data=json.loads(P.read_text());n=data['completed'];assert n>0 and n==len(data['results']);assert data['required']==len(data['plan'])==458
snapshot=D/f'dedup-copydays-nearest-wrong-native-prefix-{n:04}-20261013.json';assert not snapshot.exists();snapshot.write_text(json.dumps(data,indent=2)+'\n')
assert all(sha(p)==h for p,h in data['pins'].items());S=Path(data['source']);manifest=json.loads(Path(data['source_manifest']).read_text());assert all(sha(S/p)==h for p,h in manifest['source_hashes'].items());pairs={}
for c in data['plan']:
 assert Path(c['query']).stem[:4]==Path(c['origin']).stem[:4] and Path(c['query']).stem[:4]!=Path(c['wrong_reference']).stem[:4];pairs.setdefault((c['query'],c['wrong_reference']),set()).add(c['direction'])
assert len(pairs)==229 and all(v=={'query_to_wrong','wrong_to_query'} for v in pairs.values())
counts=dict(false_acceptance=0,inconclusive=0,rejected=0,execution_failed=0);rows=[]
for i,r in enumerate(data['results']):
 assert r['index']==i and sha(r['report'])==r['sha256'];case=json.loads(Path(r['report']).read_text());c=data['plan'][i];cmd=case['command']
 assert cmd[-6:]==[c['left'],c['right'],next(p for p in data['pins'] if p.endswith('loftr-outdoor-native-20261010.safetensors')),'640','2.0','1000000'];assert data['executable'] in cmd
 kind='execution_failed'
 if case['returncode']==0:
  v=json.loads(case['stdout']);assert v['managed_used']==0 and v['files']==[2,9] and len(v['pairs'])==1;support=v['pairs'][0]['support'];assert support is None or type(support) is bool;kind='false_acceptance' if support is True else ('rejected' if support is False else 'inconclusive');assert case['rss_bytes']==int(re.search(r'(\d+)\s+maximum resident set size',case['stderr'])[1])
 assert kind==case['status']==r['status'];counts[kind]+=1;rows.append(dict(index=i,status=kind,report_sha256=r['sha256'],elapsed_seconds=case['elapsed_seconds'],rss_bytes=case.get('rss_bytes')))
assert counts==data['counts'];out=D/f'dedup-copydays-nearest-wrong-native-prefix-{n:04}-audit-20261013.json';assert not out.exists();out.write_text(json.dumps(dict(status='verified_fixed_nearest_wrong_native_prefix_not_complete_campaign' if n<458 else 'verified_complete_fixed458_nearest_wrong_directions',verifier_sha256=sha(__file__),completed=n,required=458,counts=counts,source_files=len(manifest['source_hashes']),snapshot=str(snapshot),snapshot_sha256=sha(snapshot),rows=rows,scope=data['scope']+' Inconclusive is not rejected/true-negative. Prefix does not establish complete campaign precision or latest-root behavior.'),indent=2)+'\n');print(out,counts)
