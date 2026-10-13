#!/usr/bin/env python3
import hashlib,json,re,sys
from pathlib import Path
R=Path(__file__).resolve().parents[1]
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 name=sys.argv[1]; path=R/f'docs/research/dedup-loftr-{name}-20261011.json'; report=json.loads(path.read_text());assert report['status']=='complete_bounded_original_positive_memory_comparison'
 assert all(sha(p)==h for p,h in report['pins'].items())
 mp=Path(report['source_manifest']);m=json.loads(mp.read_text());root=Path(m['snapshot']);parent=json.loads(Path(m['parent_manifest']).read_text());assert sha(m['parent_manifest'])==m['parent_sha256']
 assert all(sha(root/f)==h for f,h in m['file_hashes'].items());assert [f for f,h in m['file_hashes'].items() if parent['file_hashes'][f]!=h]==m['changes']
 rows={r['case']:r for r in report['results']};assert len(rows)==4
 comparisons=[]
 for direction in ['forward','reverse']:
  a=rows['baseline_'+direction];b=rows['candidate_'+direction]
  for row in [a,b]:
   assert row['returncode']==0 and sha(row['log'])==row['log_sha256'];assert Path(row['log']).read_text()==row['stdout']+row['stderr'];assert row['rss_bytes']==int(re.search(r'(\d+)\s+maximum resident set size',row['stderr']).group(1))
  av=json.loads(a['stdout']);bv=json.loads(b['stdout']);assert av['pairs']==bv['pairs'] and av['source_paths']==bv['source_paths'];assert av['managed_used']==bv['managed_used']==0;assert bv['pairs'][0]['evidence']['supported'] is True
  comparisons.append({'direction':direction,'baseline_rss_bytes':a['rss_bytes'],'candidate_rss_bytes':b['rss_bytes'],'rss_reduction_fraction':1-b['rss_bytes']/a['rss_bytes'],'evidence_equal':True})
 out=R/f'docs/research/dedup-loftr-{name}-audit-20261011.json';assert not out.exists();out.write_text(json.dumps({'status':'verified_bounded_memory_comparison','report':str(path),'report_sha256':sha(path),'source_files':len(m['file_hashes']),'changes':m['changes'],'comparisons':comparisons,'limits':'One real positive pair, two directions, single fresh process per variant/direction; not hard RSS cap, full corpus or statistical timing qualification.'},indent=2)+'\n');print(json.dumps(comparisons))
if __name__=='__main__':main()
