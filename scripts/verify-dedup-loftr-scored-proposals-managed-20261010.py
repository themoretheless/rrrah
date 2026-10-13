"""Independent NumPy residual and proposal-selection audit; not a pixel oracle."""
import hashlib
import json
from pathlib import Path
import numpy as np

ROOT = Path(__file__).resolve().parents[1]
D = ROOT / 'docs/research'
T = Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays/native-loftr-inputs-20261010')
REPORT = D / 'dedup-loftr-scored-proposals-managed-20261010.json'
OUT = D / 'dedup-loftr-scored-proposals-managed-20261010-audit.json'
assert not OUT.exists()
sha = lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
r = json.loads(REPORT.read_text())
assert r['status'] == 'complete_rust_geometry_diagnostic' and len(r['results']) == 4
assert all(sha(p) == h for p,h in r['pins'].items())
manifest = json.loads((T / 'manifest.json').read_text())
audits = []
for index, (row, result) in enumerate(zip(manifest, r['results'])):
    raw = json.loads((T / f'{index}-native-proposals.json').read_text())
    native = json.loads((T / f'{index}-scored-proposals-managed-qualified.json').read_text())
    all_points = []
    for side, key in enumerate(('keypoints0', 'keypoints1')):
        original = [(2560,1920),(1024,701)][side]
        inp = row['inputs'][side]
        scale = np.array(original)/np.array([inp['width'],inp['height']])
        all_points.append((np.array(raw[key])+.5)*scale-.5)
    selected = []
    for j in np.argsort(-np.array(raw['confidence']), kind='stable'):
        p,q = all_points[0][j],all_points[1][j]
        if not (0 <= p[0] < 2560 and 0 <= p[1] < 1920 and 0 <= q[0] < 1024 and 0 <= q[1] < 701):
            continue
        if all(np.sum((p-x[0])**2)>4 and np.sum((q-x[1])**2)>4 for x in selected):
            selected.append([p.tolist(),q.tolist()])
    # Multiplication/division ordering differs across implementations by a few
    # floating-point ulps; retain strict absolute original-pixel tolerance.
    assert np.array(selected).shape == np.array(native['points']).shape
    assert np.allclose(np.array(selected),np.array(native['points']),rtol=0,atol=2e-12)
    matrix = np.array(native['matrix'])
    inverse = np.linalg.inv(matrix)
    def apply(h, point):
        p = h @ np.array([*point,1.])
        return p[:2]/p[2]
    forward, bidirectional = [],[]
    source_tolerance = 2*np.hypot(2560,1920)/np.hypot(1024,701)
    for j,(p,q) in enumerate(selected):
        if np.sum((apply(matrix,p)-q)**2) <= 4:
            forward.append(j)
            if np.sum((apply(inverse,q)-p)**2) <= source_tolerance**2:
                bidirectional.append(j)
    assert forward == native['inliers']
    assert result['geometry_returncode'] == result['confirmation_returncode'] == 0
    assert native['hypotheses'] == result['hypotheses'] == 20000
    expected_peak = len(raw['confidence'])*40
    assert result['geometry_stderr'].strip() == f'proposal_memory_used=0 proposal_memory_peak={expected_peak}'
    assert expected_peak <= 256*1024
    e = result['evidence']
    assert e == json.loads(result['confirmation_stdout'])
    assert e['status']=='ok' and e['supported'] and min(e['anchors'])>=10
    assert e['anchors'] == [len(bidirectional)]*2
    assert 'memory_used=0 ' in result['confirmation_stderr']
    for a, direction in zip(e['anchors'],e['directions']):
        assert direction['sites']==a*25 and direction['valid_sites']>=direction['sites']*.3
        assert direction['informative_pairs']>=1000
        assert direction['agreeing_pairs']>=direction['informative_pairs']*.9
    audits.append(dict(side=row['side'],clahe=row['clahe'],selected_points=len(selected),proposal_memory_peak=expected_peak,forward_inliers=len(forward),bidirectional_inliers=len(bidirectional)))
OUT.write_text(json.dumps(dict(status='verified_native_geometry_and_pixel_evidence_predicates',report=str(REPORT),report_sha256=sha(REPORT),verifier_sha256=sha(__file__),pins_verified=len(r['pins']),variants=audits,scope='Independent NumPy selection/residual recomputation, native confirmation predicates and zero retained confirmation credit. Not independent original-pixel recomputation, native fixture decoding, model budget/cancellation, integrated automatic search or unrelated precision.'),indent=2)+'\n')
print(audits)
