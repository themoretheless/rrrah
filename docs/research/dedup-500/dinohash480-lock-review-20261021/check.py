import json,tomllib,re,hashlib
from pathlib import Path
root=Path('/Users/themoretheless/.codex/tmp/rrrah-dinohash-source-20261013')
lock=tomllib.loads((root/'uv.lock').read_text());packages=lock['package'];names={p['name'] for p in packages};artifacts=[];edges=[]
for p in packages:
 for a in ([p['sdist']] if 'sdist' in p else [])+p.get('wheels',[]):
  assert re.fullmatch(r'sha256:[0-9a-f]{64}',a['hash']);assert a['url'].startswith('https://');assert a['size']>0;artifacts.append(a)
 for d in p.get('dependencies',[]):assert d['name'] in names;edges.append((p['name'],d['name']))
out=dict(lock_version=lock['version'],revision=lock['revision'],requires_python=lock['requires-python'],resolution_markers=lock['resolution-markers'],packages=[dict(name=p['name'],version=p['version'],source=p['source'],markers=p.get('resolution-markers',[])) for p in packages],package_records=len(packages),unique_names=len(names),artifact_hashes_checked=len(artifacts),dependency_edges_checked=len(edges),scope='Whole generated TOML parsed; every package/dependency/artifact examined structurally, not manual reading of every wheel URL. Artifacts not downloaded, dependencies not installed or model executed.')
Path(__file__).with_name('result.json').write_text(json.dumps(out,indent=2)+'\n');print({k:v for k,v in out.items() if k!='packages'})
