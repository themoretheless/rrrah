"""Native JPEG decode/preparation, Rust LoFTR, library selection/geometry and pixel proof.

Python orchestrates subprocesses and serializes model/point files only. Two fixed
grayscale sizes; no CLAHE, model memory/cancellation or collection qualification.
"""
import hashlib
import json
import math
import os
from pathlib import Path
import shutil
import subprocess
import time

ROOT=Path(__file__).resolve().parents[1]
D=ROOT/'docs/research'
T=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays')
WORK=T/'native-file-model-input-20261010'
OUT=D/'dedup-native-file-model-input-20261010.json'
MANIFEST=D/'dedup-model-input-snapshot-20261010.json'
m=json.loads(MANIFEST.read_text())
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
assert not OUT.exists() and not WORK.exists()
assert all(sha(Path(m['snapshot'])/p)==h for p,h in m['file_hashes'].items())
WORK.mkdir()
PREP=WORK/'model-input-qualified'
shutil.copy2(Path(m['snapshot'])/'target/release/examples/model_input_file_probe',PREP)
INFER=Path('/Users/themoretheless/.codex/tmp/rrrah-loftr-native-target-20261010/release/dedup-loftr-native-probe')
GEOMETRY=T/'native-loftr-inputs-20261010/geometry-scored-proposals-managed-qualified'
CONFIRM=T/'supplied-anchor-files-probe-qualified-20261010'
WEIGHTS=T/'learned-matching-models/loftr-outdoor-native-20261010.safetensors'
sources=[T/'original-resolution-negative-200201/209400.jpg',T/'original-resolution-strong-all/209401.jpg']
files=[PREP,INFER,GEOMETRY,CONFIRM,WEIGHTS,MANIFEST,Path(__file__),D/'dedup-model-input-native-build-20261010.log',D/'dedup-loftr-native-20261010-runtime-pins.json',D/'dedup-scored-proposals-managed-probe-source-20261010.json',*sources]
pins={str(p):sha(p) for p in files}
r=dict(status='running',pins=pins,results=[],scope=__doc__)
def save():
    temp=OUT.with_suffix('.json.tmp');temp.write_text(json.dumps(r,indent=2)+'\n');temp.replace(OUT)
def run(args):
    env=dict(os.environ)
    env['DYLD_LIBRARY_PATH']='/Users/themoretheless/.codex/tmp/rrrah-loftr-native-runtime-20261010/lib/python3.12/site-packages/torch/lib'
    p=subprocess.run(list(map(str,args)),capture_output=True,text=True,env=env)
    result=dict(returncode=p.returncode,stdout=p.stdout,stderr=p.stderr)
    if p.returncode==0:result['evidence']=json.loads(p.stdout)
    return result
save()
try:
    for side in (320,640):
        started=time.monotonic();row=dict(side=side,preparation=[]);r['results'].append(row);save()
        inputs=[];dimensions=[];original=[]
        for i,source in enumerate(sources):
            path=WORK/f'{side}-{i}.gray';prepared=run([PREP,source,side,path]);row['preparation'].append(prepared);save()
            assert prepared['returncode']==0 and prepared['evidence']['memory_used']==0
            e=prepared['evidence'];assert len(path.read_bytes())==math.prod(e['dimensions'])
            pins[str(path)]=sha(path);inputs.append(path);dimensions.extend(e['dimensions']);original.extend(e['original'])
        args=[INFER,WEIGHTS]
        for i,path in enumerate(inputs):args.extend([path,*dimensions[2*i:2*i+2]])
        inference=run(args);row['inference']=inference;save();assert inference['returncode']==0
        proposals=WORK/f'{side}-proposals.json';proposals.write_text(inference['stdout']);pins[str(proposals)]=sha(proposals)
        geometry=run([GEOMETRY,proposals,*dimensions,*original]);row['geometry']=geometry;save();assert geometry['returncode']==0
        artifact=WORK/f'{side}-geometry.json';artifact.write_text(geometry['stdout']);pins[str(artifact)]=sha(artifact)
        g=geometry['evidence']
        if g['status']=='geometry':
            model=WORK/f'{side}-model.txt';points=WORK/f'{side}-points.txt'
            model.write_text(' '.join(str(v) for row in g['matrix'] for v in row)+'\n')
            points.write_text(''.join(' '.join(map(str,[*p,*q]))+'\n' for p,q in g['points']))
            pins[str(model)]=sha(model);pins[str(points)]=sha(points)
            row['confirmation']=run([CONFIRM,*sources,model,points,2*math.hypot(*original[:2])/math.hypot(*original[2:])]);assert row['confirmation']['returncode']==0
        row['elapsed_seconds']=time.monotonic()-started;save()
        print(side,row.get('confirmation',{}).get('evidence',g['status']),flush=True)
    assert all(sha(p)==h for p,h in pins.items())
    assert all(sha(Path(m['snapshot'])/p)==h for p,h in m['file_hashes'].items())
    r['status']='complete_native_file_model_input_diagnostic'
except Exception as error:
    r.update(status='failed_native_file_model_input_diagnostic',error=str(error));raise
finally:
    save()
