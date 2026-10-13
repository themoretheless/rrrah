#!/usr/bin/env python3
import hashlib,json,re,statistics
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 p=D/'dedup-gradient-eight-real-20261012.json';r=json.loads(p.read_text());assert r['status']=='complete_ten_real_descriptor_controls_not_adopted'and r['returncode']==0 and r['build_returncode']==0;assert all(sha(k)==v for k,v in r['pins'].items());assert sha(r['build_log'])==r['build_log_sha256']
 roots=[Path(k) for k in r['pins'] if k.endswith('/crates/rrrah-dedup/src/gradient_batch_tests.rs')];assert len(roots)==1;text=roots[0].read_text();original=(R/'crates/rrrah-dedup/src/gradient.rs').read_text();a=original.index('fn match_gradients_selected(');b=original.index('\n/// Describe a rotated',a);baseline=text[text.index('fn current_four_lane('):].split('\n#[test]',1)[0].strip().replace('current_four_lane(', 'match_gradients_selected(').replace('current_four_distances(', 'distances_four(');assert baseline==original[a:b].strip()
 real=text[text.index('fn real_photo_matcher_timing()'):].split('\n#[test]',1)[0];assert 'current_four_lane('in real and 'assert_eq!(coordinates(got).unwrap(), expected);'in real
 parsed=re.findall(r'real_matcher pair=(\S+) features=(\d+):(\d+) radius=(\S+) batched=(true|false) matches=(\d+) seconds=([0-9.]+)',r['stderr']);assert len(parsed)==40
 summaries=[]
 for i in range(5):
  for radius in ['None','Some(64.0)']:
   rows=[x for x in parsed if x[0]==f'0:{i}'and x[3]==radius];assert len(rows)==4 and [x[4]for x in rows]==['false','true','true','false'] and len({x[5]for x in rows})==1
   four=statistics.median(float(x[6])for x in rows if x[4]=='false');eight=statistics.median(float(x[6])for x in rows if x[4]=='true');summaries.append(dict(pair=f'0:{i}',radius=radius,current_four_seconds=four,candidate_eight_seconds=eight,four_over_eight=four/eight))
 assert summaries==r['summary'];slower=sum(x['four_over_eight']<1 for x in summaries);assert slower==7
 out=D/'dedup-gradient-eight-real-audit-20261012.json';assert not out.exists();out.write_text(json.dumps({'status':'verified_exact_real_outcomes_no_consistent_timing_gain_not_adopted','controls':10,'candidate_slower_controls':slower,'summary':summaries,'pins':{str(x):sha(x)for x in [p,Path(__file__),R/'crates/rrrah-dedup/src/gradient.rs']},'scope':'Complete matcher alternating actual current-four versus candidate-eight on original JPEG descriptors; exact coordinates asserted in pinned executable. Seven slower medians under concurrent host load justify no adoption, not a universal performance regression. Not whole-portfolio timing or recall.'},indent=2)+'\n');print('Verified ten real controls: exact results, seven slower medians, candidate not adopted')
if __name__=='__main__':main()
