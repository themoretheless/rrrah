#!/usr/bin/env python3
from pathlib import Path
import hashlib,json,re,shutil
R=Path(__file__).resolve().parents[1];D=R/'docs/research';sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
F=D/'dedup-gradient-lanes16-root-gates-20261014';F.mkdir(exist_ok=True);rows=[];pins={}
for name,counts in [('no-features',(45,0,2)),('raster',(62,0,3))]:
 P=D/f'dedup-gradient-lanes16-root-{name}-20261014.json';d=json.loads(P.read_text());assert d['status']=='passed_pending_audit'and d['build_returncode']==d['test_returncode']==0 and (d['passed'],d['failed'],d['ignored'])==counts
 assert all(sha(p)==h for p,h in d['pins'].items())and all(sha(p)==h for p,h in d['dependency_pins'].items())and all(sha(R/p)==h for p,h in d['source_sha256'].items())
 folder=Path(f'/private/tmp/rrrah-gradient-lanes16-root-{name}-20261014');listed=re.findall(r'^(.+): test$',(folder/'inventory.log').read_text(),re.M);executed=re.findall(r'^test (.+) \.\.\. (ok|ignored(?:,.*)?)$',(folder/'tests.log').read_text(),re.M)
 assert len(listed)==len(set(listed))==len(executed)==counts[0]+counts[2]and sorted(listed)==sorted(n for n,_ in executed)
 assert ('gradient::matching_tests::sixteen_descriptor_lanes_preserve_scalar_bits_and_every_tail','ok')in executed
 dest=F/name;dest.mkdir(exist_ok=True)
 for p in [folder/'build.log',folder/'inventory.log',folder/'tests.log']:
  copy=dest/p.name
  if copy.exists():assert sha(copy)==sha(p)
  else:shutil.copyfile(p,copy)
  pins[str(copy.relative_to(R))]=sha(copy)
 rows.append(dict(mode=name,passed=counts[0],failed=0,manual_ignored=counts[2],report=str(P.relative_to(R)),report_sha256=sha(P),inventory_exact=True))
code=D/'dedup-gradient-lanes16-root-code-audit-20261014.json';a=json.loads(code.read_text());assert a['status']=='verified_target_selected_runtime_source_equivalence'and sha(R/a['root_source'])==a['root_source_sha256']
O=D/'dedup-gradient-lanes16-root-gates-audit-20261014.json';assert not O.exists();O.write_text(json.dumps(dict(status='verified_current_root_target_gated_firstpass16_unit_gates',rows=rows,auditor_sha256=sha(__file__),source_equivalence_audit=str(code.relative_to(R)),source_equivalence_audit_sha256=sha(code),retained_artifacts=pins,scope='Actual current root nooptional45pass2manualignored and raster62pass3manualignored, directrustc real pinned cached deps, exact inventory and root source pins. Firstpass16only on macOSaarch64, secondpass4; other runtime target source equals old4baseline after cfg expansion. No wholeCargo/decode/native/currentcorpus/UI/defaultcollection/allformats/allplatform/workspace completion proof.'),indent=2)+'\n');print(rows)
