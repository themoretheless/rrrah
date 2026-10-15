"""Reconstruct exactly3378 files/variant from Git+saved transforms into ignored durable workspace."""
from pathlib import Path
import ast,copy,hashlib,json,os,shutil,subprocess,datetime
R=Path(__file__).resolve().parents[1];D=R/'docs/research';W=R/'.dedup-work/20261015-common-v5';E=D/'dedup-durable-pair-v5-20261015';assert not W.exists() and not E.exists();W.mkdir(parents=True);(W/'.gitignore').write_text('*\n');E.mkdir();h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();legacy_path=D/'dedup-common-worker-fixed-collection-pair-v2-prepared-20261015.json';legacy=json.loads(legacy_path.read_text());original_path=D/'dedup-committed-build-source-415443172-v2-20261015.json';original=json.loads(original_path.read_text());assert len(original['source_files'])==3378
# Independently bind every selected blob/path/mode to the original committed Git tree.
rawtree=subprocess.check_output(['git','ls-tree','-rz',original['commit']],cwd=R);tree={}
for raw in rawtree.split(b'\0'):
 if not raw:continue
 meta,name=raw.split(b'\t',1);mode,kind,sha=meta.decode().split();tree[name.decode()]=(mode,kind,sha)
for row in original['source_files']:
 assert tree[row['path']]==(row['mode'],'blob',row['git_blob_sha1']);assert row['mode'] in ['100644','100755'];assert not Path(row['path']).is_absolute() and '..' not in Path(row['path']).parts
proc=subprocess.Popen(['git','cat-file','--batch'],cwd=R,stdin=subprocess.PIPE,stdout=subprocess.PIPE);payload,_=proc.communicate(''.join(r['git_blob_sha1']+'\n' for r in original['source_files']).encode());assert proc.returncode==0;offset=0;B=W/'original415';B.mkdir()
for row in original['source_files']:
 end=payload.index(b'\n',offset);sha,kind,size=payload[offset:end].split();size=int(size);raw=payload[end+1:end+1+size];offset=end+size+2;assert kind==b'blob' and sha.decode()==row['git_blob_sha1'] and size==row['bytes'];assert hashlib.sha1(b'blob '+str(size).encode()+b'\0'+raw).hexdigest()==row['git_blob_sha1'] and hashlib.sha256(raw).hexdigest()==row['sha256'];p=B/row['path'];p.parent.mkdir(parents=True,exist_ok=True);p.write_bytes(raw);p.chmod(0o755 if row['mode']=='100755' else 0o644)
assert offset==len(payload);del payload
for name in ['baseline','candidate']:shutil.copytree(B,W/name)
# Replay exact archived producer statements, not an approximate reimplementation.
F=W/'candidate';ns=dict(R=R,D=D,B=B,F=F,Path=Path,hashlib=hashlib)
def section(path,start,end):
 body=ast.parse(path.read_text()).body
 def assignment(n,name):return isinstance(n,ast.Assign) and any(isinstance(t,ast.Name) and t.id==name for t in n.targets)
 a=next(i for i,n in enumerate(body) if assignment(n,start));b=next(i for i,n in enumerate(body[a+1:],a+1) if assignment(n,end));return ast.Module(body=body[a:b],type_ignores=[])
prep=R/'scripts/prepare-dedup-selective-collection-full-v2-20261015.py';exec(compile(section(prep,'archive','changed'),str(prep),'exec'),ns);integration=json.loads((D/'dedup-selective-collection-full-v2-prepared-20261015.json').read_text())
for row in integration['changed']:assert h(F/row['path'])==row['candidate_sha256']
bench=R/'scripts/prepare-dedup-actual-collection-benchmark-v1-20261015.py';exec(compile(section(bench,'p','changed'),str(bench),'exec'),ns)
O=E/'overlays';O.mkdir();overlay_paths=['crates/rrrah-dedup/src/model_worker.rs','crates/rrrah-dedup/tests/worker_group.rs']
for path in overlay_paths:
 want=next(r['sha256'] for r in legacy['variants'][0]['source_files'] if r['path']==path);raw=(R/path).read_bytes();assert hashlib.sha256(raw).hexdigest()==want;out=O/Path(path).name;out.write_bytes(raw)
 for name in ['baseline','candidate']:(W/name/path).write_bytes(raw)
for path in ['crates/rrrah-dedup/src/exact.rs','crates/rrrah-dedup/src/collection.rs']:(O/('candidate-'+Path(path).name)).write_bytes((F/path).read_bytes())
variants=[]
for variant in legacy['variants']:
 source=W/variant['variant'];assert {str(p.relative_to(source)) for p in source.rglob('*') if p.is_file()}=={r['path'] for r in variant['source_files']}
 for row in variant['source_files']:assert h(source/row['path'])==row['sha256'],row['path']
 item=copy.deepcopy(variant);item['source']=str(source);variants.append(item)
manifest=dict(status='exact_legacy_source_pair_recovered_to_durable_workspace_not_qualified',variants=variants,legacy_manifest=str(legacy_path.relative_to(R)),legacy_manifest_sha256=h(legacy_path),original_git_manifest=str(original_path.relative_to(R)),original_git_manifest_sha256=h(original_path),original_commit=original['commit'],only_candidate_differences=legacy['only_candidate_differences'],overlay_artifacts=[dict(path=str(p.relative_to(R)),sha256=h(p)) for p in sorted(O.iterdir())],transform_scripts=[dict(path=str(p.relative_to(R)),sha256=h(p)) for p in [prep,bench]],target=str(W/'target'),tmp_root=str(W/'tmp'),artifact_root=str(W/'artifacts'),git_ceiling=str(W),producer_sha256=h(__file__),scope='Each3378 rawsource file/variant matcheslegacy pinnedbytes exactly. Git415selectedblobs/path/mode verified; savedASTtransforms exactlyreplayed; onlysource locations changed. Ignoredworkspace directory, noGitcheckout/mutationofactivework. Fullfresh162targetbaseline+candidate gates required becausepriorTMPsources/binaries/logs lost. CurrentRoot/cache/trace/allformats/platform/corpus/performance remainunqualified.')
out=D/'dedup-durable-source-pair-v5-prepared-20261015.json';assert not out.exists();out.write_text(json.dumps(manifest,indent=2)+'\n');print('Recovered baseline+candidate3378files each byte-exact, durable ignoredworkspace; no passing gate claim')
