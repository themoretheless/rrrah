"""Native least-squares local translations anchored to the frozen global model."""
import hashlib,json,subprocess,shutil,math
from pathlib import Path
b=Path('docs/research');root=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays')
h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
exe=root/'regional-translation-probe-screen';assert not exe.exists();shutil.copy2('target/debug/examples/regional_translation_probe',exe)
baseline=b/'dedup-screen-filter8.json';e=json.loads(baseline.read_text())['evidence'];matrix=b/'dedup-screen-global-translation-anchor.txt';assert not matrix.exists();matrix.write_text(' '.join(str(v) for row in e['matrix'] for v in row)+'\n')
points=b/'dedup-screen-inlier-regional-points.txt';previous=b/'dedup-screen-inlier-regional-geometry.json';prev=json.loads(previous.read_text())
pins={str(p):h(p) for p in [exe,matrix,points,baseline,previous,Path(__file__),Path('crates/rrrah-dedup/examples/regional_translation_probe.rs')]};assert all(h(k)==v for k,v in prev['input_hashes'].items())
v=subprocess.run([str(exe),str(points),str(matrix),'640','480'],capture_output=True,text=True);assert v.returncode==0 and all(h(k)==v for k,v in pins.items());native=json.loads(v.stdout);assert native['status']=='ok' and len(native['regions'])==4
for r in native['regions']:
 assert r['matrix'] is not None and len(r['inliers'])>=10
 delta=r['translation'];assert math.hypot(*delta)<=2+1e-8
 for i in range(3):assert r['matrix'][2][i]==e['matrix'][2][i]
 for i in range(2):
  for j in range(3):assert abs(r['matrix'][i][j]-(e['matrix'][i][j]+delta[i]*e['matrix'][2][j]))<1e-10
out=b/'dedup-screen-regional-translations.json';assert not out.exists();out.write_text(json.dumps({'status':'complete_native_experiment','input_hashes':pins,'returncode':v.returncode,'stdout':v.stdout,'stderr':v.stderr,'evidence':native,'global_point_indices':prev['global_point_indices'],'scope':'Four least-squares local target translations from global inliers; original global homography shape retained. Native fit plus independent coefficient composition, bound2 and minimum10 checks. Pixel confirmation pending.'},indent=2)+'\n')
models=b/'dedup-screen-regional-translations-models-input.txt';assert not models.exists();models.write_text(''.join(' '.join(str(v) for v in [*r['target_domain'],*[v for row in r['matrix'] for v in row]])+'\n' for r in native['regions']))
print(json.dumps([{'domain':r['target_domain'],'delta':r['translation'],'inliers':len(r['inliers'])} for r in native['regions']]))
