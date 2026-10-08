"""Alternating frozen native processes; identical anchor evidence, different color work."""
import hashlib
import json
import statistics
import subprocess
import time
from pathlib import Path

D = Path('docs/research')
T = Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays')
baseline = D / 'dedup-anchor-automatic-pixels-prefix-4.json'
row = json.loads(baseline.read_text())['results'][0]
executables = {'legacy': T / 'anchor-candidate-files-probe-qualified',
               'geometry_only': T / 'anchor-geometry-files-probe-qualified'}
source = T / 'original-resolution-negative-200201' / row['original']
target = T / 'original-resolution-strong-all' / row['query']
out = D / 'dedup-anchor-geometry-alternating.json'
assert not out.exists()
sha = lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
pins = {str(p): sha(p) for p in [*executables.values(), source, target, baseline, Path(__file__)]}
report = {'status': 'running', 'query': row['query'], 'required_runs': 12,
          'pins': pins, 'results': [],
          'scope': 'Six runs per frozen executable on one real pair; alternating order per round. Whole-process wall time under concurrent workloads. Exact anchor evidence equality; geometry-only omits legacy color diagnostics. No isolated kernel, collection, or corpus-wide performance claim.'}
def save():
    tmp = out.with_suffix('.tmp')
    tmp.write_text(json.dumps(report, indent=2) + '\n')
    tmp.replace(out)
save()
for round_index in range(6):
    order = ['legacy', 'geometry_only'] if round_index % 2 == 0 else ['geometry_only', 'legacy']
    for mode in order:
        assert all(sha(p) == digest for p, digest in pins.items())
        start = time.perf_counter()
        process = subprocess.run([str(executables[mode]), str(source), str(target),
                                  str(row['source_tolerance'])], capture_output=True, text=True)
        seconds = time.perf_counter() - start
        assert process.returncode == 0, process.stderr
        evidence = json.loads(process.stdout)
        assert evidence == row['evidence'], mode
        assert all(sha(p) == digest for p, digest in pins.items())
        report['results'].append({'round': round_index, 'mode': mode, 'seconds': seconds,
                                  'exact_evidence_equal': True})
        save()
report['median_seconds'] = {mode: statistics.median(r['seconds'] for r in report['results']
                                                  if r['mode'] == mode) for mode in executables}
report['observed_ratio'] = report['median_seconds']['legacy'] / report['median_seconds']['geometry_only']
report['status'] = 'complete'
save()
print(json.dumps(report['median_seconds']))
