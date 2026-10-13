"""Sequential same-input CPU timing controls, identical counter-free CLI; shared-host observations only."""
import hashlib,json,os,re,shutil,subprocess,time
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1];D=ROOT/'docs/research';T=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays')
OUT=D/'dedup-model-reuse-timing-20261011.json';assert not OUT.exists()
NEW=T/'loftr-model-reuse-timing-qualified-20261011';assert not NEW.exists();shutil.copy2(ROOT/'target/debug/tile_model_files',NEW)
OLD=T/'loftr-model-tiles-qualified-20261011'
w=T/'learned-matching-models/loftr-outdoor-native-20261010.safetensors';a=T/'original-resolution-negative-200201/200300.jpg';b=T/'original-resolution-negative-200201/201400.jpg'
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
prior=json.loads((D/'dedup-model-tiles-native-20261011.json').read_text());expected=next(x['evidence'] for x in prior['results'] if x['case']=='foreign-automatic')
assert sha(OLD)==prior['pins'][str(OLD)]
manifest=json.loads((D/'dedup-model-reuse-fixed-snapshot-20261011.json').read_text())
runtime=json.loads((D/'dedup-loftr-native-20261010-runtime-pins.json').read_text())['pins']
def verify():
    assert all(sha(Path(manifest['snapshot'])/p)==h for p,h in manifest['file_hashes'].items())
    assert all(sha(p)==h for p,h in runtime.items())
verify()
pins={str(p):sha(p) for p in [OLD,NEW,w,a,b,Path(__file__),D/'dedup-model-reuse-fixed-snapshot-20261011.json',D/'dedup-model-tiles-snapshot-20261011.json',D/'dedup-model-reuse-timing-build-20261011.log']}
r={'status':'running','scope':__doc__,'pins':pins,'results':[],'producer_pid':os.getpid()}
def save():
    tmp=OUT.with_suffix('.tmp');tmp.write_text(json.dumps(r,indent=2)+'\n');tmp.replace(OUT)
save()
try:
    # Alternate order to expose drift instead of comparing one historical run.
    for index,(name,binary) in enumerate([('fresh',OLD),('reuse',NEW),('reuse',NEW),('fresh',OLD)]):
        row={'index':index,'mode':name,'status':'running'};r['results'].append(row);save()
        args=['/usr/bin/time','-l','/usr/bin/env','DYLD_LIBRARY_PATH=/Users/themoretheless/.codex/tmp/rrrah-loftr-native-runtime-20261010/lib/python3.12/site-packages/torch/lib',str(binary),str(a),str(b),str(w),'640','1.6']
        started=time.monotonic();p=subprocess.Popen(args,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True);row['process_pid']=p.pid;save()
        stdout,stderr=p.communicate();row.update(returncode=p.returncode,stdout=stdout,stderr=stderr,elapsed_seconds=time.monotonic()-started);save()
        assert p.returncode==0,stderr
        e=json.loads(stdout);row['process_peak_rss_bytes']=int(re.search(r'(\d+)\s+maximum resident set size',stderr)[1])
        assert e['attempts']==expected['attempts'] and e['model_digest']==expected['model_digest'] and not e['supported'] and e['managed_used']==0
        row['exact_all_25_attempts_parity']=True;row['status']='complete';save();print(name,row['elapsed_seconds'],row['process_peak_rss_bytes'],flush=True)
    verify();assert all(sha(p)==h for p,h in pins.items())
    r['status']='complete_four_alternating_timing_controls'
except Exception as e:r.update(status='failed',error=str(e));raise
finally:save()
