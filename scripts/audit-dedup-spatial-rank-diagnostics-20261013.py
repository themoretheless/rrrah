#!/usr/bin/env python3
import json,hashlib
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
source_audit=D/'dedup-fingerprint-nonfinite-audit-20261013.json';a=json.loads(source_audit.read_text());S=Path(a['source']);assert all(sha(S/p)==h for p,h in a['source_sha256'].items())
positive_plan=D/'dedup-model-positive-pixel-diagnostic-plan-20261013.json';positive=json.loads(positive_plan.read_text());input_pins={}
for c in positive['cases']:input_pins.update(c['inputs'])
for label in ['negative','self_control']:
 q=D/('dedup-model-pixel-diagnostic-'+label+'-v2-plan-20261013.json');v=json.loads(q.read_text());input_pins.update(v['pins'])
assert all(sha(p)==h for p,h in input_pins.items())
rows=[]
for stem in ['dense-rank','dense-region-rank','multiscale-rank']:
 P=D/('dedup-model-'+stem+'-diagnostic-plan-20261013.json');O=D/('dedup-model-'+stem+'-diagnostic-20261013.json');plan=json.loads(P.read_text());result=json.loads(O.read_text())
 assert result['status']=='complete' and result['plan_sha256']==sha(P) and sha(result['executable'])==result['executable_sha256']
 H=Path(plan['harness']);assert all(sha(H/p)==h for p,h in plan['harness_sha256'].items())
 assert len(result['results'])==len(plan['cases'])==12
 for c,r in zip(plan['cases'],result['results']):
  assert c['label']==r['label'] and sha(c['plan'])==c['plan_sha256'] and r['returncode']==0 and r['result']['managed_used']==0
  input_plan=json.loads(Path(c['plan']).read_text())
  # Verify original content against pinned pre-execution input plans.
  for side in ['left','right']: assert sha(input_plan[side])==input_pins[input_plan[side]]
 rows.append(dict(kind=stem,plan=str(P),plan_sha256=sha(P),report=str(O),report_sha256=sha(O),executable_sha256=result['executable_sha256'],manifest_sha256=sha(H/'Cargo.toml'),lock_sha256=sha(H/'Cargo.lock'),results=result['results']))
out=D/'dedup-spatial-rank-diagnostics-audit-20261013.json';assert not out.exists();out.write_text(json.dumps(dict(status='verified36_spatial_rank_diagnostics_not_acceptance_fix',source_audit_sha256=sha(source_audit),source_files=len(a['source_sha256']),input_pins=input_pins,diagnostics=rows,scope='Three fixed diagnostic plans, each10 known positives plus known negative and identity. Original observed geometry, anchor-derived bounds; not independent heldout pixels, no calibrated threshold, no production change.'),indent=2)+'\n');print(out)
