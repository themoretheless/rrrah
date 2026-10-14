#!/usr/bin/env python3
from pathlib import Path
import hashlib,json
R=Path(__file__).resolve().parents[1];D=R/'docs/research';E=D/'dedup-500/evidence/jhnc--findimagedupes';P=D/'dedup-findimagedupes49-controls-20261014.json';d=json.loads(P.read_text());sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
assert sha(R/'scripts/check-dedup-findimagedupes49-20261014.py')==d['producer_sha256']
for n,h in d['source_pins'].items():assert sha(E/n)==h
mapping={'README.md':'00-README.md','COPYING':'01-COPYING','findimagedupes':'02-findimagedupes','Makefile':'03-Makefile','patchver':'04-patchver'}
tree=json.loads((E/'tree.json').read_text());assert tree['truncated']is False;blobs={v['path']:v for v in tree['tree']if v['type']=='blob'};assert set(blobs)==set(mapping)
files=[]
for name,local in mapping.items():
 b=(E/local).read_bytes();h=hashlib.sha1(b'blob '+str(len(b)).encode()+b'\0'+b).hexdigest();assert h==blobs[name]['sha']and len(b)==blobs[name]['size'];files.append(dict(path=name,local=str((E/local).relative_to(R)),sha256=sha(E/local),git_blob_sha1=h,bytes=len(b),lines=len(b.splitlines())))
s=(E/'02-findimagedupes').read_text();c=s.split('__C__\n',1)[1];assert hashlib.sha256(c.encode()).hexdigest()==d['upstream_c_sha256'];assert c in Path(d['adapter']).read_text()and sha(d['adapter'])==d['adapter_sha256']
for v in d['binaries'].values():assert sha(v['path'])==v['sha256']
rows={v['label']:v for v in d['records']};assert len(rows)==len(d['records'])==45
for v in rows.values():
 assert sha(v['stdout'])==v['stdout_sha256']and sha(v['stderr'])==v['stderr_sha256'];assert v['returncode']==0 or v['label']=='asan-short-fingerprint'
for case in d['scalar_cases']:
 data=[bytes.fromhex(v)for v in case['fingerprints']];expected=[]
 for i in range(case['new']if case['limit']else max(len(data)-1,0)):
  for j in range(i+1,len(data)):
   distance=sum(bin(x^y).count('1')for x,y in zip(data[i],data[j]))
   if distance<=case['threshold']:expected.extend([i,j,distance])
 assert expected==case['expected']==json.loads(Path(rows[f"zero-{case['index']:02}"]['stdout']).read_text())
assert json.loads(Path(rows['zero-identical']['stdout']).read_text())==[0,1,0]
assert json.loads(Path(rows['pattern-identical']['stdout']).read_text())!=[0,1,0]
assert 'heap-buffer-overflow'in Path(rows['asan-short-fingerprint']['stderr']).read_text()
F=Path(d['adapter']).parent;a=s.index('if (my ($thres_val, $thres_unit)');b=s.index('for (map {split',a);assert s[a:b]in (F/'threshold.pl').read_text()
for i,(value,threshold)in enumerate(d['threshold_cases']):assert list(map(int,Path(rows[f'threshold-{i:02}']['stdout']).read_text().split()))==[threshold,int(threshold<0 or threshold>256)]
a=s.index('sub finddupes {');b=s.index('# ======================================================================',a);assert s[a:b]in (F/'groups.pl').read_text();g=(F/'groups.sh').read_text();assert all("'"+n+"'"in g for n in ['A','B','C'])and g.count('VIEW \\')==1
O=D/'dedup-500/findimagedupes49-complete-source-review-20261014.json';assert not O.exists()
report=dict(status='complete5blob_source_review_bounded_original_helper_controls',repository=d['repository'],commit=d['commit'],index=49,whole_repo_complete=True,whole_runtime_qualified=False,fully_read_files=files,controls=str(P.relative_to(R)),controls_sha256=sha(P),auditor_sha256=sha(__file__),findings=d['findings']+['Fingerprinting retains first frame only, forced aspect-ratio160x160, grayscale/blur/normalize/equalize/16x16/1bpp; no pixel identity, alpha/color/frame or crop/rephoto guarantee.','Cache keyed by canonical path and fingerprint only, no source stamp/content or decoder-version key; existing entries reused without freshness verification unless rescan. Documentation acknowledges GraphicsMagick version invalidates DB and concurrent-write/crash corruption.','Source null-input sanity check rejects read_input AND null unconditionally, not only when stdin is a terminal; documented piped -print0 example is contradicted by inspected control flow. Not whole CLI runtime tested.','Pair comparison is quadratic, early-out perchunk; lookup allocated/recomputed everycall and all matching triplets materialized before transitive grouping, no cancellation/resource policy.','MD5 is optional verbosity output only, not proof of byte equality or acceptance stage. No source copied into library.','Makefile clean/reallyclean discard checkout/untracked work; release helper edits source in place and invokes git. Read only, not executed.','README recursion note contradicts implemented -R; rescan claims prune but no explicit prune assignment found in inspected source.'],scope=d['scope']+' All5captured blobs fully read and independently matched full tree Gitblob SHA1/size and SHA256. Captured commit-to-tree binding not independently fetched; full repository source review is distinct from runtime qualification. GPLv3-or-later source; reference only, no library source adoption.')
O.write_text(json.dumps(report,indent=2)+'\n');print('5 full blobs;24scalar,13threshold,2init-state,1ASan,1group,1syntax controls independently audited')
