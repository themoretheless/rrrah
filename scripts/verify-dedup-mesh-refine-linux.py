"""Qualify the terminal eight-suite native Linux raster gate and frozen sources."""
import hashlib,json,re
from pathlib import Path
root=Path(__file__).resolve().parents[1];base=root/'docs/research';manifest=base/'dedup-mesh-refine-linux-snapshot.json';m=json.loads(manifest.read_text());snapshot=Path(m['snapshot']);digest=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
for rel,h in m['file_hashes'].items():assert digest(snapshot/rel)==h,rel
current=[rel for rel in m['file_hashes'] if rel.startswith(('crates/rrrah-dedup/','crates/rrrah-core/')) and (rel.endswith('.rs') or rel.endswith('/Cargo.toml'))]
changed=[rel for rel in current if digest(root/rel)!=m['file_hashes'][rel]]
for rel in current:
 if rel.startswith('crates/rrrah-dedup/'):assert digest(root/rel)==m['file_hashes'][rel],rel
m['refreshed_current_files']=current
graph=base/'dedup-mesh-raster-dependency-tree.log';assert 'hayro' not in graph.read_text() and 'rrrah-decode' not in graph.read_text()
log=base/'dedup-mesh-refine-linux-native.log';text=log.read_text();expected={'rank_region':7,'triangulation':3,'diagonal_proposals':2,'piecewise_warp':4,'mesh_grid':2,'mesh_rank':2,'mesh_refine':3,'mesh_landmark_admission':3};seen={}
declared=set()
for name,count in expected.items():
 source=(snapshot/'crates/rrrah-dedup/tests'/f'{name}.rs').read_text();names=set(re.findall(r'#\[test\]\s*fn\s+(\w+)',source));assert len(names)==count,(name,names);assert not declared&names;declared|=names;seen[name]=count
actual=re.findall(r'^test (\w+) \.\.\. ok$',text,re.M);assert len(actual)==len(set(actual))==sum(expected.values());assert set(actual)==declared
results=re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;',text);assert len(results)==8;assert all(int(f)==int(i)==0 for _,f,i in results);assert sorted(int(n) for n,_,_ in results)==sorted(expected.values())
launched=set(re.findall(r'Running tests/(\w+)\.rs',text));assert launched==set(expected)
assert seen==expected,(seen,expected);assert 'FAILED' not in text and 'error:' not in text
paths=[manifest,log,graph,root/'scripts/run-dedup-linux-mesh-refine.sh',Path(__file__)]
(base/'dedup-mesh-refine-linux-state.json').write_text(json.dumps({'status':'verified_native_linux_mesh_refinement_admission','returncode':0,'tests':sum(seen.values()),'suites':seen,'snapshot_files':len(m['file_hashes']),'current_snapshot_files_verified':len(m['refreshed_current_files'])-len(changed),'current_source_drift_after_snapshot':changed,'pins':{str(p):digest(p) for p in paths},'scope':'Native Linux aarch64 raster-only tests: rank, target triangulation, source diagonal proposals, piecewise conformity, atlas, mesh rank, translation refinement and original-preserving landmark admission. All snapshot files intact and current dedup files match. Any core source drift after snapshot is explicitly reported; modified live core bytes are not qualified by this frozen run. PDF/decode dependencies are outside the raster graph. No Linux decode/real corpus, Windows or full crate suite claim.'},indent=2)+'\n');print('verified native Linux tests',sum(seen.values()))
