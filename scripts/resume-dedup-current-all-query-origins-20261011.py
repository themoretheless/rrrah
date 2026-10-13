"""Execute every 229 query x 157 original comparison; no retrieval pruning.
Publisher origin labels qualify origin retrieval, not semantic or local-mask identity.
"""
import collections,gzip,hashlib,importlib.util,json,math,os,shutil,subprocess,time
from pathlib import Path
from dedup_jpeg_domain import oriented_dimensions
ROOT=Path(__file__).resolve().parents[1];D=ROOT/'docs/research'
BASE=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays')
OUT=D/'dedup-current-all-query-origins-20261011.json'
JOURNAL=D/'dedup-current-all-query-origins-20261011.jsonl'
ARCHIVE=BASE/'current-all-query-origins-qualified-20261011'
MANIFEST=D/'dedup-projected-portfolio-snapshot-20261011.json'
PREPARED=BASE/'prepared.json'
WEIGHTS=BASE/'learned-matching-models/loftr-outdoor-native-20261010.safetensors'
TORCH=Path('/Users/themoretheless/.codex/tmp/rrrah-loftr-native-runtime-20261010/lib/python3.12/site-packages/torch/lib')
def sha(path):
 h=hashlib.sha256()
 with Path(path).open('rb') as stream:
  for block in iter(lambda:stream.read(1024*1024),b''):h.update(block)
 return h.hexdigest()
