#!/usr/bin/env python3
import hashlib,json,math
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 a=json.loads((D/'dedup-gradient-corpus-radius-audit-20261011.json').read_text());assert all(sha(p)==h for p,h in a['pins'].items())
 r=json.loads((D/'dedup-gradient-corpus-index-20261011.json').read_text());assert r['status']=='complete_bounded_normalized_corpus_index_oracle';archive=json.loads((D/'dedup-gradient-corpus-index-source-archive-20261011.json').read_text());assert sha(archive['parent_report'])==archive['parent_report_sha256']
 for p,h in r['pins'].items():
  observed=archive['sources'].get(p,{}).get('archive',p);assert sha(observed)==h
 for row in r['results']:
  assert row['returncode']==0 and row['status']=='passed_oracle_equality'
  for path,key in [('log','log_sha256'),('paths','paths_sha256'),('descriptors_path','descriptors_sha256')]:assert sha(row[path])==row[key]
 data=json.loads(Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays/prepared.json').read_text());files=sorted(data['images']['original'],key=lambda x:x['filename'])+sorted(data['images']['strong'],key=lambda x:x['filename']);assert len(files)==386
 minima=[]
 for line in Path('/Users/themoretheless/.codex/tmp/rrrah-gradient-corpus-index-qualified-20261011/all386.minimum-distances.tsv').read_text().splitlines():
  left,right,value=line.split('\t');left=int(left);right=int(right);value=float(value);assert 0<=left<right<386 and value>=0 and not math.isnan(value);minima.append((left,right,value))
 assert [(i,j) for i,j,_ in minima]==[(i,j) for i in range(386) for j in range(i+1,386)]
 positives=[v for i,j,v in minima if i<157<=j and files[i]['group_id']==files[j]['group_id']];assert len(positives)==229
 assert max(positives)==a['largest_positive_minimum_distance']
 for row in a['sweep']:
  assert sum(v<=row['radius'] for _,_,v in minima)==row['all_pairs_proposed']
  assert sum(v<=row['radius'] for v in positives)==row['publisher_original_query_proposed']
 log=Path(r['results'][-1]['log']).read_text();pairs={tuple(map(int,l.split('\t')[1:])) for l in log.splitlines() if l.startswith('P\t')}
 assert pairs=={(i,j) for i,j,v in minima if v<=0.1}
 print('verified 386 files, 11750 descriptors, 74305 file minima, 229 publisher pairs; bounded proposal evidence only')
if __name__=='__main__':main()
