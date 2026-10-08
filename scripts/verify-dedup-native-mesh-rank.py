"""Exact native/offline count parity on all six whole-domain comparisons."""
import json,hashlib
from pathlib import Path
root=Path(__file__).resolve().parents[1];base=root/'docs/research'
def digest(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
reference=base/'dedup-folded-piecewise-pixel-diagnostic.json';native=base/'dedup-folded-native-mesh-rank.jsonl'
r=json.loads(reference.read_text());assert r['status']=='complete_offline_piecewise_pixel_diagnostic';assert all(digest(k)==v for k,v in r['pins'].items())
rows=[json.loads(v) for v in native.read_text().splitlines()];assert len(rows)==6
seen=set();summary=[]
for row in rows:
 key=(row['direction'],row['filter_radius']);assert key not in seen;seen.add(key)
 expected=next(v for v in r['rows'] if v['case']=='fold_'+row['direction'] and v['radius']==row['filter_radius'])
 assert row['sites']==expected['domain_sites'] and row['covered']==expected['mesh_covered_sites']
 for field in ['valid_sites','informative_pairs','agreeing_pairs']:assert row[field]==expected[field],(key,field,row[field],expected[field])
 assert row['pixel_reads']==5*row['sites'];summary.append({'direction':row['direction'],'filter_radius':row['filter_radius'],'valid_sites':row['valid_sites'],'informative_pairs':row['informative_pairs'],'agreeing_pairs':row['agreeing_pairs'],'agreement_fraction':row['agreeing_pairs']/row['informative_pairs']})
assert seen=={(d,r) for d in ['forward','reverse'] for r in [0,3,8]}
paths=[reference,native,Path(__file__),root/'crates/rrrah-dedup/src/mesh_rank.rs',root/'crates/rrrah-dedup/src/mesh_grid.rs',root/'crates/rrrah-dedup/examples/mesh_rank_probe.rs',root/'crates/rrrah-dedup/src/rank_region.rs',root/'crates/rrrah-dedup/src/triangulation.rs',root/'crates/rrrah-dedup/src/piecewise_warp.rs',root/'crates/rrrah-dedup/src/lib.rs']
manifest=base/'dedup-rank-normalized-pixels.json';dump=json.loads(manifest.read_text());paths.append(manifest)
for stem in ['201700','201702']:
 row=next(v for v in dump['rows'] if Path(v['source']).stem==stem);assert digest(row['source'])==row['source_sha256'];paths.append(Path(row['source']))
paths.append(Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays/mesh-rank-probe-qualified'))
(base/'dedup-folded-native-mesh-rank-audit.json').write_text(json.dumps({'status':'verified_native_independent_mesh_rank_count_parity','comparisons':6,'summary':summary,'pins':{str(p):digest(p) for p in paths},'scope':'All native sites/coverage/valid/informative/agreement counts match offline independent whole-mesh math. Decoder shared with pixel dump oracle. Fixed156 correspondence union; no integrated retrieval, full negative precision or copy admission.'},indent=2)+'\n')
print(json.dumps(summary,indent=2))
