"""Known screen ROI parity through the guarded file-region public API."""
import hashlib,json,subprocess
from pathlib import Path
root=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays');base=Path('docs/research')
digest=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
exe=root/'affine-color-guarded-roi-radius24-probe';input=root/'guarded-roi-screen-control.txt'
left=root/'original-resolution-negative-200201/200500.jpg';right=root/'original-resolution-strong-all/200501.jpg'
reference=base/'dedup-native-screen-affine-color-managed.json';expected=json.loads(reference.read_text())['evidence']
paths=[exe,input,left,right,reference,Path(__file__),Path('crates/rrrah-dedup/examples/affine_color_guarded_roi_radius24_probe.rs'),*[Path('crates/rrrah-dedup/src')/name for name in ['affine_region.rs','affine_region_file.rs','region_footprint.rs','affine_color.rs','lib.rs']]]
pins={str(p):digest(p) for p in paths};output=base/'dedup-guarded-roi-radius24-screen-control.json';assert not output.exists()
report={'status':'running','input_hashes':pins,'scope':'One known ROI pair, exact count parity through guarded file API; not fresh geometry or broad coverage.'}
def save():
 temporary=output.with_suffix('.tmp');temporary.write_text(json.dumps(report,indent=2)+'\n');temporary.replace(output)
save();result=subprocess.run([str(exe),str(left),str(right),str(input)],capture_output=True,text=True)
assert all(digest(k)==v for k,v in pins.items());report.update(returncode=result.returncode,stdout=result.stdout,stderr=result.stderr);save();assert result.returncode==0
actual=json.loads(result.stdout);assert actual['status']=='ok' and actual['managed_used']==0 and len(actual['directions'])==2
for a,b in zip(actual['directions'],expected['directions']):
 for key in ['radius','training_samples','matched','pixel_reads']:assert a[key]==b[key],key
 assert a['samples']==b['heldout_samples']
report.update(status='verified_known_roi_parity',evidence=actual);save();print(actual)
