"""Independently verify36 full-source blobs,11 canonical trees and scoped native evidence."""
from pathlib import Path
import ast,hashlib,json,zipfile,base64,csv,zlib
R=Path(__file__).resolve().parents[1];D=R/'docs/research';h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();ns={'Path':Path,'hashlib':hashlib};helper=R/'scripts/audit-dedup-five-complete-source-trees-20261015.py';exec(compile(ast.Module(body=[n for n in ast.parse(helper.read_text()).body if isinstance(n,ast.FunctionDef)],type_ignores=[]),'canonical','exec'),ns);verified=[]
for slug,repo,index,nb,nt in [('go26','m-manu/go-find-duplicates',26,25,8),('string176','cshung/StringDedupAnalyzer',176,11,3)]:
 regpath=D/f'dedup-500/reviews/{index:03}.json';reg=json.loads(regpath.read_text());rp=R/reg['complete_source_review'];review=json.loads(rp.read_text());pinpath=D/f'{slug}-commit-tree-pin-20261015.json';pin=json.loads(pinpath.read_text());base=D/'dedup-500/evidence'/repo.replace('/','--');tree=json.loads((base/'tree.json').read_text());assert not tree['truncated'];built=ns['reconstruct'](tree['tree']);assert len(built)==nt and built['']==pin['tree']==review['tree'];assert reg['commit']==pin['commit']==review['commit'] and reg['repository']==pin['repository']==repo;assert reg['full_repository_read'] and review['whole_repository_complete'];assert pin['blobs']==review['blobs'] and h(pinpath)==review['pin_sha256'];entries=[e for e in tree['tree'] if e['type']=='blob'];assert len(entries)==nb;assert {str(p.relative_to(base/'full-source')) for p in (base/'full-source').rglob('*') if p.is_file()}=={e['path'] for e in entries}
 for e in entries:
  p=base/'full-source'/e['path'];b=next(b for b in pin['blobs'] if b['path']==e['path']);raw=p.read_bytes();assert e['mode']==b['mode'] and len(raw)==e['size']==b['bytes'];assert ns['git_object']('blob',raw)==e['sha']==b['git_blob'] and h(p)==b['sha256']
 missing=False
 try:missing=ns['reconstruct'](tree['tree'][:-1])['']!=pin['tree']
 except AssertionError:missing=True
 assert missing;assert ns['git_object']('blob',(base/'full-source/README.md').read_bytes()+b'!')!=next(e['sha'] for e in entries if e['path']=='README.md')
 cp=R/review['controls'];assert h(cp)==review['controls_sha256'];runtime=json.loads(cp.read_text())
 for a in runtime['artifacts']:assert h(R/a['path'])==a['sha256']
 for p,sha in runtime['source_sha256'].items():assert h(R/p)==sha
 if slug=='go26':
  assert runtime['original_top_level_cases']==5 and runtime['authored_top_level_cases']==8 and runtime['repeats']==2
  for n in ['with-crc-run','with-crc-replay']:
   data=[json.loads(l) for l in (cp.parent/f'{n}.jsonl').read_text().splitlines()];assert not any(x['Action']=='fail' or(x['Action']=='skip' and x.get('Test')) for x in data);passed=[x['Test'] for x in data if x['Action']=='pass' and x.get('Test')];assert len(passed)==19 and sum('/' not in t for t in passed)==13
  retained=json.loads((R/runtime['retained_binaries']).read_text());assert h(R/runtime['retained_binaries'])==runtime['retained_binaries_sha256'];assert len(retained['binaries'])==3 and len(retained['modules'])==8
  assert h(retained['go'])==retained['go_sha256'];originalsums=dict(line.rsplit(' ',1) for line in (base/'full-source/go.sum').read_text().splitlines())
  for b in retained['binaries']:
   assert h(b['binary'])==b['binary_sha256'];name=b['package'];names=(cp.parent/f'{name}-inventory.log').read_text().splitlines();log=(cp.parent/f'{name}-binary-replay.log').read_text();assert '\nPASS\n' in log and '--- FAIL:' not in log
   assert all(('--- PASS: '+n+' ') in log for n in names)
  for m in retained['modules']:
   p=Path(m['path']);assert h(p)==m['sha256'];z=zipfile.ZipFile(p);aggregate=hashlib.sha256()
   for n in sorted(z.namelist()):
    if n.endswith('/'):continue
    raw=z.read(n);aggregate.update((hashlib.sha256(raw).hexdigest()+'  '+n+'\n').encode());root=n.split('@')[0]+'@'+n.split('@')[1].split('/')[0];assert (Path('/private/tmp/rrrah-go26-modcache-20261015')/root/n[len(root)+1:]).read_bytes()==raw
   assert 'h1:'+base64.b64encode(aggregate.digest()).decode()==m['h1']==originalsums[m['go_sum_key']]
  oracle=json.loads((cp.parent/'crc32-collision-oracle.json').read_text());a=bytes.fromhex(oracle['a_hex']);b=bytes.fromhex(oracle['b_hex']);assert a!=b and len(a)==len(b)==8 and zlib.crc32(a)==zlib.crc32(b)==int(oracle['crc32_hex'],16);assert 'module lookup disabled by GOPROXY=off' in (cp.parent/'module-verify.log').read_text()
  prior=json.loads((R/review['previous_review']).read_text());assert h(R/review['previous_review'])==review['previous_review_sha256'];assert all(prior['source_file_hashes'][b['path']]==b['sha256'] for b in pin['blobs'])
 else:
  assert runtime['native_platform_cases']==2 and runtime['explicit_windows_ClrMD_adapter_cases']==7 and runtime['repeats']==2
  for cohort,count in [('native-platform',2),('windows-adapter',7)]:
   a=(cp.parent/f'{cohort}-run.log').read_bytes();b=(cp.parent/f'{cohort}-replay.log').read_bytes();assert a==b;lines=a.decode().splitlines();assert len(lines)==count and all(l.startswith('PASS ') for l in lines)
  for b in runtime['binaries']:assert h(b['path'])==b['sha256']
  assert runtime['original_program_sha256']==h(base/'full-source/src/StringDedupAnalyzer/Program.cs')
  assert len(list(csv.DictReader((base/'full-source/data.csv').open())))==6
 verified.append(dict(index=index,repository=repo,blobs=nb,trees=nt,review_sha256=h(rp),registry_sha256=h(regpath),missing_blob_rejected=True,changed_blob_rejected=True))
out=D/'dedup-go26-string176-complete-source-tree-audit-20261015.json';assert not out.exists();out.write_text(json.dumps(dict(status='verified36_blobs11_trees_scoped_native_evidence',repositories=verified,auditor_sha256=h(__file__),limits=['No full backend/platform/corpus/quality/scale/library speedup proof. Go wholemoduleverify offlinefailure retained;8builddependencies checked. CSharp explicit adapters, not realdump/DAC/netcore3.1 orWindows execution.']),indent=2)+'\n');print('36blobs11trees, Go19passx2+3binaries, CSharp2+7controlsx2 verified')
