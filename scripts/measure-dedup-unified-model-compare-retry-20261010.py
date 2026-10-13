"""Real LoFTR through unified native library comparison: 2 positive/4 foreign variants.

All outcomes retained. Does not qualify total inference tensor/RSS admission,
interrupting blocking forward, broad precision/recall or collection integration.
"""
import hashlib
import json
import math
import os
import re
from pathlib import Path
import shutil
import subprocess
import time

ROOT=Path(__file__).resolve().parents[1]
D=ROOT/'docs/research'
T=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays')
OUT=D/'dedup-unified-model-compare-retry-20261010.json'
BINARY=T/'unified-model-compare-retry-qualified-20261010'
MANIFEST=D/'dedup-model-compare-snapshot-20261010.json'
WEIGHTS=T/'learned-matching-models/loftr-outdoor-native-20261010.safetensors'
TARGET=T/'original-resolution-strong-all/209401.jpg'
SOURCES=[T/'original-resolution-negative-200201'/name for name in ('209400.jpg','201400.jpg','209000.jpg')]
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
assert not OUT.exists() and not BINARY.exists()
m=json.loads(MANIFEST.read_text())
assert all(sha(Path(m['snapshot'])/p)==h for p,h in m['file_hashes'].items())
shutil.copy2('/Users/themoretheless/.codex/tmp/rrrah-loftr-native-target-20261010/release/compare_files',BINARY)
runtime_path=D/'dedup-loftr-native-20261010-runtime-pins.json'
runtime=json.loads(runtime_path.read_text())
assert all(sha(p)==h for p,h in runtime['pins'].items())
files=[BINARY,MANIFEST,WEIGHTS,TARGET,*SOURCES,Path(__file__),runtime_path,D/'dedup-model-compare-native-build-20261010.log']
pins={str(p):sha(p) for p in files}
r=dict(status='running',pins=pins,results=[],scope=__doc__)
def save():
    temp=OUT.with_suffix('.json.tmp');temp.write_text(json.dumps(r,indent=2)+'\n');temp.replace(OUT)
save()
try:
    env=dict(os.environ);env['DYLD_LIBRARY_PATH']='/Users/themoretheless/.codex/tmp/rrrah-loftr-native-runtime-20261010/lib/python3.12/site-packages/torch/lib'
    for source in SOURCES:
        for side in (320,640):
            started=time.monotonic()
            p=subprocess.run(["/usr/bin/time","-l","/usr/bin/env","DYLD_LIBRARY_PATH="+env["DYLD_LIBRARY_PATH"],str(BINARY),str(source),str(TARGET),str(WEIGHTS),str(side),str(2*math.hypot(2560,1920)/math.hypot(1024,701))],capture_output=True,text=True,env=env)
            row=dict(source=str(source),max_side=side,expected_positive=source.name=='209400.jpg',returncode=p.returncode,stdout=p.stdout,stderr=p.stderr,elapsed_seconds=time.monotonic()-started)
            r['results'].append(row);save()
            assert p.returncode==0
            row['evidence']=json.loads(p.stdout)
            peak=re.search(r'(\d+)\s+maximum resident set size',p.stderr)
            assert peak, 'macOS time -l RSS observation missing'
            row['process_peak_rss_bytes']=int(peak[1])
            assert row['evidence']['status']=='ok'
            assert row['evidence']['supported']==row['expected_positive']
            assert row['evidence']['managed_used']==0 and row['evidence']['managed_peak']<=512*1024*1024
            save();print(source.name,side,row['evidence']['supported'],row['evidence']['raw_proposals'],flush=True)
    assert all(sha(p)==h for p,h in pins.items())
    assert all(sha(Path(m['snapshot'])/p)==h for p,h in m['file_hashes'].items())
    assert all(sha(p)==h for p,h in runtime['pins'].items())
    r['status']='complete_unified_native_model_comparison_diagnostic'
except Exception as error:
    r.update(status='failed_unified_native_model_comparison_diagnostic',error=str(error));raise
finally:
    save()
