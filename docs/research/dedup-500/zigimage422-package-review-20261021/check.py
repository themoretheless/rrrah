import json,re
from pathlib import Path
root=Path('.dedup-work/zigimage422-20261021')
zon=(root/'build.zig.zon').read_text();paths=re.findall(r'"([^"]+)"',zon.split('.paths = .{',1)[1].split('}',1)[0]);tree=json.loads(Path('docs/research/dedup-500/evidence/galactixx--imagehash/tree.json').read_text());blobs=[x['path'] for x in tree['tree'] if x['type']=='blob'];included=lambda n:any(n==p or n.startswith(p+'/') for p in paths)
required=['c_src/stb_impl.c','include/stb_image.h','include/stb_image_resize.h'];excluded=[n for n in required if not included(n)];assert excluded==required
fixtures=['testdata/checkerboard.png','testdata/gradient.png','testdata/noise.png'];assert all(not included(n) for n in fixtures)
source=(root/'src/imagehash.zig').read_text();assert 'for (1..4)' in source
out=dict(package_paths=paths,included_tracked_files=[n for n in blobs if included(n)],excluded_required_native_files=excluded,excluded_test_fixtures=fixtures,readme_wavelet_levels=4,implementation_wavelet_levels=list(range(1,4)),scope='Structural membership check of pinned manifest and build/source references. No Zig fetch/build/package extraction or C header semantic audit performed.')
Path(__file__).with_name('result.json').write_text(json.dumps(out,indent=2)+'\n');print(json.dumps(out,indent=2))
