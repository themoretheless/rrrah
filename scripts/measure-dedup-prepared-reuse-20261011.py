"""Qualify guarded prepared-input reuse against all prior attempts, plus final cancellation and weight mutation."""
import hashlib,json,os,re,shutil,subprocess,time
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1];D=ROOT/'docs/research'
T=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays')
OUT=D/'dedup-prepared-reuse-native-20261011.json'
MANIFEST=D/'dedup-prepared-reuse-snapshot-20261011.json'
RUNTIME=D/'dedup-loftr-native-20261010-runtime-pins.json'
BINARY=T/'loftr-prepared-reuse-qualified-20261011'
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
manifest=json.loads(MANIFEST.read_text());runtime=json.loads(RUNTIME.read_text())['pins']
def verify():
    assert all(sha(Path(manifest['snapshot'])/p)==h for p,h in manifest['file_hashes'].items())
    assert all(sha(p)==h for p,h in runtime.items())
verify();assert not OUT.exists() and not BINARY.exists()
shutil.copy2(ROOT/'target/debug/tile_reuse_files',BINARY)
w=T/'learned-matching-models/loftr-outdoor-native-20261010.safetensors'
a=T/'original-resolution-negative-200201/200300.jpg';b=T/'original-resolution-strong-all/200302.jpg'
f=T/'original-resolution-negative-200201/201400.jpg'
l=T/'original-resolution-negative-200201/209400.jpg';r=T/'original-resolution-strong-all/209401.jpg'
cases=[('crumpled-automatic',a,b,'5.12',True,11),('foreign-automatic',a,f,'1.6',False,25),('rephoto-whole',l,r,'5.157305482252365',True,1)]
pins={str(p):sha(p) for p in [BINARY,w,MANIFEST,RUNTIME,Path(__file__),a,b,f,l,r,D/'dedup-prepared-reuse-native-build-20261011.log',D/'dedup-loftr-regions-frozen-20261010.json',D/'dedup-model-tiles-native-20261011.json']}
report={'status':'running','scope':__doc__,'pins':pins,'producer_pid':os.getpid(),'results':[]}
def save():
    tmp=OUT.with_suffix('.tmp');tmp.write_text(json.dumps(report,indent=2)+'\n');tmp.replace(OUT)
save()
try:
    prior=json.loads((D/'dedup-loftr-regions-frozen-20261010.json').read_text())
    baseline=json.loads((D/'dedup-model-tiles-native-20261011.json').read_text())
    for name,source,target,tol,supported,count in cases:
        row={'case':name,'source':str(source),'target':str(target),'status':'running'};report['results'].append(row);save()
        args=['/usr/bin/time','-l','/usr/bin/env','DYLD_LIBRARY_PATH=/Users/themoretheless/.codex/tmp/rrrah-loftr-native-runtime-20261010/lib/python3.12/site-packages/torch/lib',str(BINARY),str(source),str(target),str(w),'640',tol]
        start=time.monotonic();p=subprocess.Popen(args,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True);row['process_pid']=p.pid;save()
        stdout,stderr=p.communicate();row.update(returncode=p.returncode,stdout=stdout,stderr=stderr,elapsed_seconds=time.monotonic()-start);save()
        assert p.returncode==0,stderr
        e=json.loads(stdout);row['evidence']=e;row['process_peak_rss_bytes']=int(re.search(r'(\d+)\s+maximum resident set size',stderr)[1])
        assert e['supported']==supported and len(e['attempts'])==count
        assert e['managed_used']==0 and e['managed_peak']<=512*1024**2
        assert all(not x['supported'] for x in e['attempts'][:-1])
        if supported:
            before=next(x['evidence'] for x in prior['results'] if x['case']==('crumpled-source-crop' if name=='crumpled-automatic' else 'full-rephoto-parity'))
            last=e['attempts'][-1]
            keys=['supported','raw_proposals','selected_points','geometry_inliers','hypotheses','transform','pixels']
            row['exact_prior_terminal_proof_parity']=all(last[k]==before[k] for k in keys)
            assert row['exact_prior_terminal_proof_parity']
            assert last['pixels']['supported'] and min(last['pixels']['anchors'])>=10
        old=next(x for x in baseline['results'] if x['case']==name)
        row['exact_all_attempts_parity']=e['attempts']==old['evidence']['attempts'] and e['model_digest']==old['evidence']['model_digest']
        assert row['exact_all_attempts_parity']
        row['prior_elapsed_seconds']=old['elapsed_seconds']
        row['status']='complete';save();print(name,supported,count,row['elapsed_seconds'],flush=True)
    stop=report['results'][0]['evidence']['callback_calls']-1
    report['controls']=[]
    for action in ['cancel','mutate']:
        guard=T/'loftr-prepared-reuse-20261011.guard.safetensors'
        weight=w
        if action=='mutate':
            assert not guard.exists();shutil.copy2(w,guard);weight=guard
        row={'action':action,'checkpoint':stop,'weights':str(weight),'before_sha256':sha(weight),'status':'running'}
        report['controls'].append(row);save()
        args=['/usr/bin/time','-l','/usr/bin/env','DYLD_LIBRARY_PATH=/Users/themoretheless/.codex/tmp/rrrah-loftr-native-runtime-20261010/lib/python3.12/site-packages/torch/lib',str(BINARY),str(a),str(b),str(weight),'640','5.12',action,str(stop)]
        started=time.monotonic();p=subprocess.Popen(args,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True);row['process_pid']=p.pid;save()
        stdout,stderr=p.communicate();row.update(returncode=p.returncode,stdout=stdout,stderr=stderr,elapsed_seconds=time.monotonic()-started,after_sha256=sha(weight));save()
        assert p.returncode==0,stderr
        e=json.loads(stdout);row['evidence']=e
        assert e['status']=='refused' and e['managed_used']==0 and e['callback_calls']>=stop
        assert ('Cancelled' if action=='cancel' else 'Changed') in e['error']
        assert (row['before_sha256']!=row['after_sha256'])==(action=='mutate')
        row['status']='complete';save();print(action,e['error'],flush=True)
    verify();assert all(sha(p)==h for p,h in pins.items())
    report['status']='complete_prepared_reuse_parity_and_two_lifecycle_controls'
except Exception as error:
    report.update(status='failed',error=str(error));raise
finally:save()
