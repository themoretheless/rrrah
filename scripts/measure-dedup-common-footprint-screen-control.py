"""Fixed screen ROI control: unchanged forward and new common-frame reverse."""
import hashlib,json,shutil,subprocess
from pathlib import Path
root=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays');base=Path('docs/research')
digest=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
exe=root/'affine-color-common-footprint-roi-probe';assert not exe.exists();shutil.copy2('target/debug/examples/affine_color_common_footprint_roi_probe',exe)
input=root/'guarded-roi-screen-control.txt';left=root/'original-resolution-negative-200201/200500.jpg';right=root/'original-resolution-strong-all/200501.jpg'
reference=base/'dedup-native-screen-affine-color-managed.json';expected=json.loads(reference.read_text())['evidence']['directions'][0]
paths=[exe,input,left,right,reference,Path(__file__),Path('crates/rrrah-dedup/examples/affine_color_common_footprint_roi_probe.rs'),*[Path('crates/rrrah-dedup/src')/name for name in ['affine_region.rs','affine_region_file.rs','affine_color.rs','lib.rs']]]
pins={str(p):digest(p) for p in paths};output=base/'dedup-common-footprint-screen-control.json';assert not output.exists();report={'status':'running','input_hashes':pins,'scope':'One supplied-model real ROI control. Exact prior forward count parity, common-frame reverse measured separately; no broad precision or fold coverage.'}
def save():
 temporary=output.with_suffix('.tmp');temporary.write_text(json.dumps(report,indent=2)+'\n');temporary.replace(output)
save();result=subprocess.run([str(exe),str(left),str(right),str(input)],capture_output=True,text=True);assert all(digest(k)==v for k,v in pins.items());report.update(returncode=result.returncode,stdout=result.stdout,stderr=result.stderr);save();assert result.returncode==0
actual=json.loads(result.stdout);report.update(status='terminal',evidence=actual);save();assert actual['status']=='ok' and actual['managed_used']==0 and actual['footprints']==['target','source'] and len(actual['directions'])==2
forward=actual['directions'][0]
for key in ['radius','training_samples','matched','pixel_reads']:assert forward[key]==expected[key],key
assert forward['samples']==expected['heldout_samples']
report['bidirectional_supported']=all(v['samples']>=1000 and v['matched']>=.9*v['samples'] for v in actual['directions']);report['status']='verified_common_footprint_control';save();print(actual)
