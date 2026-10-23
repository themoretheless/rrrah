"""Complete structural traversal of pinned lockfiles; not full manual text review."""
import base64
import collections
import hashlib
import json
import re
from pathlib import Path
from urllib.parse import urlsplit

ROOT = Path(__file__).resolve().parents[3]
CACHE = ROOT / '.dedup-work/webpack070-20261023'
EVIDENCE = ROOT / 'docs/research/dedup-500/evidence/atlassian-labs--webpack-deduplication-plugin'
pins = {e['path']: e['sha'] for e in json.loads((EVIDENCE/'tree.json').read_text())['tree'] if e['type']=='blob'}
metadata = {}
def read(name):
    data = (CACHE/name).read_bytes()
    assert hashlib.sha1(b'blob '+str(len(data)).encode()+b'\0'+data).hexdigest()==pins[name]
    metadata[name] = {'bytes':len(data),'sha256':hashlib.sha256(data).hexdigest(),'git_blob_verified':True}
    return data.decode()

def check_integrity(value):
    for token in value.split():
        algorithm, encoded = token.split('-',1)
        decoded = base64.b64decode(encoded,validate=True)
        assert len(decoded)=={'sha1':20,'sha256':32,'sha384':48,'sha512':64}[algorithm]

npm = json.loads(read('package-lock.json'))
manifest = json.loads(read('package.json'))
yarn = read('yarn.lock')
entries = []
current = None
section = None
for number,line in enumerate(yarn.splitlines(),1):
    if not line or line.startswith('#'): continue
    if not line.startswith(' '):
        assert line.endswith(':')
        selectors = [x.strip().strip('"') for x in re.split(r', (?=(?:[^\"]*\"[^\"]*\")*[^\"]*$)',line[:-1])]
        current = {'selectors':selectors,'dependencies':{},'optionalDependencies':{}}
        entries.append(current); section = None
    elif line in ('  dependencies:','  optionalDependencies:'):
        section = line.strip()[:-1]
    elif line.startswith('    '):
        match = re.fullmatch(r'    ("[^"]+"|\S+) "([^"]+)"',line)
        assert match and section, (number,line)
        name, constraint = match.groups()
        current[section][name.strip('"')] = constraint
    else:
        match = re.fullmatch(r'  (version|resolved|integrity) (.+)',line)
        assert match and current, (number,line)
        key,value = match.groups();assert key not in current
        current[key] = value.strip('"')
selector_map = {}
for entry in entries:
    assert all(k in entry for k in ('version','resolved','integrity'))
    check_integrity(entry['integrity'])
    for selector in entry['selectors']:
        assert selector not in selector_map
        selector_map[selector]=entry
unresolved = []
for entry in entries:
    for kind in ('dependencies','optionalDependencies'):
        for name,constraint in entry[kind].items():
            if f'{name}@{constraint}' not in selector_map:
                unresolved.append([entry['selectors'][0],kind,name,constraint])
npm_entries = []
def walk(dependencies, parents=()):
    for name,record in dependencies.items():
        assert isinstance(record,dict) and isinstance(record['version'],str)
        if 'integrity' in record:check_integrity(record['integrity'])
        npm_entries.append((parents+(name,),record))
        walk(record.get('dependencies',{}),parents+(name,))
walk(npm['dependencies'])
comparisons = []
for kind in ('dependencies','devDependencies'):
    for name,constraint in manifest[kind].items():
        locked_yarn=selector_map.get(f'{name}@{constraint}')
        locked_npm=npm['dependencies'].get(name)
        comparisons.append({'name':name,'kind':kind,'requested':constraint,'npm':locked_npm and locked_npm['version'],'yarn':locked_yarn and locked_yarn['version']})
result = {'inputs':metadata,'npm_lock_version':npm['lockfileVersion'],'npm_records':len(npm_entries),'npm_record_fields':dict(collections.Counter(k for _,r in npm_entries for k in r)),'npm_resolved_hosts':dict(collections.Counter(urlsplit(r['resolved']).hostname for _,r in npm_entries if 'resolved' in r)),'yarn_records':len(entries),'yarn_selectors':len(selector_map),'yarn_edges':sum(len(e[k]) for e in entries for k in ('dependencies','optionalDependencies')),'yarn_resolved_hosts':dict(collections.Counter(urlsplit(e['resolved']).hostname for e in entries)),'unresolved_yarn_edge_selectors':unresolved,'direct_dependencies':comparisons,'direct_version_disagreements':[e for e in comparisons if e['npm']!=e['yarn']],'tarballs_fetched_or_verified':False,'full_manual_lock_text_review':False}
print(json.dumps(result,indent=2))
