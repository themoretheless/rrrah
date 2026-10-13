#!/usr/bin/env python3
"""Independently freeze and verify available category-negative outcomes."""
import csv
import hashlib
import json
from collections import Counter
from pathlib import Path
R=Path(__file__).resolve().parents[1]
D=R/'docs/research'
T=Path('/Users/themoretheless/.codex/tmp')
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
report=D/'dedup-semantic-category-negatives-20261013.json'
raw=report.read_bytes();data=json.loads(raw);n=data['completed']
assert 0<n<=8 and len(data['plan'])==data['required']==8 and len(data['results'])==n
assert all(sha(p)==h for p,h in data['pins'].items())
preplan=D/'dedup-semantic-category-negatives-plan-20261013.json'
pre=json.loads(preplan.read_text());assert data['plan']==pre['plan']
expected=[('Q41923','R026396'),('Q42962','R040062'),('Q17383','R043984'),('Q20149','R026440')]
assert [(p['left_id'],p['right_id']) for p in data['plan']]==[direction for a,b in expected for direction in [(a,b),(b,a)]]
ground_truth=next(Path(p) for p in pre['pins'] if p.endswith('/dev_ground_truth.csv'))
labels=dict(csv.reader(ground_truth.read_text().splitlines()));assert len(labels)==10000
prepared=json.loads((D/'dedup-disc21-heldout-negatives-prepared-20261012.json').read_text())
images={p['id']:p for p in prepared['images']}
previous={p['query'] for p in prepared['plan']}
assert all(q not in labels and q not in previous for q,_ in expected)
manifest=json.loads(Path(data['source_manifest']).read_text())
assert all(sha(Path(data['source'])/p)==h for p,h in manifest['source_hashes'].items())
counts=dict(false_acceptance=0,inconclusive=0,rejected=0,execution_failed=0)
pins=dict(data['pins']);rss=[];model_attempted=0
for i,row in enumerate(data['results']):
    assert row['index']==i and sha(row['report'])==row['sha256']
    case=json.loads(Path(row['report']).read_text());item=data['plan'][i]
    assert case['index']==i
    assert item['left']==images[item['left_id']]['path'] and item['right']==images[item['right_id']]['path']
    command=['/usr/bin/time','-l','/usr/bin/env','DYLD_LIBRARY_PATH='+str(T/'rrrah-loftr-native-runtime-20261010/lib/python3.12/site-packages/torch/lib'),data['executable'],item['left'],item['right'],str(T/'rrrah-dedup-copydays/learned-matching-models/loftr-outdoor-native-20261010.safetensors'),'640','2.0','1000000']
    assert case['command']==command
    kind='execution_failed'
    if case['returncode']==0:
        v=json.loads(case['stdout']);assert v['managed_used']==0 and v['files']==[2,9] and v['opposite_trials']==1000000 and len(v['pairs'])==1
        pair=v['pairs'][0];assert (pair['left'],pair['right'])==(2,9) and type(pair['model_attempted']) is bool
        support=pair['support'];assert support is None or type(support) is bool
        assert support is not None or (pair['model_attempted'] and pair['detail']['terminal']=='inconclusive')
        kind='false_acceptance' if support is True else ('rejected' if support is False else 'inconclusive')
        model_attempted+=int(pair['model_attempted']);rss.append(case['rss_bytes'])
    assert kind==case['status']==row['status'];counts[kind]+=1;pins[row['report']]=row['sha256']
assert counts==data['counts']
complete=n==8 and data['status']=='complete_fixed8_category_negative_directions'
if n==8:assert complete
capture=T/f'rrrah-semantic-category-negatives-capture-{n:02}-20261013.json'
out=D/(('dedup-semantic-category-negatives-complete-audit-20261013.json') if complete else f'dedup-semantic-category-negatives-prefix-{n:02}-audit-20261013.json')
assert not out.exists() and not capture.exists();capture.write_bytes(raw)
pins.update({str(p):sha(p) for p in [capture,Path(__file__)]})
result=dict(status='verified_complete8_category_negative_directions' if complete else 'verified_category_negative_prefix',completed=n,required=8,counts=counts,model_attempted_directions=model_attempted,observed_process_peak_rss_bytes_max=max(rss) if rss else None,pins=pins,scope=data['scope'])
out.write_text(json.dumps(result,indent=2)+'\n');print(result['status'],n,counts)
