"""Independent all-target/source/binary/result audit of the ENOSPC recovery replay."""
from pathlib import Path
import collections,hashlib,json,re,runpy,subprocess
R=Path(__file__).resolve().parents[1];D=R/'docs/research'
h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
report=D/'dedup-common-fixed-baseline-replay-v3-20261015.json';data=json.loads(report.read_text());assert data['status']=='terminal_pending_audit'
assert data['producer_sha256']==h(R/'scripts/replay-dedup-common-fixed-baseline-all-targets-v3-20261015.py')
assert data['original_report_sha256']==h(data['original_report']) and data['failed_audit_sha256']==h(data['failed_audit'])
original=json.loads(Path(data['original_report']).read_text());failure=json.loads(Path(data['failed_audit']).read_text());assert original['status']=='terminal_failure' and failure['failed']==9 and not failure['gate_passed']
assert data['manifest_sha256']==h(data['manifest'])==original['manifest_sha256']
manifest=json.loads(Path(data['manifest']).read_text());snapshot=next(v for v in manifest['variants'] if v['variant']=='baseline');assert data['source']==snapshot['source']
for row in snapshot['source_files']:assert h(Path(data['source'])/row['path'])==row['sha256']
expected={row['copy']:row for row in original['retained_binaries']};assert len(expected)==len(data['targets'])==162
assert set(expected)=={row['binary'] for row in data['targets']}
assert [row['index'] for row in data['targets']]==list(range(162))
account=runpy.run_path(str(R/'scripts/audit-dedup-complete-collection-terminal-20261015.py'))['account']
targets=[]
for row in data['targets']:
 old=expected[row['binary']];assert row['original']==old['original'] and row['binary_sha256']==old['sha256']==h(row['binary'])
 assert row['inventory']==old['inventory'] and row['inventory_sha256']==old['inventory_sha256']==h(row['inventory'])
 assert row['command']==[row['binary']] and row['cwd']==str(Path(data['source'])/'crates'/row['package'])
 names=set(re.findall(r'^(.+): test$',Path(row['inventory']).read_text(),re.M))
 fresh=subprocess.run([row['binary'],'--list'],capture_output=True,text=True);assert fresh.returncode==0 and names==set(re.findall(r'^(.+): test$',fresh.stdout,re.M))
 assert h(row['log'])==row['log_sha256'];result=account(Path(row['log']).read_text(),names)
 assert bool(row['returncode'])==bool(result['failed'])
 targets.append(dict(binary=row['binary'],binary_sha256=row['binary_sha256'],log=row['log'],log_sha256=row['log_sha256'],**result))
failed=sum(row['failed'] for row in targets);assert bool(data['returncode'])==bool(failed)
output=D/'dedup-common-fixed-baseline-replay-audit-v3-20261015.json';assert not output.exists()
output.write_text(json.dumps(dict(status='verified_failed_complete_gate' if failed else 'verified_passing_complete_gate',gate_passed=not failed,manifest_sha256=data['manifest_sha256'],producer_report_sha256=h(report),auditor_sha256=h(__file__),original_failed_audit_sha256=data['failed_audit_sha256'],sources_verified=len(snapshot['source_files']),targets=targets,passed=sum(r['passed'] for r in targets),failed=failed,ignored=sum(r['ignored'] for r in targets),scope=data['scope']),indent=2)+'\n')
print('Replay audited',len(targets),'targets',sum(r['passed'] for r in targets),'passed',failed,'failed')
