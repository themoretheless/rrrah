#!/usr/bin/env python3
import json,re,hashlib,subprocess
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp');sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();P=D/'dedup-bank-partition-20261013.json';data=json.loads(P.read_text());assert data['status']=='passed_pending_audit' and data['returncode']==0 and data['source_unchanged'];S=Path(data['source']);assert all(sha(S/p)==h for p,h in data['source_sha256'].items());L=Path(data['log']);assert sha(L)==data['log_sha256']
dest=T/'rrrah-bank-partition-qualified-20261013';assert not dest.exists();dest.mkdir();rows=[]
for block in re.split(r'(?=     Running )',L.read_text()):
 run=re.search(r'Running (unittests src/lib\.rs|tests/(\w+)\.rs) \(([^)]+)\)',block)
 if not run:continue
 _,name,binary=run.groups();name=name or 'raster_library';m=re.search(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;',block);assert m and m[2]=='0';out=dest/name;subprocess.run(['cp','-cp',binary,str(out)],check=True);assert sha(out)==sha(binary);rows.append(dict(target=name,passed=int(m[1]),ignored=int(m[3]),executable=str(out),executable_sha256=sha(out)))
assert len(rows)==5 and {r['target'] for r in rows}==set(data['targets'])|{'raster_library'}
result=dict(status='verified_provenance_preserving_alias_component_partition',source=str(S),source_files=len(data['source_sha256']),source_sha256=data['source_sha256'],producer_report_sha256=sha(P),log_sha256=sha(L),targets=rows,passed=sum(r['passed'] for r in rows),ignored=sum(r['ignored'] for r in rows),scope=data['scope'],controls=['Transitive source/target alias chains preserve all alternative correspondences and bank indices','A single component remains unsplit; empty-bank IDs retained','300 seeded graphs match independent flood-fill oracle','Managed output clone lifetime, exact payload and one-byte-short rollback','All declared work limits prepaid; malformed suffixes refuse atomically','Every one-shot cancellation checkpoint releases scratch and discards partial assignments with identical retry'])
out=D/'dedup-bank-partition-audit-20261013.json';assert not out.exists();out.write_text(json.dumps(result,indent=2)+'\n');print(result['status'],result['passed'],result['ignored'])
