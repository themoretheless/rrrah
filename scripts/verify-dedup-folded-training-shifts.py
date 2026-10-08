"""Independent shifted-model, native-point and training-only selection audit."""
import argparse
import hashlib
import json
import math
from pathlib import Path
from dedup_jpeg_domain import oriented_dimensions

parser = argparse.ArgumentParser()
parser.add_argument('report', type=Path)
parser.add_argument('output', type=Path)
args = parser.parse_args()
assert not args.output.exists()
digest = lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
r = json.loads(args.report.read_text())
assert r['status'] == 'complete_training_shift_diagnostic'
assert r['required_models'] == 6 and len(r['rows']) == 54
assert all(digest(k) == v for k, v in r['input_hashes'].items())
base = args.report.parent
geometry = json.loads((base / 'dedup-folded-201702-witness-geometry.json').read_text())
audit = json.loads((base / 'dedup-folded-201702-witness-geometry-audit.json').read_text())
assert audit['status'] == 'verified_native_witness_domains' and audit['models'] == 6
assert audit['input_hashes'][str(base / 'dedup-folded-201702-witness-geometry.json')] == digest(base / 'dedup-folded-201702-witness-geometry.json')
points_path = base / 'dedup-folded-201702-regional-points.txt'
assert geometry['input_hashes'][str(points_path)] == digest(points_path)
points = [list(map(float, line.split())) for line in points_path.read_text().splitlines()]
models = [row for row in geometry['evidence']['regions'] if row['matrix'] is not None]
diagnostic = json.loads((base / 'dedup-folded-training-diagnostic.json').read_text())
prepared = next(Path(p) for p in r['input_hashes'] if Path(p).name == 'prepared.json')
manifest = json.loads(prepared.read_text())
left = next(Path(p) for p in r['input_hashes'] if Path(p).name == '201700.jpg')
item = next(v for v in manifest['images']['original'] if v['filename'] == left.name)
assert digest(left) == item['source_sha256']
sw, sh = oriented_dimensions(left, item['source_size'])
shifts = [(0,0), *[(x,y) for y in [-1,0,1] for x in [-1,0,1] if (x,y) != (0,0)]]
summaries = []
selected = []
for index, native in enumerate(models):
    values = list(map(float, Path(diagnostic['rows'][index+1]['input']).read_text().split()))
    sx, sy, w, h = map(int, values[9:13])
    x0, y0 = max(0,sx-32), max(0,sy-32)
    source = [x0,y0,min(sw,sx+w+32)-x0,min(sh,sy+h+32)-y0]
    target = list(map(int, values[13:]))
    baseline = None
    eligible = []
    for offset, shift in enumerate(shifts):
        row_index = index*9 + offset
        row = r['rows'][row_index]
        assert row['model_index'] == index and row['shift'] == list(shift)
        assert row['source_domain'] == source and row['target_domain'] == target
        h = native['matrix']
        expected = [[h[j][k]+shift[j]*h[2][k] for k in range(3)] for j in range(2)] + [h[2]]
        assert row['matrix'] == expected
        inliers = []
        for p in native['point_indices']:
            sx,sy,tx,ty = points[p]
            denominator = expected[2][0]*sx+expected[2][1]*sy+expected[2][2]
            assert denominator != 0
            projected = [(expected[j][0]*sx+expected[j][1]*sy+expected[j][2])/denominator for j in range(2)]
            if math.dist(projected,[tx,ty]) <= 2.:
                inliers.append(p)
        assert row['inliers'] == inliers
        if len(inliers) < 10:
            assert row['status'] == 'insufficient_geometric_inliers' and 'evidence' not in row
            continue
        path = Path(row['input'])
        assert digest(path) == row['input_sha256']
        assert list(map(float,path.read_text().split())) == [v for axis in expected for v in axis]+source+target
        assert row['returncode'] == 0
        evidence = row['evidence']
        assert evidence == json.loads(row['stdout'])
        assert evidence['status'] == row['status'] and evidence['managed_used'] == 0
        assert 0 <= evidence['managed_peak'] <= 512*1024*1024
        assert evidence['footprints'] == ['target','source']
        if row['status'] != 'ok':
            assert row['status'] == 'refusal:Region(Color(Bounds))' and evidence['directions'] == []
            continue
        assert len(evidence['directions']) == 2
        for d, domain, reads in zip(evidence['directions'],[target,source],[5,8]):
            assert d['radius'] == 8
            assert 1000 <= d['training_samples'] <= 100000
            assert 1000 <= d['samples'] <= 100000
            assert d['training_samples']+d['samples'] <= domain[2]*domain[3]
            assert 0 <= d['training_matched'] <= d['training_samples']
            assert 0 <= d['matched'] <= d['samples']
            assert d['pixel_reads'] <= domain[2]*domain[3]*289*reads
            assert all(math.isfinite(d[key]) and d[key]>=0 for key in ['training_squared_error','heldout_squared_error'])
        f = evidence['directions'][0]
        counts = [f['training_samples'],f['samples']]
        if offset == 0:
            baseline = counts
        assert row['same_forward_counts'] == (counts == baseline)
        if counts == baseline:
            loss = f['training_squared_error']/f['training_samples']
            assert row['training_loss'] == loss
            eligible.append((loss,abs(shift[0])+abs(shift[1]),*shift,row_index))
    if eligible:
        choice = min(eligible)
        selected.append({'model_index':index,'row_index':choice[-1], 'training_loss':choice[0], 'shift':list(choice[2:4])})
        row = r['rows'][choice[-1]]
        fractions = [d['matched']/d['samples'] for d in row['evidence']['directions']]
        summaries.append({'model_index':index,'shift':row['shift'],'heldout_fractions':fractions,'supported':all(f>=.9 for f in fractions)})
assert r['selections'] == selected
assert all(digest(k) == v for k,v in r['input_hashes'].items())
args.output.write_text(json.dumps({'status':'verified_training_only_shift_diagnostic', 'cases':54,'selected_models':len(selected),'supports':sum(row['supported'] for row in summaries),'summary':summaries,'input_hashes':{str(p):digest(p) for p in [args.report,Path(__file__),Path(__file__).with_name('dedup_jpeg_domain.py')]},'scope':'All nine shifts per native model, fixed rectangles, exact native-point residual membership, finite bounded diagnostics and independent training-only selection arithmetic. Equal forward sample counts do not independently prove identical per-pixel membership; no pixel resampling/fit oracle or automatic fold recovery.'},indent=2)+'\n')
