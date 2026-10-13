"""Verify the exact Cartesian schedule and recorded API outcomes; partial is never complete."""
import argparse,collections,gzip,hashlib,json,math
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1];D=ROOT/'docs/research'
REPORT=D/'dedup-current-all-query-origins-20261011.json'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def verify_row(row,index,original,query,raw):
 assert row['index']==index
 assert row['original']==original['filename'] and row['query']==query['filename']
 assert row['original_group']==original['group_id'] and row['query_group']==query['group_id']
 positive=original['group_id']==query['group_id']
 assert row['label']==('publisher_origin_copy' if positive else 'different_publisher_origin')
 assert math.isfinite(row['elapsed_seconds']) and row['elapsed_seconds']>0
 assert Path(row['source']).name==row['original'] and Path(row['target']).name==row['query']
 assert isinstance(raw['stdout'],str) and isinstance(raw['stderr'],str)
 if row['returncode']:
  assert row['status']=='execution_failed' and 'supported' not in row
  return
 evidence=json.loads(raw['stdout'])
 assert evidence['status']=='ok' and evidence['files']==[0,1]
 assert evidence['source_paths']==[row['source'],row['target']]
 assert evidence['managed_used']==0 and evidence['managed_peak']<=512*1024*1024
 assert evidence['managed_peak']==row['managed_peak'] and len(evidence['pairs'])==1
 pair=evidence['pairs'][0];assert (pair['left'],pair['right'])==(0,1)
 proof=pair['evidence'];supported=proof['supported'];assert isinstance(supported,bool)
 assert row['supported']==supported and row['terminal']==proof['terminal']
 if proof['terminal']=='model_tiles':
  assert 1<=len(proof['attempts'])<=25
  if supported:assert proof['attempts'][-1]['evidence']['pixels']['supported']
 else:
  assert proof['terminal']=='native' and 1<=proof['native_recipes']<=6
  if supported:assert proof['pixels']['supported']
 expected=('positive_supported' if positive else 'different_origin_supported_review') if supported else ('positive_miss' if positive else 'different_origin_rejected')
 assert row['status']==expected
 if supported:
  terminal=proof['attempts'][-1]['evidence'] if proof['terminal']=='model_tiles' else proof
  pixels=terminal['pixels'];assert len(pixels['anchors'])==len(pixels['directions'])==2
  assert all(n>=10 for n in pixels['anchors'])
  for direction in pixels['directions']:
   assert direction['sites']>0 and direction['informative_pairs']>=1000
   assert direction['valid_sites']/direction['sites']>=.3
   assert direction['agreeing_pairs']/direction['informative_pairs']>=.9

def main():
 parser=argparse.ArgumentParser();parser.add_argument('--partial',action='store_true');args=parser.parse_args()
 report=json.loads(REPORT.read_text());corpus_path=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays/prepared.json')
 assert sha(corpus_path)==report['pins'][str(corpus_path)]
 corpus=json.loads(corpus_path.read_text());originals=sorted(corpus['images']['original'],key=lambda x:x['filename']);queries=sorted(corpus['images']['strong'],key=lambda x:x['filename'])
 assert len(originals)==157 and len(queries)==229
 assert report['required_comparisons']==35953 and report['required_positive_pairs']==229 and report['required_different_origin_pairs']==35724
 completed=report['completed'];assert 0<=completed<=35953;counts=collections.Counter()
 with Path(report['journal']).open() as journal:
  for index in range(completed):
   row=json.loads(next(journal));query=queries[index//157];original=originals[index%157]
   assert sha(row['raw_output'])==row['raw_output_sha256']
   with gzip.open(row['raw_output'],'rt') as stream:raw=json.load(stream)
   verify_row(row,index,original,query,raw)
   assert report['pins'][row['source']]==original['source_sha256'] and report['pins'][row['target']]==query['source_sha256']
   counts[row['status']]+=1
  if not args.partial:assert journal.read()==''
 assert dict(counts)==report['counts']
 if args.partial:
  print(json.dumps({'status':'verified_partial_prefix_not_completion','completed':completed,'required':35953,'counts':dict(counts)}));return
 assert report['status']=='complete_bounded_all_query_origin_execution' and completed==35953
 assert sha(report['journal'])==report['journal_sha256']
 assert all(sha(p)==h for p,h in report['pins'].items())
 manifest=json.loads(Path(report['source_manifest']).read_text())
 assert all(sha(Path(manifest['snapshot'])/p)==h for p,h in manifest['file_hashes'].items())
 positive=sum(v for k,v in counts.items() if k.startswith('positive_'))
 negative=sum(v for k,v in counts.items() if k.startswith('different_origin_'))
 print(json.dumps({'status':'verified_complete_cartesian_execution','completed':completed,'counts':dict(counts),'positive_decisions':positive,'different_origin_decisions':negative,'execution_failures':counts['execution_failed'],'all_known_positives_supported':counts['positive_supported']==229,'different_origin_supports_require_review':counts['different_origin_supported_review'],'limits':'Complete execution does not imply all cases passed; origin labels are not semantic/local-mask annotations.'}))
if __name__=='__main__':main()
