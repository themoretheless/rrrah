#!/usr/bin/env python3
from pathlib import Path
import json,hashlib,shutil
R=Path(__file__).resolve().parents[1];D=R/'docs/research';sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();M=D/'dedup-committed-build-source-415443172-v2-20261015.json';m=json.loads(M.read_text());B=Path(m['source']);F=Path('/private/tmp/rrrah-container-cancel-erasure-v1-20261015');assert not F.exists()
for row in m['source_files']:assert sha(B/row['path'])==row['sha256']
source='crates/rrrah-dedup/src/container.rs';assert sha(R/source)==sha(B/source);shutil.copytree(B,F);p=F/source;s=p.read_text();start=s.index('fn prepare_presentation(');end=s.index(') -> Result<PreparedPresentation, ContainerError> {',start);signature=s[start:end];assert signature.count('cancel: impl Fn() -> bool')==1;s=s[:start]+signature.replace('cancel: impl Fn() -> bool','cancel: &dyn Fn() -> bool')+s[end:]
lines=s.splitlines(keepends=True);changed_calls=0
for i,line in enumerate(lines):
 if 'prepare_presentation('in line and 'budget, cancelled)'in line:
  lines[i]=line.replace('budget, cancelled)','budget, &cancelled)');changed_calls+=1
assert changed_calls==4;p.write_text(''.join(lines));changed=[]
for row in m['source_files']:
 h=sha(F/row['path'])
 if h!=row['sha256']:changed.append(dict(path=row['path'],baseline_sha256=row['sha256'],candidate_sha256=h))
assert len(changed)==1 and changed[0]['path']==source;O=D/'dedup-container-cancel-erasure-v1-prepared-20261015.json';assert not O.exists();O.write_text(json.dumps(dict(status='prepared_not_compiled_not_adopted',producer_sha256=sha(__file__),source=str(F),baseline_manifest=str(M),baseline_manifest_sha256=sha(M),changed=changed,unchanged_files=len(m['source_files'])-1,scope='Only private prepare_presentation callbackparameter erased toborrowed dynFn; four existing byvalueclosure callsites nowborrow, alreadyborrowedcallsite unchanged. Functionbody and source/decode/budget/callback/publication guards unchanged. Fullsame baseline package/target gates and runtime/performance/compiler measurements required; longcurrent baseline container codegen motivates hypothesis only, not rootcause or speedup proof.'),indent=2)+'\n');print('Container callbackerasure candidate:1file,1private signature,4borrowcalls')
