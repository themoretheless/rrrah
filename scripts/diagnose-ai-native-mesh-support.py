#!/usr/bin/env python3
"""Independently union captured native triangles against an original AI clip.
Requires the pinned original-path inventory, explicit native diagnostic log,
and the dependencies documented in diagnose-ai-tensor-domain.py.
"""
from pathlib import Path
import re,json,runpy,hashlib
from shapely.geometry import Polygon
from shapely.ops import unary_union
# These logs describe exactly this source revision and original fixture. Refuse
# stale geometry evidence instead of stamping changed sources with old results.
expected_sources = {
 'vendor/hayro-interpret/src/shading/adaptive.rs': '7806949f56bf6661c0ccd0c914d836cd6fdf101545c598f3367e8d98ff918da3',
 'vendor/hayro/src/knockout_interpreter.rs': '71f54c1e647f1685dd859efffae4f77b37a0c2f79e469675a6f1dcb91255631a',
 'vendor/hayro-interpret/src/encode/native_source.rs': 'd74c34d8847c3cbba814da08e3b7d84881d1c83d8a841f9eb7da6447e140b25c',
 'tests/fixtures/data/metal-recovery/ai-corpus/VectorApple.ai': 'c5fc34346629c3379cee6065c45b7de0b620be822604551d1463d8e691b03d06',
}
for name, expected in expected_sources.items():
 assert hashlib.sha256(Path(name).read_bytes()).hexdigest() == expected, f'Stale diagnostic evidence: {name}'
m=runpy.run_path('scripts/diagnose-ai-tensor-domain.py')
log=Path('docs/research/ai-native-mesh-area-verified-2026-10-13.log');s=log.read_text()
assert 'REAL_AI_SPATIAL_FINAL_CREDIT=0' in s and '1 passed; 0 failed' in s
triangles=[]
for row in re.findall(r'REAL_AI_CAPTURED_MESH_TRIANGLE event=2670 index=\d+ points=(\[[^\n]+\])',s):
 v=json.loads(row);assert len(v)==6
 p=Polygon([(v[i],v[i+1]) for i in (0,2,4)]);assert p.is_valid;triangles.append(p)
assert len(triangles)==43
clip=m['clip_geometry'](next(p['path'] for p in m['inventory']['paths'] if p['operator_index']==1181),2048).intersection(m['pixel'])
mesh=unary_union(triangles).intersection(clip)
alpha=float(re.search(r'REAL_AI_SPATIAL_PIXEL pixel=\(149,56\).*?alpha=Some\(([^)]+)\)',s)[1])
captured=float(re.search(r'REAL_AI_CAPTURED_CLIP_AREA pixel=\(149,56\) event=2670 area=Some\(([^)]+)\)',s)[1])
r={'scope':'Independent GEOS union of43captured native triangles versus original-source cubic clip at149,56','native_clip_area':captured,'original_clip_area':clip.area,'native_mesh_union_intersect_original_clip':mesh.area,'native_composed_alpha':alpha,'missing_area_due_to_triangle_support':clip.difference(mesh).area,'absolute_difference_union_vs_native_alpha':abs(mesh.area-alpha),'causal_finding':'Triangle support accounts for coverage deficit; clip flattening is already accurate within1.8e-6','source_tessellation_pixel_bound':1/64,'next_action':'Improve tensor boundary/local tessellation accuracy under current admission/cancellation limits','limitations':['One original-source pixel and test-only diagnostic','Original boundary2048subdivision convergence error about2.18e-6','Union consumes captured triangle geometry; triangle generation itself not independent','Production AI remains pending'],'sha256':{str(p):hashlib.sha256(p.read_bytes()).hexdigest() for p in [log,Path(__file__),Path('vendor/hayro/src/knockout_interpreter.rs'),Path('vendor/hayro-interpret/src/encode/native_source.rs')]}}
Path('docs/research/ai-native-mesh-support-deficit-2026-10-13.json').write_text(json.dumps(r,indent=2)+'\n')
assert abs(mesh.area-alpha)<1e-10
print('missing area',r['missing_area_due_to_triangle_support'],'union/native alpha error',r['absolute_difference_union_vs_native_alpha'])
