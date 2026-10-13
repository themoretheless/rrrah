#!/usr/bin/env python3
import json,hashlib,re,subprocess
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
P=D/'dedup-complete-decode-gate-20261013.json';d=json.loads(P.read_text());assert d['status']=='terminal_failure' and d['returncode']==101 and d['source_unchanged']
S=Path(d['source']);pins=d.get('source_sha256',d.get('source_hashes'));assert pins and all(sha(S/p)==h for p,h in pins.items())
L=Path(d['log']);s=L.read_text();assert sha(L)==d['log_sha256']
F=T/'rrrah-complete-decode-gate-qualified-v2-20261013';assert not F.exists();F.mkdir()
rows=[]
pattern=r'Running (.*?) \(([^\n]+)\)\n(.*?)(?=\n\s*Running |\Z)'
for m in re.finditer(pattern,s,re.S):
 target,exe,body=m.groups();summary=re.search(r'test result: (ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored;',body)
 assert summary, target
 src=Path(exe);dst=F/src.name
 if target=='tests/visual_publication.rs': src=T/'rrrah-visual-publication-fullgate-failed-20261013'
 assert src.is_file();subprocess.run(['cp','-cp',str(src),str(dst)],check=True)
 status,passed,failed,ignored=summary.groups();rows.append(dict(target=target,status=status,passed=int(passed),failed=int(failed),ignored=int(ignored),executable=str(dst),executable_sha256=sha(dst)))
assert len(rows)==148 and sum(r['failed'] for r in rows)==1
out=D/'dedup-complete-decode-gate-audit-v2-20261013.json';assert not out.exists();out.write_text(json.dumps(dict(status='verified_frozen_gate_one_fixture_permission_failure',producer_sha256=sha(P),log_sha256=sha(L),source_files=len(pins),executed_targets=len(rows),passed=sum(r['passed'] for r in rows),failed=1,ignored=sum(r['ignored'] for r in rows),rows=rows,scope='Executed library/integration targets from saved log; zero-test targets do not establish runtime coverage. Frozen source predates new geometry_validation/bank_partition APIs. Original gate remains failed; focused repair is separate evidence.'),indent=2)+'\n');print(out)
