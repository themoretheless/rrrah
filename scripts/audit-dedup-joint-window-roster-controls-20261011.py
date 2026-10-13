#!/usr/bin/env python3
"""Close the bounded window diagnostic without treating unknowns as negatives."""
import hashlib,json
from pathlib import Path
R=Path(__file__).resolve().parents[1]; D=R/'docs/research'
def sha(p): return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 p=D/'dedup-joint-model-window-roster-controls-20261011.json'; h=D/'dedup-joint-window-roster-harness-20261011.json'
 report=json.loads(p.read_text()); harness=json.loads(h.read_text())
 assert report['status']=='complete_bounded_joint_model_gap_controls'
 pins={**report['pins'],**harness['pins'],str(p):sha(p),str(h):sha(h),str(Path(__file__)):sha(__file__)}
 assert all(sha(k)==v for k,v in pins.items())
 assert [x['baseline_index'] for x in report['plan']]==[255,67]
 results=[]
 assert len(report['plan'])==len(report['results'])==2
 for plan,row in zip(report['plan'],report['results']):
  assert row['returncode']==0
  v=json.loads(row['stdout']); roster=v['window_roster']; assert len(roster)==v['attempts']==25
  assert v['managed_used']==0 and not v['supported']
  assert len({(tuple(a['left_rectangle']),tuple(a['right_rectangle'])) for a in roster})==25
  assert all(not a['supported'] for a in roster)
  pins[row['log']]=row['log_sha256'];assert sha(row['log'])==row['log_sha256']
  counts={k:sum(bool(a[k]) for a in roster) for k in ['has_comparison','has_geometry','supported']}
  results.append({'baseline_index':plan['baseline_index'],'group_id':plan['group_id'],'direction':plan['direction'],'attempts':25,**counts,'unknown':True})
 out=D/'dedup-joint-window-roster-audit-20261011.json';assert not out.exists()
 out.write_text(json.dumps({'status':'verified_all_window_diagnostics','pins':pins,'results':results,'scope':'Both reverse positives remain unknown. Geometry counts describe all25 windows; no semantic negative/precision/full-corpus claim. Frozen pre-buffer-reuse production.'},indent=2)+'\n')
 print(json.dumps(results))
if __name__=='__main__':main()
