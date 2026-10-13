#!/usr/bin/env python3
import json,hashlib,re
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research/dedup-500';P=D/'azux-hashes-runtime-controls-20261013.json';d=json.loads(P.read_text());M=D/'azux-hashes-full-source-20261013.json';m=json.loads(M.read_text());S=Path(m['source']);V=D/'azux-hashes-complete-source-review-20261013.json';v=json.loads(V.read_text());sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();assert d['status']=='complete_pending_independent_audit';assert all(sha(k)==h for k,h in d['pins'].items());t=json.loads((D/'evidence/AzuxDario--Perceptual-image-hashes/tree.json').read_text());assert not t['truncated'];tree={e['path']:e for e in t['tree'] if e['type']=='blob'};assert len(tree)==len(m['files'])==13;assert set(tree)==set(v['complete_text_files_read'])=={str(p.relative_to(S)) for p in S.rglob('*') if p.is_file()}
for row in m['files']:
 b=(S/row['path']).read_bytes();assert hashlib.sha1(b'blob '+str(len(b)).encode()+b'\0'+b).hexdigest()==tree[row['path']]['sha']==row['git_blob'];assert sha(S/row['path'])==row['sha256'];b.decode('utf-8')
assert len(d['rows'])==7;safe=[];counter=[]
for row in d['rows']:
 if row['kind']=='build':assert row['returncode']==0 and sha(row['executable'])==row['executable_sha256'];continue
 for key in ['stdout','stderr']:assert sha(row[key])==row[key+'_sha256']
 out=Path(row['stdout']).read_text();err=Path(row['stderr']).read_text()
 if row['mode']=='safe':
  assert row['returncode']==0 and not err and out.count('PASS ')==6 and 'MEAN_MISMATCHES 97\n' in out and 'TOP_MISMATCHES 56\n' in out;safe.append(row['label'])
 else:
  assert row['label']=='sanitized' and row['returncode']!=0
  if row['mode']=='phash':assert 'PerceptualHash.cpp:43:' in err and 'nan is outside the range' in err and 'UndefinedBehaviorSanitizer' in err
  elif row['mode']=='position_zero':assert 'Image.cpp:11:' in err and 'division by zero' in err and 'UndefinedBehaviorSanitizer' in err
  elif row['mode']=='difference_short':assert 'AddressSanitizer: heap-buffer-overflow' in err and 'DifferenceHash.cpp:15' in err and 'READ of size 1' in err
  else:raise AssertionError(row['mode'])
  counter.append(row['mode'])
assert safe==['normal','sanitized'] and set(counter)=={'phash','position_zero','difference_short'};O=D/'azux-hashes-complete-runtime-audit-20261013.json';assert not O.exists();O.write_text(json.dumps(dict(status='verified13file_source_six_controls_and_three_sanitizer_counterexamples',repository=m['repository'],commit=m['commit'],pins={str(p):sha(p) for p in [P,M,V,Path(__file__),D/'evidence/AzuxDario--Perceptual-image-hashes/tree.json']},complete_text_files=13,safe_distinct_controls=6,safe_build_modes=safe,seeded_mean_oracle_grids=100,mean_mismatches=97,top_stride_mismatches=56,sanitizer_counterexamples=counter,scope='Independent complete Git/source pins and actual compiler/executable/log/sanitizer result verification. Original implementation translation units unchanged; one private-method probe via access-only header macro. No upstream test/build system, decoded-pixel recipe, all-platform, performance, precision or correct full DCT claim. Source not adopted.'),indent=2)+'\n');print(O)
