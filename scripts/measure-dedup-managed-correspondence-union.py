"""Native managed union on all raw recipe correspondences; preserve known recovery."""
import hashlib,json,subprocess
from pathlib import Path
root=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays');exe=root/'gradient-scales-probe-managed-correspondence-union';folder=root/'original-resolution-hard-crops';out=Path('docs/research/dedup-managed-correspondence-union-200301.json');assert not out.exists()
reports=[Path('docs/research/dedup-original-lowcontrast-200301.json'),Path('docs/research/dedup-original-smooth-candidates-200301.json')];baseline=Path('docs/research/dedup-original-correspondence-union-200301.json');h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
for p in [*reports,baseline]:
 r=json.loads(p.read_text());assert r['status']=='complete_native' and all(h(k)==v for k,v in r['input_hashes'].items())
rows=[tuple(v for point in e for v in point) for p in reports for e in json.loads(p.read_text())['evidence']['correspondences']]
points=Path('docs/research/dedup-managed-correspondence-union-raw-200301.tsv');assert not points.exists();points.write_text(''.join(' '.join(repr(v) for v in row)+'\n' for row in rows));points.chmod(0o444)
pins={str(p):h(p) for p in [exe,points,Path(__file__),*reports,baseline,folder/'200300.jpg',folder/'200301.jpg']}
r={'status':'running_native','input_hashes':pins,'raw_recipe_matches':len(rows),'scope':'Native managed distinct-location union on every raw correspondence from two pinned recipes; unchanged model/original-pixel regional confirmation. Exact parity to prior diagnostic excluding peak, which now includes retained union credit. Geometry construction remains plain/unmanaged; no fresh extraction file/collection integration or negative precision.'}
def save():
 t=out.with_suffix('.tmp');t.write_text(json.dumps(r,indent=2)+'\n');t.replace(out)
save();v=subprocess.run([str(exe),'original-correspondence-union',str(points),str(folder)],capture_output=True,text=True);r.update(returncode=v.returncode,stdout=v.stdout,stderr=v.stderr);save();assert all(h(k)==v for k,v in pins.items())
if v.returncode:r['status']='native_failed';save();raise SystemExit(v.returncode)
e=json.loads(v.stdout);prior=json.loads(baseline.read_text())['evidence'];keys=['status','correspondences','matrix','inliers','regions','region_support_count','managed_used'];assert {k:e[k] for k in keys}=={k:prior[k] for k in keys};assert e['managed_used']==0 and e['managed_peak']<=512*1024*1024;r.update(status='verified_native_exact_recipe_parity',evidence=e);save()
