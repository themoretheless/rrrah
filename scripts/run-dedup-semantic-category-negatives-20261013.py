#!/usr/bin/env python3
"""Precommit four manually category-near DISC21 negatives, then run both directions."""
import csv
import hashlib
import json
import os
import re
import subprocess
import time
from pathlib import Path
R=Path(__file__).resolve().parents[1]
D=R/'docs/research'
T=Path('/Users/themoretheless/.codex/tmp')
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
prepared=D/'dedup-disc21-heldout-negatives-prepared-20261012.json'
parent=json.loads(prepared.read_text())
preplan=Path(parent['preplan']);initial=json.loads(preplan.read_text())
ground_truth=next(Path(p) for p in initial['pins'] if p.endswith('/dev_ground_truth.csv'))
assert sha(ground_truth)==initial['pins'][str(ground_truth)]
labels=dict(csv.reader(ground_truth.read_text().splitlines()))
assert len(labels)==10000
images={p['id']:p for p in parent['images']}
previous={p['query'] for p in parent['plan']}
chosen=[('Q41923','R026396','portrait category, different framing/person appearance'),
        ('Q42962','R040062','portrait category, different indoor/outdoor subjects'),
        ('Q17383','R043984','water landscape category, seal/coast versus river valley'),
        ('Q20149','R026440','urban architecture category, distinct facades and viewpoint')]
base_path=D/'dedup-current-native-bank-all-positives-20261012.json'
base=json.loads(base_path.read_text())
audit_path=D/'dedup-current-native-bank-complete-audit-20261013.json'
audit=json.loads(audit_path.read_text())
assert base['status']=='complete_current_fixed458_execution' and audit['completed']==458
assert all(sha(p)==h for p,h in audit['pins'].items())
binary=Path(base['executable'])
weights=T/'rrrah-dedup-copydays/learned-matching-models/loftr-outdoor-native-20261010.safetensors'
assert sha(binary)==base['pins'][str(binary)] and sha(weights)==base['pins'][str(weights)]
plan=[]
for q,r,reason in chosen:
    assert q not in labels and q not in previous and r in parent['references']
    for a,b in [(q,r),(r,q)]:
        plan.append(dict(left_id=a,right_id=b,left=images[a]['path'],right=images[b]['path'],
                         published_label='no_reference_match',manual_category_reason=reason))
pins={str(p):sha(p) for p in [prepared,preplan,ground_truth,audit_path,base_path,binary,weights,Path(__file__),
    T/'rrrah-disc21-semantic-references-20261013.png',T/'rrrah-disc21-semantic-queries-20261013.png']}
for item in images.values():
    if item['id'] in {v for pair in chosen for v in pair[:2]}:
        assert sha(item['path'])==item['sha256'];pins[item['path']]=item['sha256']
manifest=json.loads(Path(base['source_manifest']).read_text())
assert all(sha(Path(base['source'])/p)==h for p,h in manifest['source_hashes'].items())
P=D/'dedup-semantic-category-negatives-20261013.json'
precommit=D/'dedup-semantic-category-negatives-plan-20261013.json'
folder=T/'rrrah-semantic-category-negatives-20261013'
assert not P.exists() and not precommit.exists() and not folder.exists()
folder.mkdir()
scope='Four hand-selected broad-category-near pairs from existing64query/8ref DISC21 pool, unseen by prior16 matcher controls; eight directions. Published nonmatch by query absence from complete dev CSV. Not burst pairs, independent training holdout or representative semantic precision. Frozen2696-file caller,2.0 tolerance/640input/native4096/opposite1m/model25banks radii2,6 unchanged; no latest-root/fullformats/RSS-cap claim.'
precommit.write_text(json.dumps(dict(plan=plan,pins=pins,scope=scope),indent=2)+'\n')
pins[str(precommit)]=sha(precommit)
data=dict(status='running',producer_pid=os.getpid(),source=base['source'],source_manifest=base['source_manifest'],
          executable=str(binary),plan=plan,pins=pins,completed=0,required=8,results=[],
          counts=dict(false_acceptance=0,inconclusive=0,rejected=0,execution_failed=0),scope=scope)
def save():
    temp=P.with_suffix('.tmp');temp.write_text(json.dumps(data,indent=2)+'\n');temp.replace(P)
save()
try:
    for index,item in enumerate(plan):
        cmd=['/usr/bin/time','-l','/usr/bin/env','DYLD_LIBRARY_PATH='+str(T/'rrrah-loftr-native-runtime-20261010/lib/python3.12/site-packages/torch/lib'),str(binary),item['left'],item['right'],str(weights),'640','2.0','1000000']
        start=time.monotonic();child=subprocess.Popen(cmd,cwd='/tmp',stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
        data.update(active_index=index,child_pid=child.pid);save();stdout,stderr=child.communicate()
        case=dict(index=index,command=cmd,returncode=child.returncode,stdout=stdout,stderr=stderr,elapsed_seconds=time.monotonic()-start)
        kind='execution_failed'
        if child.returncode==0:
            value=json.loads(stdout);assert value['files']==[2,9] and value['managed_used']==0 and value['opposite_trials']==1000000 and len(value['pairs'])==1
            pair=value['pairs'][0];assert (pair['left'],pair['right'])==(2,9)
            support=pair['support'];assert support is None or type(support) is bool
            kind='false_acceptance' if support is True else ('rejected' if support is False else 'inconclusive')
            case['rss_bytes']=int(re.search(r'(\d+)\s+maximum resident set size',stderr)[1])
        case['status']=kind;out=folder/f'{index:02}.json';assert not out.exists();out.write_text(json.dumps(case,indent=2)+'\n')
        data['results'].append(dict(index=index,status=kind,report=str(out),sha256=sha(out)));data['counts'][kind]+=1;data['completed']=index+1;save();print(index,kind,flush=True)
    assert all(sha(p)==h for p,h in pins.items())
    assert all(sha(Path(base['source'])/p)==h for p,h in manifest['source_hashes'].items())
    data['status']='complete_fixed8_category_negative_directions';save()
except Exception as error:
    data.update(status='terminal_controller_failure',error=str(error));save();raise
