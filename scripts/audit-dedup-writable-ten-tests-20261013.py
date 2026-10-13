#!/usr/bin/env python3
import hashlib,json,re,subprocess
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp');sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
report=D/'dedup-writable-ten-tests-20261013.json';data=json.loads(report.read_text());assert data['status']=='passed_pending_audit' and data['returncode']==0 and data['source_unchanged']
S=Path(data['source']);assert all(sha(S/p)==h for p,h in data['source_sha256'].items());log=Path(data['log']);assert sha(log)==data['log_sha256']
dest=T/'rrrah-writable-ten-tests-qualified-20261013';assert not dest.exists();dest.mkdir();rows=[]
for block in re.split(r'(?=     Running tests/)',log.read_text()):
 run=re.search(r'Running tests/(\w+)\.rs \(([^)]+)\)',block)
 if not run:continue
 name,binary=run.groups();m=re.search(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;',block);assert m and m[2]=='0'
 output=dest/name;subprocess.run(['cp','-cp',binary,str(output)],check=True);assert sha(output)==sha(binary)
 rows.append(dict(target=name,passed=int(m[1]),ignored=int(m[3]),executable=str(output),executable_sha256=sha(output)))
assert len(rows)==10 and {r['target'] for r in rows}==set(data['targets'])
out=D/'dedup-writable-ten-tests-audit-20261013.json';assert not out.exists();result=dict(status='verified_ten_repaired_fixture_targets',source=str(S),source_sha256=data['source_sha256'],producer_report_sha256=sha(report),log_sha256=sha(log),targets=rows,passed=sum(r['passed'] for r in rows),ignored=sum(r['ignored'] for r in rows),scope=data['scope']);out.write_text(json.dumps(result,indent=2)+'\n');print(result['status'],result['passed'],result['ignored'])
