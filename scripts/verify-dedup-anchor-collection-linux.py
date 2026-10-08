"""Exact declared test names, terminal Linux results and immutable source pins."""
import hashlib,json,re
from pathlib import Path
D=Path('docs/research');manifest=D/'dedup-anchor-collection-linux-snapshot.json';m=json.loads(manifest.read_text());snapshot=Path(m['snapshot']);h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
for rel,digest in m['file_hashes'].items():assert h(snapshot/rel)==digest,rel
changed=[]
archives={'crates/rrrah-dedup/src/anchor_rank_file.rs':'docs/research/dedup-anchor-file-before-fallback.rs'}
for rel,digest in m['file_hashes'].items():
 if rel.startswith('crates/rrrah-dedup/') and h(rel)!=digest:
  assert rel in archives and h(archives[rel])==digest,rel
  changed.append(rel)

suites=['anchor_collection','anchor_collection_preflight','anchor_collection_scaled','anchor_candidate_search','anchor_rank_file','anchor_rank','local_rank','local_rank_cached','piecewise_local_rank','piecewise_warp','mesh_rank_regions','projective_grid','mesh_grid','mesh_rank','rank_region','projective_exhaustive_domains','projective_domains','projective_geometry'];declared=set();counts={}
for name in suites:
 names=set(re.findall(r'#\[test\]\s*(?:#\[[^\n]*\]\s*)*fn\s+(\w+)',(snapshot/'crates/rrrah-dedup/tests'/f'{name}.rs').read_text()));assert names and not names&declared;declared|=names;counts[name]=len(names)
log=D/'dedup-anchor-collection-linux-native.log';s=log.read_text();actual=re.findall(r'^test (\w+) \.\.\. ok$',s,re.M);assert len(actual)==len(set(actual))==len(declared) and set(actual)==declared
assert set(re.findall(r'Running tests/(\w+)\.rs',s))==set(suites)
results=re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;',s);assert len(results)==len(suites) and sorted(int(n) for n,_,_ in results)==sorted(counts.values()) and all(int(f)==int(i)==0 for _,f,i in results)
assert 'FAILED' not in s and 'error:' not in s
out=D/'dedup-anchor-collection-linux-state.json';out.write_text(json.dumps({'status':'verified_native_linux_indexed_anchor_collection_and_files','tests':len(declared),'suites':counts,'snapshot_files':len(m['file_hashes']),'current_files_changed_after_snapshot':changed,'pins':{str(p):h(p) for p in [manifest,log,Path(__file__),Path('scripts/run-dedup-linux-local-rank.sh')]},'scope':'Native Linux aarch64 automatic candidate/guarded file fixture tests including scale, full original points, exact direct parity, request-token cancellation, file mutation and managed release, plus geometry/rank regressions. Current indexed collection lifecycle,12preiteration cases and exact scaled indexed-versus-all-direct parity included. Exact frozen dedup source/test bytes; any newer fallback wrapper is excluded and the archived module matches snapshot bytes; other dependencies preserved from prior frozen file snapshot; no real corpus, Windows, unrelated precision, full package or collection admission proof.'},indent=2)+'\n');print('Verified',len(declared),'native Linux tests')
