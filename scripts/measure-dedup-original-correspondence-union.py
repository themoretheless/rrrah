"""Union of independently measured candidate recipes; unchanged geometry/pixels."""
import hashlib,json,subprocess
from pathlib import Path
root=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays');exe=root/'gradient-scales-probe-original-correspondence-union';folder=root/'original-resolution-hard-crops';out=Path('docs/research/dedup-original-correspondence-union-200301.json');assert not out.exists()
reports=[Path('docs/research/dedup-original-lowcontrast-200301.json'),Path('docs/research/dedup-original-smooth-candidates-200301.json')];audits=[Path('docs/research/dedup-original-lowcontrast-200301-audit.json'),Path('docs/research/dedup-original-smooth-candidates-200301-audit.json')];h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
for p in [*reports,*audits]:
 r=json.loads(p.read_text());assert all(h(k)==v for k,v in r['input_hashes'].items())
rows=sorted(set(tuple(v for point in e for v in point) for p in reports for e in json.loads(p.read_text())['evidence']['correspondences']))
selected=[]
for row in rows:
 if any(sum((row[k]-prior[k])**2 for k in [0,1])<=4 or sum((row[k]-prior[k])**2 for k in [2,3])<=4 for prior in selected):continue
 selected.append(row)
points=Path('docs/research/dedup-original-correspondence-union-200301.tsv');assert not points.exists();points.write_text(''.join(' '.join(repr(v) for v in row)+'\n' for row in selected));points.chmod(0o444)
pins={str(p):h(p) for p in [exe,points,Path(__file__),*reports,*audits,folder/'200300.jpg',folder/'200301.jpg']}
r={'status':'running_native','input_hashes':pins,'source_recipe_matches':[len(json.loads(p.read_text())['evidence']['correspondences']) for p in reports],'unique_union_matches':len(rows),'selected_matches':len(selected),'scope':'Deterministic exact union plus radius2 independent source/target-location selection from pinned prior native candidate recipes. Native projective minimum10 and unchanged source-pixel grid8/radius7 confirmation. Plain geometry scratch excluded; diagnostic only, no collection integration, whole decision or negative precision.'}
def save():
 t=out.with_suffix('.tmp');t.write_text(json.dumps(r,indent=2)+'\n');t.replace(out)
save();v=subprocess.run([str(exe),'original-correspondence-union',str(points),str(folder)],capture_output=True,text=True);r.update(returncode=v.returncode,stdout=v.stdout,stderr=v.stderr);save();assert all(h(k)==v for k,v in pins.items())
if v.returncode:r['status']='native_failed';save();raise SystemExit(v.returncode)
e=json.loads(v.stdout);assert e['status']=='ok' and e['managed_used']==0 and e['managed_peak']<=512*1024*1024;r.update(status='complete_native',evidence=e);save()
