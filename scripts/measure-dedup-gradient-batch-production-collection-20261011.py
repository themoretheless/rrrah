"""Frozen production four-lane matcher: whole rephoto and exact full three-unique-photo collection parity."""
import hashlib,json,os,re,shutil,subprocess,time
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1];D=ROOT/'docs/research';T=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays')
OUT=D/'dedup-gradient-batch-production-collection-20261011.json';assert not OUT.exists()
MANIFEST=D/'dedup-gradient-batch-production-snapshot-20261011.json';RUNTIME=D/'dedup-loftr-native-20261010-runtime-pins.json'
manifest=json.loads(MANIFEST.read_text());runtime=json.loads(RUNTIME.read_text())['pins'];sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
def verify():
    assert all(sha(Path(manifest['snapshot'])/p)==h for p,h in manifest['file_hashes'].items())
    assert all(sha(p)==h for p,h in runtime.items())
verify()
binaries={}
for name in ['tile_model_files','tile_collection_files']:
    out=T/(name+'-gradient-batch-production-qualified-20261011');assert not out.exists();shutil.copy2(ROOT/'target/debug'/name,out);binaries[name]=out
w=T/'learned-matching-models/loftr-outdoor-native-20261010.safetensors';a=T/'original-resolution-negative-200201/200300.jpg';b=T/'original-resolution-strong-all/200302.jpg';f=T/'original-resolution-negative-200201/201400.jpg';l=T/'original-resolution-negative-200201/209400.jpg';r=T/'original-resolution-strong-all/209401.jpg'
fixture=Path(manifest['snapshot'])/'crates/rrrah-dedup/tests/fixtures/rotation'
mirror_paths=[fixture/'base.png',fixture/'mirrored.png']
pins={str(p):sha(p) for p in [*binaries.values(),w,a,b,f,l,r,MANIFEST,RUNTIME,Path(__file__),D/'dedup-gradient-batch-production-native-build-20261011.log',D/'dedup-model-tiles-native-20261011.json',D/'dedup-position-extent-collection-20261011.json',D/'dedup-portfolio-collection-sticky-measurement-20261010.json',*mirror_paths]}
report={'status':'running','scope':__doc__,'pins':pins,'producer_pid':os.getpid(),'results':[]}
def save():
    tmp=OUT.with_suffix('.tmp');tmp.write_text(json.dumps(report,indent=2)+'\n');tmp.replace(OUT)
save()
try:
    prior=json.loads((D/'dedup-model-tiles-native-20261011.json').read_text())
    cases=[('whole-rephoto',[str(binaries['tile_model_files']),str(l),str(r),str(w),'640','5.157305482252365']),('native-mirror-pair',[str(binaries['tile_collection_files']),str(w),'320','2','native-first',*map(str,mirror_paths)]),('three-unique-photo-collection',[str(binaries['tile_collection_files']),str(w),'640','5.12','model-first',str(a),str(b),str(f)])]
    for name,args in cases:
        row={'case':name,'status':'running','args':args};report['results'].append(row);save()
        command=['/usr/bin/time','-l','/usr/bin/env','DYLD_LIBRARY_PATH=/Users/themoretheless/.codex/tmp/rrrah-loftr-native-runtime-20261010/lib/python3.12/site-packages/torch/lib',*args]
        started=time.monotonic();p=subprocess.Popen(command,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True);row['process_pid']=p.pid;save()
        stdout,stderr=p.communicate();row.update(returncode=p.returncode,stdout=stdout,stderr=stderr,elapsed_seconds=time.monotonic()-started);save();assert p.returncode==0,stderr
        e=json.loads(stdout);row['evidence']=e;row['process_peak_rss_bytes']=int(re.search(r'(\d+)\s+maximum resident set size',stderr)[1]);assert e['status']=='ok' and e['managed_used']==0 and e['managed_peak']<=512*1024**2
        if name=='whole-rephoto':
            old=next(x['evidence'] for x in prior['results'] if x['case']=='rephoto-whole')
            assert e['supported'] and e['attempts']==old['attempts'] and e['model_digest']==old['model_digest'];row['exact_whole_proof_parity']=True
        elif name=='native-mirror-pair':
            assert e['files']==[0,1] and e['source_paths']==list(map(str,mirror_paths))
            assert len(e['pairs'])==1
            pair=e['pairs'][0]
            assert pair['attempted_stages']==1 and pair['first_miss'] is None
            assert pair['evidence']['terminal']=='native' and pair['evidence']['supported']
            legacy=json.loads((D/'dedup-portfolio-collection-sticky-measurement-20261010.json').read_text())
            old=next(x for x in legacy['results'] if x['case']=='rotation-controls')['evidence']['pairs'][0]['evidence']
            assert pair['evidence']==old
            row['exact_native_mirror_proof_parity']=True
        else:
            assert e['files']==[0,1,2] and e['source_paths']==[str(a),str(b),str(f)]
            pairs=e['pairs'];assert [(x['left'],x['right']) for x in pairs]==[(0,1),(0,2),(1,2)]
            assert [x['evidence']['supported'] for x in pairs]==[True,False,False]
            first=pairs[0];assert first['attempted_stages']==1 and first['first_miss'] is None and first['evidence']['terminal']=='model_tiles'
            old=next(x['evidence'] for x in prior['results'] if x['case']=='crumpled-automatic')
            flat=[]
            for attempt in first['evidence']['attempts']:
                proof=dict(attempt['evidence']);assert proof.pop('model_digest')==old['model_digest']
                flat.append({'left_rectangle':attempt['left_rectangle'],'right_rectangle':attempt['right_rectangle'],**proof})
            assert flat==old['attempts'];row['exact_crumpled_all_attempt_proof_parity']=True
            for pair in pairs[1:]:
                assert pair['attempted_stages']==2 and pair['first_miss']['stage']=='model_tiles' and pair['first_miss']['attempts']==25
                assert pair['evidence']['terminal']=='native' and pair['evidence']['native_recipes']==6
            assert e['managed_retained_before_drop']>0
            old_collection=json.loads((D/'dedup-position-extent-collection-20261011.json').read_text())
            old_pairs=next(x['evidence']['pairs'] for x in old_collection['results'] if x['case']=='three-unique-photo-collection')
            assert pairs==old_pairs
            row['exact_all_pair_evidence_parity']=True
        row['status']='complete';save();print(name,row['elapsed_seconds'],row['process_peak_rss_bytes'],flush=True)
    verify();assert all(sha(p)==h for p,h in pins.items());report['status']='complete_four_lane_native_mirror_whole_rephoto_and_exact_three_pair_collection'
except Exception as error:report.update(status='failed',error=str(error));raise
finally:save()
