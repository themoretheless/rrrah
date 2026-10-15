from pathlib import Path
import hashlib,json,subprocess
R=Path(__file__).resolve().parents[1];D=R/'docs/research/dedup-500'
h=lambda b:hashlib.sha256(b).hexdigest()
up=json.loads((D/'x9133-upstream-tree-20261015.json').read_text());tree=up['tree'];assert not tree['truncated']
mapping={'README.md':'00-README.md','license.txt':'01-license.txt','bin/x9dedupe':'02-x9dedupe'}
rows=[]
for row in tree['tree']:
 if row['type']!='blob':continue
 b=up['ignore'].encode() if row['path']=='.gitignore' else (D/'evidence/jim-collier--x9dedupe'/mapping[row['path']]).read_bytes()
 assert len(b)==row['size'] and hashlib.sha1(b'blob '+str(len(b)).encode()+b'\0'+b).hexdigest()==row['sha'],row
 rows.append(dict(row,sha256=h(b),content=b.decode(),fully_read=True))
def tree_hash(entries):
 b=b''.join(x['mode'].lstrip('0').encode()+b' '+x['path'].encode()+b'\0'+bytes.fromhex(x['sha']) for x in sorted(entries,key=lambda x:x['path']+('/' if x['type']=='tree' else '')))
 return hashlib.sha1(b'tree '+str(len(b)).encode()+b'\0'+b).hexdigest()
root=[x for x in tree['tree'] if '/' not in x['path']]
child=[dict(x,path=x['path'].split('/',1)[1]) for x in tree['tree'] if x['path'].startswith('bin/')]
assert tree_hash(root)==tree['sha'];assert tree_hash(child)==next(x['sha'] for x in root if x['path']=='bin')
for run in [1,2]:
 p=D/f'x9133-shell-controls-run{run}-20261015.json';r=json.loads(p.read_text());assert r['status']=='passed' and r['controls']==10
review={'status':'complete_source_read_scoped_shell_controls','repository':'jim-collier/x9dedupe','index':133,'commit':up['commit'],'root_tree':tree['sha'],'whole_repo_complete':True,'full_repository_read':True,'blobs':rows,'tree_object_hashes_reconstructed':True,'tracked_blobs':4,'tracked_trees':2,'license':'GPL-3.0; no production code copied','reading_scope':'All1550shell lines, README121lines, full.gitignore; GPL3byteidentical previouslyread437license. All generatedscript source read; no assets or hidden blobs omitted.','runtime_controls':{'runs':2,'controls_per_run':10,'script':'scripts/qualify-x9133-shell-controls-20261015.py','logs':[f'docs/research/dedup-500/x9133-shell-controls-run{i}-20261015.json' for i in [1,2]]},'findings':[{'status':'runtime_selected_helpers','finding':'Root slash lost, spaces beside separators stripped, backslash rewritten, empty trim leaks prior output, eval executes command substitution from data.'},{'status':'runtime_source_operator_reproduction','finding':'Unquoted output options split whitespace paths; set-e pipeline masks child nonzero without pipefail.'},{'status':'static_only','finding':'Recopy delegates cp and uses existence checks rather than byteverification; two renames without rollback, source mutation between checks remains possible.'},{'status':'static_only','finding':'ZFS property restoration only normalpath; cleanup does not restore; lsof failures swallowed and integer calendar arithmetic used for elapsed checks.'}],'transfer_decision':'Reference only: preserve path bytes/argv arrays; failclosed childstatus; revalidate before action; explicit rollback. Do not adopt storage mutations or shell eval.','limits':['No wholeCLI/root/rmlint/ZFS/Btrfs/XFS/generatedrecopy execution','No upstreamtests detected in pinned tree; no installed Linux dependency or Bash4 qualification','No storage saving, throughput, corpus, platform or library speedup claim']}
out=D/'x9133-complete-source-review-20261015.json';assert not out.exists();out.write_text(json.dumps(review,indent=2)+'\n')
p=D/'reviews/133.json';original=p.read_bytes();(D/'x9133-prior-registry-20261015.json').write_bytes(original);d=json.loads(original);d.update(full_repository_read=True,complete_source_review=str(out.relative_to(R)),review_scope='All4tracked text blobs and2trees fully read;10scoped selectedhelper/operator controls twice, no storageCLI execution',finding='Filesystem orchestration reference only; path/eval/pipeline and recopy mutation findings documented.');p.write_text(json.dumps(d,indent=2)+'\n')
print(json.dumps({'status':'verified','blobs':4,'trees':2,'controls_per_run':10}))
