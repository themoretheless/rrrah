from pathlib import Path
import os,subprocess,json,hashlib
R=Path('/Users/themoretheless/Documents/Sources/rrrah');W=Path('/private/tmp/rrrah-go26-native-20261015');E=R/'docs/research/go26-native-controls-20261015';GO='/Users/themoretheless/.codex/tmp/rrrah-go-1.27.2-20261012/go/bin/go';env=dict(os.environ,GOTOOLCHAIN='local',GOPROXY='off',GOSUMDB='off',GOMODCACHE='/private/tmp/rrrah-go26-modcache-20261015',GOCACHE='/private/tmp/rrrah-go26-buildcache-20261015',GOMAXPROCS='2');h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();records=[]
for pkg,name in [('.','main'),('./service','service'),('./bytesutil','bytesutil')]:
 binary=W/f'{name}.test'
 with (E/f'compile-{name}.log').open('wb') as log:subprocess.run([GO,'test','-mod=readonly','-c','-o',str(binary),pkg],env=env,cwd=W,stdout=log,stderr=subprocess.STDOUT,check=True)
 for mode,args in [('inventory',['-test.list','.']),('binary-replay',['-test.v'])]:
  with (E/f'{name}-{mode}.log').open('wb') as log:subprocess.run([str(binary),*args],env=env,cwd=W if pkg=='.' else W/pkg,stdout=log,stderr=subprocess.STDOUT,check=True)
 records.append(dict(package=pkg,binary=str(binary),binary_sha256=h(binary)))
with (E/'module-verify.log').open('wb') as log:subprocess.run([GO,'mod','verify'],env=env,cwd=W,stdout=log,stderr=subprocess.STDOUT,check=True)
mods=[]
for p in sorted(Path(env['GOMODCACHE']).rglob('*.zip')):mods.append(dict(path=str(p),sha256=h(p),ziphash=p.with_suffix('.ziphash').read_text().strip()))
(E/'retained-binaries.json').write_text(json.dumps(dict(go=GO,go_sha256=h(GO),binaries=records,modules=mods,environment= {k:env[k] for k in ['GOTOOLCHAIN','GOPROXY','GOSUMDB','GOMODCACHE','GOCACHE','GOMAXPROCS']}),indent=2)+'\n');print('all3compiledbinaries replayed, modulesverified')
