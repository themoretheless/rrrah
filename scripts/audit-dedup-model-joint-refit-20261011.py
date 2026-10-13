#!/usr/bin/env python3
"""Check captured point identity, joint residuals and unchanged pixel refusal."""
import hashlib
import json
import math
import struct
from pathlib import Path

R = Path(__file__).resolve().parents[1]
D = R / 'docs/research'

def sha(p):
    return hashlib.sha256(Path(p).read_bytes()).hexdigest()

def invert(h):
    a,b,c = h[0]; d,e,f = h[1]; g,i,j = h[2]
    adj = [[e*j-f*i,c*i-b*j,b*f-c*e],
           [f*g-d*j,a*j-c*g,c*d-a*f],
           [d*i-e*g,b*g-a*i,a*e-b*d]]
    det = a*adj[0][0]+b*adj[1][0]+c*adj[2][0]
    assert math.isfinite(det) and det != 0
    return [[v/det for v in row] for row in adj]

def project(h, p):
    v = [row[0]*p[0]+row[1]*p[1]+row[2] for row in h]
    assert all(math.isfinite(x) for x in v) and v[2] != 0
    return [v[0]/v[2],v[1]/v[2]]

def main():
    report = D/'dedup-model-joint-refit-20261011.json'
    out = D/'dedup-model-joint-refit-audit-20261011.json'
    assert not out.exists()
    r = json.loads(report.read_text())
    assert r['status'] == 'complete_bounded_model_joint_refit'
    assert len(r['plan']) == len(r['results']) == 6
    pins = dict(r['pins'])
    pins.update({str(report):sha(report),str(Path(__file__)):sha(__file__)})
    assert all(sha(p)==h for p,h in pins.items())
    harvest = json.loads((D/'dedup-model-points-trace-harvest-20261011.json').read_text())
    assert all(sha(p)==h for p,h in harvest['pins'].items())
    decisions = []
    for plan,row in zip(r['plan'],r['results']):
        assert row['returncode']==0 and row['status']=='inconclusive'
        assert (row['trace_index'],row['trials'])==(plan['trace_index'],plan['trials'])
        inp = json.loads(Path(plan['input']).read_text())
        trace = harvest['trace'][plan['trace_index']]
        assert [p['source_bits']+p['target_bits'] for p in inp['points']]==trace['coordinate_bits']
        assert inp['source_tolerance']==harvest['plan']['source_tolerance']
        assert inp['source_dimensions']==trace['source_dimensions']
        assert inp['target_dimensions']==trace['target_dimensions']
        assert sha(row['log'])==row['log_sha256']
        assert Path(row['log']).read_text()==row['stdout']+row['stderr']
        pins[row['log']]=row['log_sha256']
        v = json.loads(row['stdout'])
        assert v['managed_used']==0 and v['seed']==173
        assert v['trials']==plan['trials'] and v['points']==len(inp['points'])
        assert v['source_tolerance']==inp['source_tolerance'] and v['target_tolerance']==2
        assert v['maximum_projection_checks']==2*v['points']*(v['trials']+1)
        result = {'trace_index':plan['trace_index'],'trials':plan['trials'],
                  'geometric_inliers':0,'pixel_supported':None}
        if v['best'] is not None:
            h = v['best']['transform']; inverse = invert(h); error = 0
            indices = v['best']['joint_inliers']
            assert len(set(indices))==len(indices)
            for index in indices:
                bits = trace['coordinate_bits'][index]
                p = [struct.unpack('>d',struct.pack('>Q',b))[0] for b in bits]
                forward = sum((a-b)**2 for a,b in zip(project(h,p[:2]),p[2:]))
                reverse = sum((a-b)**2 for a,b in zip(project(inverse,p[2:]),p[:2]))
                assert forward<=4 and reverse<=v['source_tolerance']**2
                error += forward/4+reverse/v['source_tolerance']**2
            assert math.isclose(error,v['best']['normalized_bidirectional_error'],rel_tol=1e-7,abs_tol=1e-7)
            pixels = v['anchor_result']; assert pixels['supported'] is False
            ratios = [d['agreeing_pairs']/d['informative_pairs'] for d in pixels['directions']]
            assert min(ratios)<.9
            result.update(geometric_inliers=len(indices),pixel_supported=False,
                          pixel_agreement=ratios,normalized_error=error)
        else:
            assert v['anchor_result'] is None
        decisions.append(result)
    assert {(v['trace_index'],v['trials']) for v in decisions}=={(i,t) for i in [0,11,16] for t in [20000,1000000]}
    assert all(sha(p)==h for p,h in pins.items())
    out.write_text(json.dumps({'status':'verified_bounded_model_joint_refit_inconclusive',
        'pins':pins,'results':decisions,'scope':'Exact-bit identity and independent forward/inverse residual arithmetic checked. Original-pixel output remains below unchanged 90 percent agreement. No new model API adoption, corpus recovery or negative decision.'},indent=2)+'\n')
    print('Verified six refits; no supported original-pixel match')

if __name__=='__main__':
    main()
