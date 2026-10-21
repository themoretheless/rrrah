import sys,json,base64,hashlib
from pathlib import Path
sys.path.insert(0,str(Path('.dedup-work/photos20-yaml-deps').resolve()))
import yaml
C=Path(sys.argv[1]);m=json.loads((C/'package.json').read_text());n=json.loads((C/'package-lock.json').read_text());p=yaml.safe_load((C/'pnpm-lock.yaml').read_text());errors=[];checked=0
for manager,records in [('npm',n['packages']),('pnpm',p['packages'])]:
 for key,v in records.items():
  assert isinstance(v,dict)
  integrity=v.get('integrity') if manager=='npm' else v.get('resolution',{}).get('integrity')
  if integrity:
   for token in integrity.split():
    algo,_,encoded=token.partition('-')
    try:
     digest=base64.b64decode(encoded,validate=True);assert algo in ['sha1','sha256','sha384','sha512'];assert len(digest)==hashlib.new(algo).digest_size;checked+=1
    except Exception:errors.append({'manager':manager,'key':key,'problem':'integrity_syntax'})
root=p['importers']['.'];diff=[]
for kind in ['dependencies','devDependencies']:
 assert n['packages'][''][kind]==m[kind]
 for name,spec in m[kind].items():
  actual=root.get(kind,{}).get(name,{}).get('specifier')
  if actual!=spec:diff.append({'kind':kind,'name':name,'manifest':spec,'pnpm':actual})
assert not errors
report={'npm_lockfile_version':n['lockfileVersion'],'npm_root_version':n['packages']['']['version'],'npm_package_records':len(n['packages']),'npm_direct_manifest_maps_equal':True,'pnpm_lockfile_version':p['lockfileVersion'],'pnpm_package_records':len(p['packages']),'pnpm_snapshot_records':len(p['snapshots']),'pnpm_direct_manifest_differences':diff,'integrity_tokens_syntax_checked':checked,'errors':errors,'record_tables_sha256':{k:hashlib.sha256(json.dumps(v,sort_keys=True).encode()).hexdigest() for k,v in [('npm_packages',n['packages']),('pnpm_packages',p['packages']),('pnpm_snapshots',p['snapshots'])]},'limits':['All records structurally consumed, not package source audit, tarball download/integrity authentication or installation test.','No dependency semver/peer closure solver run.']}
print(json.dumps(report,indent=2))