def main():
 assert OUT.exists() and JOURNAL.exists() and ARCHIVE.exists()
 previous=json.loads(OUT.read_text());assert previous['status']=='running'
 for pid in [previous['producer_pid'],previous.get('current',{}).get('process_pid')]:
  if pid:assert subprocess.run(['ps','-p',str(pid),'-o','pid='],capture_output=True).returncode!=0
 assert all(sha(p)==h for p,h in previous['pins'].items())
 checkpoint=ARCHIVE/'interrupted-checkpoint-20261011.json';assert not checkpoint.exists();shutil.copy2(OUT,checkpoint)
 verifier_path=ROOT/'scripts/verify-dedup-current-all-query-origins-20261011.py'
 spec=importlib.util.spec_from_file_location('origin_verifier',verifier_path);verifier=importlib.util.module_from_spec(spec);spec.loader.exec_module(verifier)
 completed_rows=[]
 with JOURNAL.open() as journal:
  for line in journal:completed_rows.append(json.loads(line))
 assert len(completed_rows)>=previous['completed']
 generic=json.loads((D/'dedup-projected-portfolio-generic-20261011.json').read_text())
 assert generic['status']=='complete_bounded_frozen_projected_portfolio_generic_execution' and generic['tests']==131
 controls=json.loads((D/'dedup-projected-portfolio-native-20261011.json').read_text())
 assert controls['status']=='complete_bounded_eight_native_projected_portfolio_controls_execution'
 assert controls['all_expected_outcomes_match'] and controls['all_independent_projection_evidence_exact']
 prior=json.loads((D/'dedup-insufficient-anchor-full229-audit-20261011.json').read_text())
 assert prior['all_229_supported'] and prior['executed_pairs']==229
 for artifact in prior['artifacts']:
  assert sha(artifact['source'])==sha(artifact['archive'])==artifact['sha256']
 current_audit_path=D/'dedup-projected-portfolio-native-audit-20261011.json'
 current_audit=json.loads(current_audit_path.read_text())
 assert current_audit['status']=='complete_bounded_current_projected_portfolio_native_and_dependency_qualification'
 assert all(sha(p)==h for p,h in current_audit['pins'].items())
 manifest=json.loads(MANIFEST.read_text());snapshot=Path(manifest['snapshot'])
 def verify_source():assert all(sha(snapshot/p)==h for p,h in manifest['file_hashes'].items())
 verify_source();corpus=json.loads(PREPARED.read_text())
 originals=sorted(corpus['images']['original'],key=lambda x:x['filename'])
 queries=sorted(corpus['images']['strong'],key=lambda x:x['filename'])
 assert len(originals)==157 and len(queries)==229
 assert len({x['group_id'] for x in originals})==157
 expected={x['query_id']:x for x in corpus['positive_pairs']};assert len(expected)==229
 paths={};dimensions={}
 for kind,rows,folder in [('original',originals,'original-resolution-negative-200201'),('strong',queries,'original-resolution-strong-all')]:
  for row in rows:
   path=BASE/folder/row['filename'];assert sha(path)==row['source_sha256']
   dims=oriented_dimensions(path,row['source_size']);assert math.prod(dims)<=6400000 and path.stat().st_size<=5242880
   paths[kind,row['filename']]=path;dimensions[str(path)]=dims
 for query in queries:
  pair=expected[query['filename']];assert pair['label']=='publisher_origin_copy'
  assert pair['right']['source_sha256']==query['source_sha256'] and pair['left']['group_id']==query['group_id']
 binary=ARCHIVE/'tile_collection_files';assert sha(binary)==previous['pins'][str(binary)]
 assert str(snapshot).encode() in binary.read_bytes()
 for index,row in enumerate(completed_rows):
  query=queries[index//157];original=originals[index%157]
  assert sha(row['raw_output'])==row['raw_output_sha256']
  with gzip.open(row['raw_output'],'rt') as stream:raw=json.load(stream)
  verifier.verify_row(row,index,original,query,raw)
  assert row['source']==str(paths['original',original['filename']]) and row['target']==str(paths['strong',query['filename']])
 runtime_path=D/'dedup-loftr-native-20261010-runtime-pins.json'
 runtime=json.loads(runtime_path.read_text())['pins']
 pins={str(p):sha(p) for p in [MANIFEST,PREPARED,WEIGHTS,binary,Path(__file__),ROOT/'scripts/dedup_jpeg_domain.py',current_audit_path,runtime_path,D/'dedup-current-portfolio-corpus-native-build-20261011.log',D/'dedup-projected-portfolio-generic-20261011.json',D/'dedup-projected-portfolio-native-20261011.json',D/'dedup-insufficient-anchor-full229-audit-20261011.json',*paths.values()]}
 pins={**previous['pins'],**pins};pins.update(runtime);pins.update({str(checkpoint):sha(checkpoint),str(verifier_path):sha(verifier_path)})
 def verify_pins():assert all(sha(p)==h for p,h in pins.items())
 verify_pins()
 report={'status':'running','producer_pid':os.getpid(),'source_manifest':str(MANIFEST),'pins':pins,'required_comparisons':35953,'required_positive_pairs':229,'required_different_origin_pairs':35724,'completed':0,'counts':{},'journal':str(JOURNAL),'scope':__doc__,'limits':['Every comparison is a fresh complete two-file ModelFirst collection; not a single indexed 386-file collection','Different publisher origins are retrieval negatives, not independently annotated local visual negatives; supports require review','Original-resolution pixel rank evidence does not establish whole-image/file identity','No hard total Torch/workspace/RSS bound or comparative performance claim','This is the legacy rank-confirmed model/native portfolio on current frozen production; projected RGBA portfolio controls are separate']}
 report.update(completed=len(completed_rows),counts=dict(collections.Counter(r['status'] for r in completed_rows)),resume_history=[{'checkpoint':str(checkpoint),'prior_producer_pid':previous['producer_pid'],'preserved_rows':len(completed_rows),'unproven_interrupted_pair':previous.get('current'),'reason':'Recorded producer/process handles and PIDs confirmed absent after interruption'}])
 def save():
  tmp=OUT.with_suffix('.json.tmp');tmp.write_text(json.dumps(report,indent=2)+'\n');tmp.replace(OUT)
 save()
 try:
  with JOURNAL.open('a') as journal:
   scheduled=0
   for query in queries:
    for original in originals:
     index=scheduled;scheduled+=1
     if index<len(completed_rows):continue
     left=paths['original',original['filename']];right=paths['strong',query['filename']]
     positive=original['group_id']==query['group_id']
     if positive:assert expected[query['filename']]['left']['source_sha256']==original['source_sha256']
     source_tolerance=2*math.hypot(*dimensions[str(left)])/math.hypot(*dimensions[str(right)])
     guards=[binary,WEIGHTS,MANIFEST,PREPARED,left,right]
     assert all(sha(p)==pins[str(p)] for p in guards)
     assert index==report['completed'];row={'index':index,'query':query['filename'],'original':original['filename'],'query_group':query['group_id'],'original_group':original['group_id'],'label':'publisher_origin_copy' if positive else 'different_publisher_origin','source':str(left),'target':str(right),'source_tolerance':source_tolerance}
     command=['/usr/bin/time','-l','/usr/bin/env',f'DYLD_LIBRARY_PATH={TORCH}',str(binary),str(WEIGHTS),'640',str(source_tolerance),'model-first',str(left),str(right)]
     process=subprocess.Popen(command,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
     report['current']={**row,'process_pid':process.pid,'command':command};save();start=time.monotonic()
     stdout,stderr=process.communicate();assert all(sha(p)==pins[str(p)] for p in guards)
     row.update(returncode=process.returncode,elapsed_seconds=time.monotonic()-start)
     raw=ARCHIVE/f'{index:05d}.json.gz'
     with gzip.open(raw,'wt') as stream:json.dump({'stdout':stdout,'stderr':stderr},stream)
     row.update(raw_output=str(raw),raw_output_sha256=sha(raw))
     if process.returncode:row['status']='execution_failed'
     else:
      evidence=json.loads(stdout);assert evidence['status']=='ok' and evidence['files']==[0,1] and evidence['source_paths']==[str(left),str(right)]
      assert evidence['managed_used']==0 and evidence['managed_peak']<=512*1024*1024 and len(evidence['pairs'])==1
      pair=evidence['pairs'][0];assert (pair['left'],pair['right'])==(0,1)
      proof=pair['evidence'];assert isinstance(proof['supported'],bool)
      row.update(supported=proof['supported'],terminal=proof['terminal'],managed_peak=evidence['managed_peak'])
      if proof['supported']:
       pixelproof=proof['attempts'][-1]['evidence'] if proof['terminal']=='model_tiles' else proof
       assert pixelproof['pixels']['supported']
      row['status']=('positive_supported' if positive else 'different_origin_supported_review') if proof['supported'] else ('positive_miss' if positive else 'different_origin_rejected')
     journal.write(json.dumps(row)+'\n');journal.flush();os.fsync(journal.fileno())
     report['completed']+=1;counts=report['counts'];counts[row['status']]=counts.get(row['status'],0)+1
     report.pop('current',None);save();print(index,row['query'],row['original'],row['status'],flush=True)
  assert report['completed']==35953
  verify_source();verify_pins();report.update(status='complete_bounded_all_query_origin_execution',journal_sha256=sha(JOURNAL))
 except Exception as error:report.update(status='failed_integrity_or_producer',error=str(error));raise
 finally:save()
if __name__=='__main__':main()
