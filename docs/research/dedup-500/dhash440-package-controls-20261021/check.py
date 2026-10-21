from pathlib import Path
import tarfile,json,hashlib
root=Path('.dedup-work/dhash440-20261021');consumer=Path('.dedup-work/dhash440-consumer-20261021');folder=Path(__file__).resolve().parent
archive=root/'review-package.tgz';installed=json.loads((consumer/'node_modules/@claudiu-ceia/dhash/package.json').read_text());expected=json.loads((root/'package.json').read_text())
for field in ['name','version','description','homepage','repository','keywords','engines','sideEffects']:assert installed[field]==expected[field]
assert installed['dependencies']['sharp']=='0.35.4';assert installed.get('private') is not True
with tarfile.open(archive) as t:files=[{'path':m.name,'bytes':m.size,'sha256':hashlib.sha256(t.extractfile(m).read()).hexdigest()} for m in t.getmembers() if m.isfile()]
result={'archive_sha256':hashlib.sha256(archive.read_bytes()).hexdigest(),'files':files,'installed_name':installed['name'],'installed_version':installed['version'],'sharp':'0.35.4','metadata_matches':True,'pack_command':'deno run -A scripts/pack_npm.ts review-package.tgz','pack_exit_code':0,'consumer_install':'npm install --registry=https://registry.npmjs.org --ignore-scripts --no-audit --no-fund ../dhash440-20261021/review-package.tgz','consumer_runtime':'DHASH_FIXTURES=<pinned tests> node22 integration.mjs','runtime_exit_code':0,'scope':'Actual original packaging script, separate npm consumer, unchanged original npm_integration.mjs. Full test_npm.ts not run because Bun absent; no npm audit/TypeScript7/multi-Node/platform matrix or publication.'}
(folder/'result.json').write_text(json.dumps(result,indent=2)+'\n')
