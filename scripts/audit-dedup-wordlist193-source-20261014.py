#!/usr/bin/env python3
from pathlib import Path
import json,hashlib
R=Path(__file__).resolve().parents[1];D=R/'docs/research';P=D/'dedup-500/wordlist193-complete-source-review-20261014.json';d=json.loads(P.read_text());sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();assert sha(R/'scripts/check-dedup-wordlist193-20261014.py')==d['producer_sha256'];assert len(d['fully_read_files'])==3 and len(d['controls'])==7 and d['syntax_returncode']==0
for v in d['fully_read_files']:
 b=(R/v['local']).read_bytes();assert sha(R/v['local'])==v['sha256']and hashlib.sha1(b'blob '+str(len(b)).encode()+b'\0'+b).hexdigest()==v['git_blob_sha1']and len(b)==v['bytes']
for v in d['controls']:
 data=bytes.fromhex(v['input_hex']);expected=b''
 if data:
  lines=data.split(b'\n')
  if data.endswith(b'\n'):lines.pop()
  expected=b''.join(line.removesuffix(b'\r')+b'\n'for line in lines)
 assert expected.hex()==v['output_hex']==v['expected_hex']and v['returncode']==0
O=D/'dedup-wordlist193-controls-audit-20261014.json';assert not O.exists();O.write_text(json.dumps(dict(status='verified3blob_source_pins7text_normalization_controls',review=str(P.relative_to(R)),review_sha256=sha(P),auditor_sha256=sha(__file__),controls=7,scope='Independent source/producer pins and bytes-level normalization oracle; bash syntax result from producer. No rling/whole-script/network/runtime/corpus or library adoption claim.'),indent=2)+'\n');print('193 source/normalization independently audited')
