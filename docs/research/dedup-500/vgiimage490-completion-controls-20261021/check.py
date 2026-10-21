from pathlib import Path
import tomllib,json,hashlib,subprocess
r=Path('.dedup-work/vgiimage490-20261021');generated=Path('.dedup-work/vgiimage490-generated-20261021');out=Path(__file__).resolve().parent
lock=tomllib.loads((r/'Cargo.lock').read_text());pkgs=lock['package'];seen=set();checksums=0
for p in pkgs:
 key=(p['name'],p['version'],p.get('source'));assert key not in seen;seen.add(key)
 if 'checksum' in p:assert len(bytes.fromhex(p['checksum']))==32;checksums+=1
versions=[]
for tag in ['v0.1.1','v0.1.2']:
 p=subprocess.run(['bash',str(r/'ci/check-version.sh'),tag],capture_output=True,text=True);assert p.returncode==(0 if tag=='v0.1.1' else 1);versions.append(dict(tag=tag,exit_code=p.returncode,output=(p.stdout+p.stderr).strip()))
fixtures=[]
for name in ['gradient.png','gradient.jpg','gps.jpg']:
 original=(r/'test/sql/data'/name).read_bytes();new=(generated/'test/sql/data'/name).read_bytes();assert original==new
 fixtures.append(dict(path=name,bytes=len(original),sha256=hashlib.sha256(original).hexdigest(),generated_byte_identical=True))
# Exercise original awk preprocessor against the whole original SQL hash file.
text=(r/'test/sql/hash.test').read_text();preprocess=[]
for transport in ['http','subprocess','unix']:
 p=subprocess.run(['awk','-v','transport='+transport,'-f',str(r/'ci/preprocess-require.awk')],input=text,capture_output=True,text=True,check=True)
 assert ('INSTALL httpfs FROM core;' in p.stdout)==(transport=='http');assert 'require-env VGI_IMAGE_WORKER' in p.stdout
 preprocess.append(dict(transport=transport,httpfs_injected=transport=='http',require_env_preserved=True))
result=dict(lock_packages=len(pkgs),checksum_records=checksums,lock_scope='Every package structurally assessed, checksum length/uniqueness checked; original locked build/test succeeds. Not manual transitive dependency review.',fixture_regeneration=fixtures,version_script=versions,preprocessor=preprocess,scope='Original example generator, version script and awk executed; no SQL runner, Docker or wasm runtime.')
(out/'result.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result,indent=2))
