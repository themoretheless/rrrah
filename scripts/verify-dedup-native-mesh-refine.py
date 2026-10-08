"""Full real native proposal parity against independent selected-score math."""
import json,hashlib
from pathlib import Path
root=Path(__file__).resolve().parents[1];base=root/'docs/research';digest=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
reference=base/'dedup-folded-dense-small-window-seed-diagnostic.json';r=json.loads(reference.read_text());assert all(digest(k)==v for k,v in r['pins'].items())
proof=base/'dedup-folded-dense-small-window-seed-audit.json';a=json.loads(proof.read_text());assert a['status']=='verified_direct_window_selected_proposal_scores' and a['rows']==1156 and a['report_sha256']==digest(reference)
input_pins=base/'dedup-native-mesh-refine-input-pins.json';pins=json.loads(input_pins.read_text());assert all(digest(k)==v for k,v in pins.items())
manifest=base/'dedup-rank-normalized-pixels.json';m=json.loads(manifest.read_text());summary=[]
for case,count in [('201700',702),('201900',454)]:
 native=base/f'dedup-native-mesh-refine-{case}.jsonl';rows=[json.loads(v) for v in native.read_text().splitlines()];expected=[v for v in r['rows'] if v['case']==case];assert len(rows)==len(expected)==count
 seeds=base/f'dedup-native-refine-seeds-{case}.txt';inputs=[list(map(int,line.split())) for line in seeds.read_text().splitlines()];assert inputs==[v['target'] for v in expected]
 for row,old in zip(rows,expected):
  assert row['target']==old['target'] and row['status']=='scored';assert row['offset']==old['offset'],(case,row,old)
  assert row['training_pairs']==old['training_pairs'] and row['validation_pairs']==old['heldout_pairs']
  assert row['training_agreeing']/row['training_pairs']==old['training_fraction'];assert row['validation_agreeing']/row['validation_pairs']==old['heldout_fraction'];assert row['eligible']==old['eligible_proposal']
 summary.append({'case':case,'seeds':count,'eligible_proposals':sum(v['eligible'] for v in rows)});pins[str(native)]=digest(native)
for stem in ['201700','201900','201702']:
 row=next(v for v in m['rows'] if Path(v['source']).stem==stem);assert digest(row['pixels'])==row['pixels_sha256'];pins[row['pixels']]=row['pixels_sha256']
for path in [reference,proof,input_pins,manifest,Path(__file__)]:pins[str(path)]=digest(path)
(base/'dedup-native-mesh-refine-parity-audit.json').write_text(json.dumps({'status':'verified_native_real_mesh_translation_parity','seeds':1156,'summary':summary,'pins':pins,'scope':'Every selected source offset, target-defined training/validation count, agreeing count and eligibility matches independently direct-window-verified offline proposal data.625 candidates/seed searched natively. Guarded normalized pixel inputs/shared decoder; no native raw decode wrapper, spatial holdout independence, full negative precision or copy admission.'},indent=2)+'\n');print(summary)
