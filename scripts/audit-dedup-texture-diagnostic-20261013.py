#!/usr/bin/env python3
import json,hashlib
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
P=D/'dedup-model-texture-diagnostic-v3-plan-20261013.json';O=D/'dedup-model-texture-diagnostic-v3-20261013.json';plan=json.loads(P.read_text());data=json.loads(O.read_text());assert data['status']=='complete' and data['plan_sha256']==sha(P) and sha(data['executable'])==data['executable_sha256'];assert len(data['results'])==len(plan['cases'])==18
H=Path(plan['harness']);assert all(sha(H/p)==h for p,h in plan['harness_sha256'].items())
A=D/'dedup-fingerprint-nonfinite-audit-20261013.json';a=json.loads(A.read_text());S=Path(a['source']);assert all(sha(S/p)==h for p,h in a['source_sha256'].items())
inputs=dict(plan['control_inputs']);positive=json.loads((D/'dedup-model-positive-pixel-diagnostic-plan-20261013.json').read_text())
for c in positive['cases']:inputs.update(c['inputs'])
for label in ['negative','self_control']:inputs.update(json.loads((D/('dedup-model-pixel-diagnostic-'+label+'-v2-plan-20261013.json')).read_text())['pins'])
assert all(sha(p)==h for p,h in inputs.items())
prior=json.loads((D/'dedup-model-texture-diagnostic-v2-20261013.json').read_text());assert len(prior['results'])==12
controls=[]
for i,(c,r) in enumerate(zip(plan['cases'],data['results'])):
 assert c['label']==r['label'] and sha(c['plan'])==c['plan_sha256'] and r['returncode']==0 and r['result']['managed_used']==0
 for direction in r['result']['directions']:
  for e in direction:
   assert not e['work_cap_exceeded'] and e['total_tiles']==e['invalid_tiles']+e['flat_tiles']+e['informative_tiles'] and sum(e['histogram_minus1_to1_step0_2'])==e['informative_tiles']
   if 'expect' in c:
    mean=e['mean_correlation'];want=c['expect'];assert (e['informative_tiles']==0) if want=='flat' else (e['informative_tiles']>0 and mean is not None and (mean>.999 if want=='positive' else mean<-.999 if want=='negative' else abs(mean)<.3))
 if i<12:assert r==prior['results'][i]
 else:controls.append(dict(label=c['label'],expected=c['expect'],passed=True))
out=D/'dedup-texture-diagnostic-audit-20261013.json';assert not out.exists();out.write_text(json.dumps(dict(status='verified18_texture_diagnostics_not_acceptance_fix',source_audit_sha256=sha(A),source_files=len(a['source_sha256']),plan_sha256=sha(P),report_sha256=sha(O),executable_sha256=data['executable_sha256'],manifest_sha256=sha(H/'Cargo.toml'),lock_sha256=sha(H/'Cargo.lock'),input_pins=inputs,controls=controls,natural_results_identical_to_v2=True,scope='18 actual native executions, including6 authored PFM math controls with explicit linear-sRGB primaries. Texture correlation at saved geometry and anchor-derived regions is not a duplicate classifier, calibrated threshold or independent holdout. Initial PFM control calls correctly refused unspecified primaries; preserved separately.'),indent=2)+'\n');print(out)
