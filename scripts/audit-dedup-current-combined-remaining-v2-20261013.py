#!/usr/bin/env python3
"""Audit every terminal target, preserving successes without hiding harness failures."""
import hashlib
import json
import re
import subprocess
from pathlib import Path
R=Path(__file__).resolve().parents[1]
D=R/'docs/research'
T=Path('/Users/themoretheless/.codex/tmp')
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
report=D/'dedup-current-combined-remaining-v2-20261013.json'
data=json.loads(report.read_text())
assert data['status']=='terminal_failure' and data['returncode']==101 and data['source_unchanged']
S=Path(data['source'])
assert all(sha(S/p)==h for p,h in data['source_sha256'].items())
log=Path(data['log']);assert sha(log)==data['log_sha256']
dest=T/'rrrah-current-combined-remaining-v2-qualified-20261013'
assert not dest.exists();dest.mkdir()
rows=[]
for block in re.split(r'(?=     Running tests/)',log.read_text()):
    run=re.search(r'Running tests/(\w+)\.rs \(([^)]+)\)',block)
    if not run:continue
    name,binary=run.groups()
    summaries=re.findall(r'test result: (ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored;',block)
    assert len(summaries)==1
    state,passed,failed,ignored=summaries[0]
    panics=re.findall(r"thread '.+?panicked at.*?(?=\n\n|\nfailures:)",block,re.S)
    if state=='FAILED':assert panics and all('PermissionDenied' in p for p in panics)
    else:assert failed=='0'
    output=dest/name
    subprocess.run(['cp','-cp',binary,str(output)],check=True)
    assert sha(output)==sha(binary)
    rows.append(dict(target=name,status=state,passed=int(passed),failed=int(failed),ignored=int(ignored),
                     panic_diagnostics=panics,executable=str(output),executable_sha256=sha(output)))
assert len(rows)==len(data['targets'])==92 and {r['target'] for r in rows}==set(data['targets'])
assert sum(r['status']=='FAILED' for r in rows)==10
result=dict(status='verified_terminal92_targets_with10_permission_failures',source=str(S),
    source_files=len(data['source_sha256']),source_sha256=data['source_sha256'],
    producer_report=str(report),producer_report_sha256=sha(report),log=str(log),log_sha256=sha(log),
    targets=rows,successful_targets=sum(r['status']=='ok' for r in rows),failed_targets=10,
    passed_in_successful_targets=sum(r['passed'] for r in rows if r['status']=='ok'),
    passed_in_failed_targets=sum(r['passed'] for r in rows if r['status']=='FAILED'),
    failed_tests=sum(r['failed'] for r in rows),ignored=sum(r['ignored'] for r in rows),
    scope='All92 scheduled targets executed on frozen3771-file source with3 lifecycle fixture repairs.10 targets failed exclusively observed PermissionDenied panics; no full-current green gate. Successful target tests saved separately; partial passed tests in failed targets are not complete-target qualification. Source excludes later publication/exact/cancel/finite-statistics/RAW/PFM changes.')
out=D/'dedup-current-combined-remaining-v2-audit-20261013.json'
assert not out.exists();out.write_text(json.dumps(result,indent=2)+'\n')
print({k:result[k] for k in ['successful_targets','failed_targets','passed_in_successful_targets','passed_in_failed_targets','failed_tests','ignored']})
