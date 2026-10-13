#!/usr/bin/env python3
import hashlib,json,re
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 rp=D/'dedup-loftr-selected-windows-collection-20261012.json';r=json.loads(rp.read_text());assert r['status']=='complete_one_negative_pair_equivalent_not_adopted';assert all(sha(p)==h for p,h in r['pins'].items());assert [x['mode'] for x in r['results']]==['baseline','selected']
 bp=D/'dedup-loftr-selected-windows-collection-build-audit-20261012.json';b=json.loads(bp.read_text());assert all(sha(p)==h for p,h in b['pins'].items())
 values=[]
 for row in r['results']:
  assert row['status']=='complete' and row['returncode']==0 and Path(row['stdout_path']).read_text()==row['stdout'] and Path(row['stderr_path']).read_text()==row['stderr'];v=json.loads(row['stdout']);assert v==row['evidence'] and v['managed_used']==0 and v['managed_report_used']>0
  assert v['evidence']['file_ids']==[1,2] and len(v['evidence']['pairs'])==1;pair=v['evidence']['pairs'][0];assert not pair['supported']
  for direction in ['forward','reverse']:
   result=pair[direction];assert result is not None and not result['supported'];assert result['primary']['attempts']==25 and result['recovery']['search']['attempts']==9
  assert row['rss_bytes']==int(re.search(r'(\d+)\s+maximum resident set size',row['stderr'])[1]);values.append(v)
 assert values[0]==values[1]
 a,c=r['results'];delta=c['rss_bytes']-a['rss_bytes'];assert delta>0
 out=D/'dedup-loftr-selected-windows-collection-audit-20261012.json';assert not out.exists();pins=dict(r['pins']);pins.update({str(p):sha(p) for p in [rp,bp,Path(__file__)]});out.write_text(json.dumps({'status':'verified_equal_whole_collection_evidence_candidate_not_adopted_due_to_rss','baseline_rss_bytes':a['rss_bytes'],'selected_rss_bytes':c['rss_bytes'],'rss_delta_bytes':delta,'rss_delta_fraction':delta/a['rss_bytes'],'pins':pins,'scope':'One fixed publisher distractor, actual unchanged public bidirectional collection,25primary+9recovery attempts each direction. Complete JSON evidence and managed accounting identical, zero retained managed credit. Candidate whole RSS is higher despite isolated fine-stage reduction. One fresh process per variant, concurrent workload, so elapsed differences are not a qualified speed benchmark. Not general precision/fullpositive/full500/platform/hardcap evidence. Candidate retained as experiment, not adopted.'},indent=2)+'\n');print('Verified complete evidence equality; candidate RSS delta',delta,'bytes; not adopted')
if __name__=='__main__':main()
