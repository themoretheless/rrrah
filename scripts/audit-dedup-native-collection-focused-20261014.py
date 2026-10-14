#!/usr/bin/env python3
from pathlib import Path
import hashlib
import json
import re
import subprocess

R=Path(__file__).resolve().parents[1];D=R/'docs/research'
P=D/'dedup-native-collection-focused-20261014.json';d=json.loads(P.read_text());sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
assert d['status']=='passed_pending_audit' and d['build_returncode']==0 and d['source_unchanged']
Q=Path(d['plan']);assert sha(Q)==d['plan_sha256'];q=json.loads(Q.read_text());H=Path(q['harness']);assert all(sha(H/p)==h for p,h in q['harness_sha256'].items())
M=Path(q['source_manifest']);assert sha(M)==q['source_manifest_sha256'];m=json.loads(M.read_text());S=Path(m['source']);assert all(sha(S/p)==h for p,h in m['source_sha256'].items())
assert sha(R/'scripts/run-dedup-native-collection-focused-20261014.py')==d['producer_sha256']
assert all(sha(p)==h for p,h in d['build_pins'].items())
assert len(d['rows'])==2 and {row['target'] for row in d['rows']}=={'portfolio_collection','search_portfolio'}
F=Path('/private/tmp/rrrah-native-collection-focused-qualified-20261014');F.mkdir(exist_ok=False);rows=[]
for row in d['rows']:
    assert row['returncode']==0 and sha(row['executable'])==row['executable_sha256'] and sha(row['log'])==row['log_sha256']
    text=Path(row['log']).read_text();summary=re.search(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;',text);assert summary and summary[2]=='0'
    if row['target']=='portfolio_collection':assert 'test native_consensus_collection_empty_batch_never_loads_invalid_model ... ok' in text
    executable=F/row['target'];subprocess.run(['cp','-cp',row['executable'],str(executable)],check=True);assert sha(executable)==row['executable_sha256']
    rows.append(dict(target=row['target'],passed=int(summary[1]),failed=0,ignored=int(summary[3]),executable=str(executable),executable_sha256=sha(executable),log=row['log'],log_sha256=sha(row['log'])))
O=D/'dedup-native-collection-focused-audit-20261014.json';assert not O.exists();O.write_text(json.dumps(dict(status='verified_frozen_native_collection_adapter_two_integration_targets',rows=rows,passed=sum(row['passed'] for row in rows),failed=0,ignored=sum(row['ignored'] for row in rows),pins={str(p):sha(p) for p in [P,Q,M,Path(__file__)]},scope=d['scope']+' Native adapter compiled with actual LoFTR dependency; empty batch regression demonstrates invalid model is not loaded. Generic providers in remaining tests are synthetic; no learned multi-pair inference, directional-cap native inference, full corpus/default/UI/workspace qualification.'),indent=2)+'\n');print(O)
