"""Verify frozen source and exact test declarations of native Linux local-rank gate."""
import hashlib,json,re
from pathlib import Path
root=Path(__file__).resolve().parents[1];D=root/'docs/research';p=D/'dedup-local-rank-linux-snapshot.json';m=json.loads(p.read_text());snapshot=Path(m['snapshot']);h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
for rel,digest in m['file_hashes'].items():assert h(snapshot/rel)==digest,rel
current=[rel for rel in m['file_hashes'] if rel.startswith('crates/rrrah-dedup/')]
for rel in current:assert h(root/rel)==m['file_hashes'][rel],rel
suites={'local_rank':5,'mesh_rank_regions':3,'projective_grid':3,'mesh_grid':2,'mesh_rank':2,'rank_region':7,'projective_exhaustive_domains':3,'projective_domains':4,'projective_geometry':14};declared=set()
for name,count in suites.items():
 names=set(re.findall(r'#\[test\]\s*(?:#\[[^\n]*\]\s*)*fn\s+(\w+)',(snapshot/'crates/rrrah-dedup/tests'/f'{name}.rs').read_text()));assert len(names)==count and not names&declared;declared|=names
log=D/'dedup-local-rank-linux-native.log';s=log.read_text();actual=re.findall(r'^test (\w+) \.\.\. ok$',s,re.M);assert len(actual)==len(set(actual))==43 and set(actual)==declared
results=re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;',s);assert len(results)==9 and sorted(int(n) for n,_,_ in results)==sorted(suites.values()) and all(int(f)==int(i)==0 for _,f,i in results)
assert set(re.findall(r'Running tests/(\w+)\.rs',s))==set(suites) and 'FAILED' not in s and 'error:' not in s
files=[p,log,root/'scripts/run-dedup-linux-local-rank.sh',Path(__file__)]
result={'status':'verified_native_linux_local_rank_projective_regions','terminal_exit_code':0,'tests':43,'suites':suites,'current_dedup_files_verified':len(current),'snapshot_files':len(m['file_hashes']),'pins':{str(p):h(p) for p in files},'scope':'Native Linux aarch64 raster authored geometry/atlas/selected-region/local-rank tests. Current dedup files match frozen snapshot. Not decoder real corpus, Windows, full crate suite, precision or comparative performance.'};(D/'dedup-local-rank-linux-state.json').write_text(json.dumps(result,indent=2)+'\n');print('Verified43 native Linux tests')
