"""Finite whole-grid optimized binary parity on one positive and two negatives."""
import json,hashlib,subprocess,shutil,time
from pathlib import Path
root=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays');base=Path('docs/research');h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();exe=root/'affine-color-grid-image-probe-screen-meanfix-release';assert not exe.exists();shutil.copy2('target/release/examples/affine_color_grid_image_probe',exe);positive=base/'dedup-native-screen-affine-color-grid-meanfix.json';pos=json.loads(positive.read_text());assert pos['status']=='terminal' and all(h(k)==v for k,v in pos['input_hashes'].items());live=base/'dedup-native-screen-affine-color-grid-negatives-meanfix.json';negative=json.loads(live.read_text());selected=[v for v in negative['rows'] if Path(v['source']).name in ['200000.jpg','201900.jpg']];assert len(selected)==2;snapshot=base/'dedup-affine-grid-release-negative-reference.json';assert not snapshot.exists();snapshot.write_text(json.dumps({'rows':selected},indent=2)+'\n');pins={str(p):h(p) for p in [exe,positive,snapshot,Path(__file__)]};cases=[]
left=root/'original-resolution-negative-200201/200500.jpg';right=root/'original-resolution-strong-all/200501.jpg';anchor=base/'dedup-screen-global-translation-anchor.txt';cases.append((left,right,anchor,pos['evidence']))
for row in selected:
 left=Path(row['source']);input=Path(row['input']);assert h(left)==row['source_sha256'] and h(input)==row['input_sha256'];assert row['returncode']==0 and row['evidence']['status']=='ok';cases.append((left,right,input,row['evidence']))
for left,right,input,_ in cases:
 for path in [left,right,input]:pins[str(path)]=h(path)
out=base/'dedup-affine-grid-release-parity.json';assert not out.exists();report={'status':'running','input_hashes':pins,'required_cases':3,'rows':[],'scope':'Finite exact semantic parity of all automatic grid rows on one positive/two unrelated originals. Not full corpus, independent resampling or measured paired speedup.'}
def save():
 tmp=out.with_suffix('.tmp');tmp.write_text(json.dumps(report,indent=2)+'\n');tmp.replace(out)
save()
for left,right,input,expected in cases:
 start=time.monotonic();v=subprocess.run([str(exe),str(left),str(right),str(input)],capture_output=True,text=True);elapsed=time.monotonic()-start;assert all(h(k)==v for k,v in pins.items());row={'source':str(left),'returncode':v.returncode,'stdout':v.stdout,'stderr':v.stderr,'elapsed_seconds':elapsed};report['rows'].append(row);save();assert v.returncode==0;e=json.loads(v.stdout);assert e['status']==expected['status'] and e['regions']==expected['regions'] and e['managed_used']==expected['managed_used']==0 and 0<=e['managed_peak']<=512*1024*1024;row.update(evidence=e,semantic_parity=True);save();print(left.name,elapsed,flush=True)
report['status']='verified_three_case_semantic_parity';save()
