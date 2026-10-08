"""Real-image library-vs-independent-diagnostic sample parity and memory release."""
import json,hashlib,subprocess,shutil
from pathlib import Path
root=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays');base=Path('docs/research');h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();exe=root/'affine-color-managed-image-probe-screen';assert not exe.exists();shutil.copy2('target/debug/examples/affine_color_managed_image_probe',exe)
reference=base/'dedup-native-screen-affine-color-footprint.json';ref=json.loads(reference.read_text());assert all(h(k)==v for k,v in ref['input_hashes'].items());left=root/'original-resolution-negative-200201/200500.jpg';right=root/'original-resolution-strong-all/200501.jpg';anchor=base/'dedup-screen-global-translation-anchor.txt'
pins={str(p):h(p) for p in [reference,left,right,anchor,exe,Path(__file__),Path('crates/rrrah-dedup/src/affine_region.rs'),Path('crates/rrrah-dedup/src/affine_color.rs'),Path('crates/rrrah-dedup/src/region_footprint.rs'),Path('crates/rrrah-dedup/src/lib.rs'),Path('crates/rrrah-dedup/examples/affine_color_managed_image_probe.rs')]}
r=subprocess.run([str(exe),str(left),str(right),str(anchor)],capture_output=True,text=True);assert all(h(k)==v for k,v in pins.items());result={'input_hashes':pins,'returncode':r.returncode,'stdout':r.stdout,'stderr':r.stderr};out=base/'dedup-native-screen-affine-color-managed.json';assert not out.exists()
if r.returncode==0:
 e=json.loads(r.stdout);assert e['managed_used']==0 and e['managed_peak']<=512*1024*1024
 for actual,expected in zip(e['directions'],ref['evidence']['directions']):
  assert actual['direction']==expected['direction'] and actual['radius']==expected['filter_radius']
  for key in ['training_samples','heldout_samples','pixel_reads']:assert actual[key]==expected[key]
  assert actual['matched']==expected['evidence']['matched']
 result.update(status='verified_native_managed_sample_parity',evidence=e)
else:result['status']='native_failure'
out.write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result));raise SystemExit(r.returncode)
