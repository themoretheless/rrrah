#!/usr/bin/env python3
"""Qualify terminal458 current campaign, preserving its frozen source scope."""
import hashlib
import json
from collections import Counter
from pathlib import Path
R=Path(__file__).resolve().parents[1]
D=R/'docs/research'
T=Path('/Users/themoretheless/.codex/tmp')
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
report=D/'dedup-current-native-bank-all-positives-20261012.json'
prefix=D/'dedup-current-native-bank-prefix-0458-audit-20261012.json'
data=json.loads(report.read_text())
assert data['status']=='complete_current_fixed458_execution', 'Producer is not terminal complete'
assert data['completed']==data['required']==len(data['plan'])==len(data['results'])==458
control=json.loads(prefix.read_text())
assert control['completed']==458 and control['counts']==data['counts']
assert all(sha(p)==h for p,h in control['pins'].items())
manifest=json.loads(Path(data['source_manifest']).read_text())
assert all(sha(Path(data['source'])/p)==h for p,h in manifest['source_hashes'].items())
priority=[3,7,27,57,67,153,255]
assert [p['baseline_index'] for p in data['plan']]==priority+[i for i in range(458) if i not in priority]
unordered=Counter(tuple(sorted((p['left'],p['right']))) for p in data['plan'])
directed={(p['left'],p['right']) for p in data['plan']}
assert len(unordered)==229 and set(unordered.values())=={2}
assert len(directed)==458 and all((b,a) in directed for a,b in directed)
model_attempted=0
rss=[]
for row,item in zip(data['results'],data['plan']):
    assert sha(row['report'])==row['sha256']
    case=json.loads(Path(row['report']).read_text())
    assert case['index']==row['index'] and case['baseline_index']==item['baseline_index']
    if case['returncode']==0:
        value=json.loads(case['stdout'])
        assert value['managed_used']==0 and value['opposite_trials']==1000000
        model_attempted+=int(value['pairs'][0]['model_attempted'])
        rss.append(case['rss_bytes'])
pins=dict(control['pins'])
pins.update({str(p):sha(p) for p in [report,prefix,Path(__file__)]})
result=dict(status='verified_complete_fixed458_campaign',completed=458,unordered_pairs=229,
    both_directions=True,counts=data['counts'],all_supported=data['counts']['supported']==458,
    model_attempted_directions=model_attempted,observed_process_peak_rss_bytes_max=max(rss) if rss else None,
    source=data['source'],source_files=len(manifest['source_hashes']),pins=pins,
    scope='Terminal frozen2696-file campaign, fixed229publisher pairs both directions, original input/policy/order and unchanged pins. Source precedes latest exact/visual/local publication and finite-statistics fixes. Not a full-current integration gate, semantic/burst negative precision, allformats/platforms/full500source or enforced RSS/deadline cap.')
out=D/'dedup-current-native-bank-complete-audit-20261013.json'
assert not out.exists()
out.write_text(json.dumps(result,indent=2)+'\n')
print(result['status'],result['counts'],result['observed_process_peak_rss_bytes_max'])
