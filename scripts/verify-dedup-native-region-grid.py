#!/usr/bin/env python3
"""Audit single-lifecycle regional native decisions against pinned prior grid."""
import argparse
import hashlib
import json
import subprocess
import tempfile
from pathlib import Path
p=argparse.ArgumentParser()
for key in ('manifest','probe','report','reference','whole_reference','output'):p.add_argument(key,type=Path)
p.add_argument('--checkpoint',action='store_true');a=p.parse_args()
paths=(a.manifest,a.report,a.reference,a.whole_reference);raw={v:v.read_bytes() for v in paths}
m,r,b,w=[json.loads(raw[v]) for v in paths]
digest=lambda path:hashlib.sha256(path.read_bytes()).hexdigest()
assert len(m['positive_pairs'])==229 and len({v['query_id'] for v in m['positive_pairs']})==229
assert r['mode']==b['mode']=='pyramid_region_grid'
assert r['probe_sha256']==digest(a.probe)
assert b['probe_sha256']==digest(a.probe.parent/'photo-probe-pyramid-region-grid')
assert r['manifest_sha256']==b['manifest_sha256']==hashlib.sha256(raw[a.manifest]).hexdigest()
assert b['completed_pairs']==len(b['results'])==229 and b['required_pairs']==229
assert 0<r['completed_pairs']==len(r['results'])<=229 and r['required_pairs']==229
if not a.checkpoint:assert r['completed_pairs']==229 and r['status']=='complete_measurement_not_full_library_qualification'
for row,old,label in zip(r['results'],b['results'],m['positive_pairs']):
 assert row['query_id']==old['query_id']==label['query_id'] and row['label']==old['label']==label['label']
 assert row['status']==old['status']=='ok' and row['returncode']==old['returncode']==0
 for key in ('candidate','geometry','regions','region_support_count'):
  assert row['evidence'][key]==old['evidence'][key],(label['query_id'],key)
 assert row['evidence']['managed_used_after_drop']==0
 assert type(row['evidence']['managed_peak'])is int and 0<=row['evidence']['managed_peak']<=64*1024*1024
# The independent existing verifier reconstructs domains from PNG dimensions,
# validates pixel admission numerically, hashes inputs and checks whole geometry.
with tempfile.TemporaryDirectory() as directory:
 d=Path(directory);snapshot=d/'report.json';snapshot.write_bytes(raw[a.report])
 command=['python3',str(Path(__file__).with_name('verify-dedup-regional-positives.py')),str(a.manifest),str(a.probe),str(snapshot),str(a.whole_reference)]
 if a.checkpoint:command.append('--checkpoint')
 completed=subprocess.run(command,capture_output=True,text=True)
 assert completed.returncode==0,completed.stderr
 integrity=json.loads(completed.stdout)
for path,data in raw.items():assert path.read_bytes()==data,'Report changed: retry audit.'
out={'status':'verified','completed_pairs':r['completed_pairs'],'native_fields_equal':True,'integrity':integrity,
     'report_sha256':hashlib.sha256(raw[a.report]).hexdigest(),'reference_sha256':hashlib.sha256(raw[a.reference]).hexdigest(),
     'scope':'Single-lifecycle automatic-grid file API versus prior pinned multi-call diagnostics. Native decisions/domain coordinates/pixel counts equal. Not integrated collection corpus, region masks, semantic/burst or full library qualification.'}
a.output.write_text(json.dumps(out,indent=2)+'\n');print(json.dumps({'completed_pairs':r['completed_pairs'],'native_fields_equal':True,'integrity':integrity},indent=2))
