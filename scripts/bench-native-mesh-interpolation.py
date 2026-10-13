#!/usr/bin/env python3
"""Serial ABBA comparison of bounded native triangle sampling executables."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import statistics
import subprocess

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--baseline', type=Path, required=True)
parser.add_argument('--candidate', type=Path, required=True)
parser.add_argument('--report', type=Path, required=True)
parser.add_argument('--blocks', type=int, default=4)
args = parser.parse_args()
if not 1 <= args.blocks <= 8:
    parser.error('--blocks must be in 1..8')
executables = {k: p.resolve(strict=True) for k, p in [('baseline', args.baseline), ('candidate', args.candidate)]}
rows = []
reference = None
for block in range(-1, args.blocks):
    kinds = ['baseline', 'candidate'] if block == -1 else ['baseline', 'candidate', 'candidate', 'baseline']
    for position, kind in enumerate(kinds):
        result = subprocess.run([str(executables[kind]), 'native_selected_mesh_interpolation_benchmark', '--ignored', '--nocapture'], capture_output=True, text=True, timeout=60)
        log = result.stdout + result.stderr
        path = args.report.with_name(f'{args.report.stem}-{block}-{position}-{kind}.log')
        with path.open('x') as output:
            output.write(log)
        match = re.search(r'NATIVE_MESH_INTERPOLATION_BENCH iterations=(\d+) elapsed_ns=(\d+) work=(\d+) checksum=(\[[^\n]+\])', log)
        if result.returncode != 0 or match is None:
            raise RuntimeError(f'invalid native benchmark: {path}')
        iterations, elapsed, work = map(int, match.groups()[:3])
        checksum = json.loads(match.group(4))
        signature = (iterations, work, checksum)
        if iterations != 5_000_000 or work != 5 * iterations or elapsed <= 0:
            raise RuntimeError(f'work/iteration contract changed: {path}')
        if reference is None:
            reference = signature
        if reference != signature:
            raise RuntimeError(f'native outputs changed: {path}')
        row = dict(block=block, position=position, kind=kind, warmup=block == -1, iterations=iterations, elapsed_ns=elapsed, work=work, checksum=checksum, log=str(path.resolve()), sha256=hashlib.sha256(path.read_bytes()).hexdigest())
        rows.append(row)
        print(json.dumps(row), flush=True)
ratios = []
for block in range(args.blocks):
    totals = {kind: sum(r['elapsed_ns'] for r in rows if r['block'] == block and r['kind'] == kind) for kind in executables}
    ratios.append(totals['baseline'] / totals['candidate'])
report = dict(scope='Bounded native selected-triangle sampler; single synthetic CMYK triangle, varying points, unchanged 25M work debit per run; not whole-page/viewer throughput', blocks=args.blocks, rows=rows, block_ratios=ratios, median_baseline_over_candidate=statistics.median(ratios), exact_checksums=True, executable_sha256={kind: hashlib.sha256(path.read_bytes()).hexdigest() for kind, path in executables.items()}, script_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest())
with args.report.open('x') as output:
    json.dump(report, output, indent=2)
    output.write('\n')
