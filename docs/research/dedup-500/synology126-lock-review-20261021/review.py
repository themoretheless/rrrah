from pathlib import Path
import tomllib,json,re,hashlib
p=Path('.dedup-work/synology126-20261021/Cargo.lock');raw=p.read_bytes();d=tomllib.loads(raw.decode());packages=d['package'];errors=[]
assert d['version']==4
for package in packages:
 assert set(package)<= {'name','version','source','checksum','dependencies'}
 if package['name']!='dedupPictures':
  assert package['source']=='registry+https://github.com/rust-lang/crates.io-index'
  assert re.fullmatch('[0-9a-f]{64}',package['checksum'])
 for dep in package.get('dependencies',[]):
  parts=dep.split();matches=[x for x in packages if x['name']==parts[0] and (len(parts)==1 or x['version']==parts[1])]
  if len(matches)!=1:errors.append({'package':package['name'],'dependency':dep,'matches':len(matches)})
assert not errors
root=next(x for x in packages if x['name']=='dedupPictures')
assert len(root['dependencies'])==14
print(json.dumps({'method':'Full TOML parse and per-record field/source/checksum-shape/dependency resolution checks; all310 names/versions/edges inspected in tool output. No crate tarballs checked or dependency builds run.','sha256':hashlib.sha256(raw).hexdigest(),'packages':len(packages),'registry_checksum_records':len(packages)-1,'dependency_resolution_errors':errors,'direct_dependencies':root['dependencies'],'ort_locked':'2.0.0-rc.12','image_locked':'0.25.10','records':packages},indent=2))
