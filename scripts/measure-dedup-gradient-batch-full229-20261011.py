"""Execute all 229 publisher-origin positive pairs on the frozen production tiled/native API."""
import hashlib,json,math,os,re,subprocess,time
from pathlib import Path
from dedup_jpeg_domain import oriented_dimensions
ROOT=Path(__file__).resolve().parents[1];D=ROOT/'docs/research';T=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays')
OUT=D/'dedup-gradient-batch-full229-20261011.json';assert not OUT.exists()
MP=D/'dedup-gradient-batch-production-snapshot-20261011.json';m=json.loads(MP.read_text());RP=D/'dedup-loftr-native-20261010-runtime-pins.json';runtime=json.loads(RP.read_text())['pins']
BINARY=T/'tile_collection_files-gradient-batch-production-qualified-20261011';WEIGHTS=T/'learned-matching-models/loftr-outdoor-native-20261010.safetensors';PREPARED=T/'prepared.json';corpus=json.loads(PREPARED.read_text())
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
def frozen_verify():
 assert all(sha(Path(m['snapshot'])/p)==h for p,h in m['file_hashes'].items())
 assert all(sha(p)==h for p,h in runtime.items())
frozen_verify()
assert corpus['original_images']==157 and corpus['strong_images']==229 and corpus['required_positive_pairs']==229
pairs=corpus['positive_pairs'];assert len(pairs)==229 and len({x['query_id'] for x in pairs})==229
# Prior hard cases first, without omitting or repeating any pair.
order=sorted(pairs,key=lambda x:({'200302.jpg':0,'209401.jpg':1}.get(x['query_id'],2),x['query_id']))
images={};domains={}
for kind,folder in [('original','original-resolution-negative-200201'),('strong','original-resolution-strong-all')]:
 for row in corpus['images'][kind]:
  path=T/folder/row['filename'];assert sha(path)==row['source_sha256'];assert path.stat().st_size<=5_242_880
  dims=oriented_dimensions(path,row['source_size']);assert dims[0]*dims[1]<=6_400_000
  images[(kind,row['filename'])]=path;domains[str(path)]=dims
assert len(images)==386
files=[BINARY,WEIGHTS,PREPARED,MP,RP,Path(__file__),ROOT/'scripts/dedup_jpeg_domain.py',D/'dedup-gradient-batch-production-native-build-20261011.log',D/'dedup-gradient-batch-production-prefix-audit-20261011.json',*images.values()]
pins={str(p):sha(p) for p in files}
report={'status':'running','scope':__doc__,'producer_pid':os.getpid(),'required_pairs':229,'source_images':386,'pair_order':[x['query_id'] for x in order],'pins':pins,'results':[],'limits':'Known publisher-origin copies, not whole-image identity or independently annotated local masks. Each case is a fresh two-file ModelFirst collection, all 25 model regions and all six native recipes available, no pruning. Does not qualify all-query negatives, one 386-file indexed collection, all formats or hard process RSS.'}
def save():
 temp=OUT.with_suffix('.tmp');temp.write_text(json.dumps(report,indent=2)+'\n');temp.replace(OUT)
save()
try:
 for item in order:
  assert item['label']=='publisher_origin_copy' and item['left']['group_id']==item['right']['group_id']
  left=images[('original',item['left']['filename'])];right=images[('strong',item['right']['filename'])]
  source_tol=2*math.hypot(*domains[str(left)])/math.hypot(*domains[str(right)])
  guards=[BINARY,WEIGHTS,PREPARED,MP,RP,left,right]
  assert all(sha(p)==pins[str(p)] for p in guards)
  row={'query':item['query_id'],'original':item['left']['filename'],'source':str(left),'target':str(right),'source_tolerance':source_tol,'status':'running'};report['results'].append(row);save()
  command=['/usr/bin/time','-l','/usr/bin/env','DYLD_LIBRARY_PATH=/Users/themoretheless/.codex/tmp/rrrah-loftr-native-runtime-20261010/lib/python3.12/site-packages/torch/lib',str(BINARY),str(WEIGHTS),'640',str(source_tol),'model-first',str(left),str(right)]
  started=time.monotonic();p=subprocess.Popen(command,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True);row['process_pid']=p.pid;save();stdout,stderr=p.communicate();row.update(returncode=p.returncode,stdout=stdout,stderr=stderr,elapsed_seconds=time.monotonic()-started)
  assert all(sha(p)==pins[str(p)] for p in guards),'Runtime/input changed; result not trusted'
  rss=re.search(r'(\d+)\s+maximum resident set size',stderr)
  if rss:row['process_peak_rss_bytes']=int(rss[1])
  cpu=re.search(r'([0-9.]+) real\s+([0-9.]+) user\s+([0-9.]+) sys',stderr)
  if cpu:row['cpu_seconds']=float(cpu[2])+float(cpu[3])
  if p.returncode:
   row['status']='execution_failed'
  else:
   e=json.loads(stdout);row['evidence']=e
   assert e['status']=='ok' and e['files']==[0,1] and e['source_paths']==[str(left),str(right)]
   assert e['managed_used']==0 and e['managed_peak']<=512*1024**2 and len(e['pairs'])==1
   pair=e['pairs'][0];assert (pair['left'],pair['right'])==(0,1)
   proof=pair['evidence'];row['supported']=proof['supported'];row['status']='supported' if row['supported'] else 'miss'
   if proof['terminal']=='model_tiles':
    assert 1<=len(proof['attempts'])<=25
    if proof['supported']:assert proof['attempts'][-1]['evidence']['pixels']['supported']
   else:
    assert proof['terminal']=='native' and 1<=proof['native_recipes']<=6
    if proof['supported']:assert proof['pixels']['supported']
  report['completed_pairs']=len(report['results']);report['counts']={status:sum(x['status']==status for x in report['results']) for status in ['supported','miss','execution_failed']};save()
  print(row['query'],row['status'],round(row['elapsed_seconds'],3),flush=True)
 frozen_verify();assert all(sha(p)==h for p,h in pins.items());assert len(report['results'])==229
 report['status']='complete_bounded_229_pair_execution';report['all_229_supported']=report['counts']['supported']==229
except Exception as error:
 report.update(status='failed_integrity_or_producer',error=str(error));raise
finally:save()
print(report['status'],report.get('counts'),flush=True)
