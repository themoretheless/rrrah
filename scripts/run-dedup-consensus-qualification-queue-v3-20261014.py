#!/usr/bin/env python3
from pathlib import Path
import json,hashlib,os,subprocess,sys
R=Path(__file__).resolve().parents[1];D=R/'docs/research';sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();A=D/'dedup-consensus-duration-v4-focused-audit-20261014.json';a=json.loads(A.read_text());assert a['passed']==212 and a['failed']==0 and a['targets']==5;assert all(sha(row['executable'])==row['executable_sha256']for row in a['rows']);O=D/'dedup-consensus-qualification-queue-v3-20261014.json';assert not O.exists();d=dict(status='executing',producer_pid=os.getpid(),previous_queue='docs/research/dedup-consensus-qualification-queue-v2-20261014.json',duration_v4_audit_sha256=sha(A),completed=[],scope='Resume after corrected outputpath-only auditor collision. All actualv4tests alreadypassed and5binaries preserved in uniquev4directory. Run only remainingv5genericdecode stage; preserve priorfailure and oldv3binaries.')
def save():
 t=O.with_suffix('.tmp');t.write_text(json.dumps(d,indent=2)+'\n');t.replace(O)
save()
try:
 for stage in ['run-dedup-consensus-native-collection-v5-focused-20261014.py','audit-dedup-consensus-native-collection-v5-focused-20261014.py']:
  d.update(stage=stage);save();subprocess.run([sys.executable,str(R/'scripts'/stage)],cwd=R,check=True);d['completed'].append(stage);save()
 d.update(status='completed_selected_frozen_decode_stages');save()
except BaseException as e:d.update(status='stopped_on_failure',error=repr(e));save();raise
