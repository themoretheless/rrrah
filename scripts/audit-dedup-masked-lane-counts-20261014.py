#!/usr/bin/env python3
from pathlib import Path
import hashlib,json,shutil
R=Path(__file__).resolve().parents[1];D=R/'docs/research';P=D/'dedup-masked-lane-counts-20261014.json';d=json.loads(P.read_text());sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
assert d['status']=='complete_pending_audit'and len(d['results'])==3
assert all(sha(p)==h for p,h in d['pins'].items())and all(sha(p)==h for p,h in d['feature_pins'].items())
rows=[];F=D/'dedup-masked-lane-counts-20261014';F.mkdir(exist_ok=False)
for result in d['results']:
    assert sha(result['stdout'])==result['stdout_sha256']and sha(result['stderr'])==result['stderr_sha256']and Path(result['stderr']).read_bytes()==b''
    value=json.loads(Path(result['stdout']).read_text());assert value==result['value']and value['left']==value['right']==2048
    assert sum(value['counts'])==1048576 and value['callbacks']==8394754
    c=value['counts'];before=4*sum(c[1:]);after=c[1]+2*c[2]+4*sum(c[3:]);assert before==result['second_pass_distance_pairs_before']and after==result['second_pass_distance_pairs_after']
    total=2048*2048;rows.append(dict(label=result['label'],mask_counts=c,first_pass_pairs=total,second_pass_pairs_before=before,second_pass_pairs_after=after,total_distance_pair_reduction=(before-after)/(total+before)))
    for key in ['stdout','stderr']:
        p=Path(result[key]);copy=F/p.name;shutil.copyfile(p,copy);assert sha(p)==sha(copy)
assert rows[1]['label']=='reverse'and rows[1]['total_distance_pair_reduction']==0
O=D/'dedup-masked-lane-counts-audit-20261014.json';assert not O.exists();O.write_text(json.dumps(dict(status='verified_standalone_lane_distribution',report=str(P.relative_to(R)),report_sha256=sha(P),auditor_sha256=sha(__file__),rows=rows,scope=d['scope'],interpretation='Reverse case saves zero descriptor pairs, yet public CPU median differed. Removed-pair count alone cannot attribute timing changes; machine-code layout and shared-host variation require care. Forward saves only2.27percent total pairs and self0.67percent. No consistent whole-component gain proved.'),indent=2)+'\n')
public_path=D/'dedup-masked-lanes-natural-large-audit-20261014.json';public=json.loads(public_path.read_text());assert public['status']=='verified_three_large_natural_public_managed_comparisons'
assert any(r['median_cpu_ratio']>1.05 for r in public['summary']if r['extra_bytes'])
Decision=D/'dedup-gradient-masked-lanes-decision-v2-20261014.json';assert not Decision.exists();Decision.write_text(json.dumps(dict(status='not_adopted_after_actual_large_natural_public_comparison',public_audit=str(public_path.relative_to(R)),public_audit_sha256=sha(public_path),lane_audit=str(O.relative_to(R)),lane_audit_sha256=sha(O),reason='All coordinates/callbacks/memory gates pass, but genuine large natural forward medianCPUregresses6.76percent while reverse improves5.43percent and self10.57percent. Reverse removes zero distance pairs; timings are not a consistent algorithmic gain. Wide per-pair ranges and limited cases do not justify production adoption.',production_gradient_changed=False,remaining='Existing native full-operation/unroll/corpus/default/platform/format/collection/resource gates remain active and incomplete.',next='Evaluate ordered kernel unroll8/16against current4without added branch policy or heap; standalone bit parity plus fresh-process CPU gate running. Pure kernel results still require managed/native/whole-operation qualification before adoption.'),indent=2)+'\n');print(rows)
