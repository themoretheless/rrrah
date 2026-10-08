"""Verify terminal native Linux exhaustive/sampled projective-domain tests."""
import hashlib,json,re
from pathlib import Path
root=Path(__file__).resolve().parents[1];b=root/'docs/research';manifest=b/'dedup-projective-domains-linux-snapshot.json';m=json.loads(manifest.read_text());snapshot=Path(m['snapshot']);h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
for rel,digest in m['file_hashes'].items():assert h(snapshot/rel)==digest,rel
current=[rel for rel in m['file_hashes'] if rel.startswith('crates/rrrah-dedup/') and (rel.endswith('.rs') or rel.endswith('/Cargo.toml'))]
for rel in current:assert h(root/rel)==m['file_hashes'][rel],rel
log=b/'dedup-projective-domains-linux-native.log';s=log.read_text();suites={'projective_domains':4,'projective_exhaustive_domains':3,'projective_geometry':14};declared=set()
for name,count in suites.items():
 names=set(re.findall(r'#\[test\]\s*(?:#\[[^\n]*\]\s*)*fn\s+(\w+)',(snapshot/'crates/rrrah-dedup/tests'/f'{name}.rs').read_text()));assert len(names)==count and not declared&names;declared|=names
actual=re.findall(r'^test (\w+) \.\.\. ok$',s,re.M);assert len(actual)==len(set(actual))==21 and set(actual)==declared
results=re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;',s);assert len(results)==3 and sorted(int(n) for n,_,_ in results)==[3,4,14] and all(int(f)==int(i)==0 for _,f,i in results)
assert set(re.findall(r'Running tests/(\w+)\.rs',s))==set(suites) and 'FAILED' not in s and 'error:' not in s
graph=b/'dedup-projective-domains-minimal-dependency-tree.log';assert all(name not in graph.read_text() for name in ['rrrah-core','rrrah-decode','hayro'])
files=[manifest,log,graph,root/'scripts/run-dedup-linux-projective-domains.sh',Path(__file__)]
(b/'dedup-projective-domains-linux-state.json').write_text(json.dumps({'status':'verified_native_linux_projective_domain_selection','terminal_exit_code':0,'tests':21,'suites':suites,'snapshot_files':len(m['file_hashes']),'current_dedup_files_verified':len(current),'pins':{str(p):h(p) for p in files},'scope':'Native Linux aarch64 no-default-feature domain/projective tests, exact names/counts and frozen source checks. All current dedup files match snapshot. Core/decode/PDF excluded by matching minimal dependency graph. No Linux real pixels/decoder, Windows or full crate suite claim.'},indent=2)+'\n');print('Verified21 native Linux tests')
