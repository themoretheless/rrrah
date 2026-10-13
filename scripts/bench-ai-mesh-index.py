#!/usr/bin/env python3
"""Compare two native AI diagnostic executables in serial ABBA blocks."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import statistics
import subprocess
import time
import threading

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--baseline', type=Path, required=True)
parser.add_argument('--candidate', type=Path, required=True)
parser.add_argument('--report', type=Path, required=True)
parser.add_argument('--corpus', type=Path, required=True)
parser.add_argument('--blocks', type=int, default=2)
args = parser.parse_args()
if not 1 <= args.blocks <= 8:
    parser.error('--blocks must be in 1..8')
executables = {k: p.resolve(strict=True) for k, p in [('baseline', args.baseline), ('candidate', args.candidate)]}
env = {k: v for k, v in os.environ.items() if not k.startswith(('RRRAH_AI_', 'RRRAH_TRACE_'))}
env.update(RRRAH_AI_REPLAY_CORPUS=str(args.corpus.resolve(strict=True)), RRRAH_AI_SPATIAL_PIXELS='199,25;138,39')
rows = []
reference = None

def run(kind, block, position, warmup=False):
    global reference
    start = time.perf_counter()
    process = subprocess.Popen([str(executables[kind]), 'real_ai_spatial_scene_problem_pixel_diagnostic', '--ignored', '--nocapture'], env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
    watchdog = threading.Timer(60, lambda: process.kill() if process.poll() is None else None)
    watchdog.start()
    output = []
    preflight_seconds = None
    try:
        for line in process.stdout:
            output.append(line)
            if line.startswith('REAL_AI_SPATIAL_PREFLIGHT=Ok(())'):
                preflight_seconds = time.perf_counter() - start
        returncode = process.wait()
    finally:
        watchdog.cancel()
        watchdog.join()
        process.stdout.close()
        if process.poll() is None:
            process.kill()
            process.wait()
    elapsed = time.perf_counter() - start
    log = ''.join(output)
    suffix = 'warmup' if warmup else str(block)
    path = args.report.with_name(f'{args.report.stem}-{suffix}-{position}-{kind}.log')
    with path.open('x') as output:
        output.write(log)
    pixels = [line for line in log.splitlines() if line.startswith('REAL_AI_SPATIAL_PIXEL ')]
    fingerprints = [re.search(r'pixel=.*?deadline=', line).group(0) for line in pixels]
    valid = (returncode == 0 and preflight_seconds is not None and len(pixels) == 2 and all('completed=true' in line and 'deadline=false' in line for line in pixels) and 'REAL_AI_SPATIAL_PREFLIGHT=Ok(())' in log and 'REAL_AI_SPATIAL_FINAL_CREDIT=0' in log)
    if not valid:
        raise RuntimeError(f'invalid diagnostic run: {path}')
    if reference is None:
        reference = fingerprints
    if fingerprints != reference:
        raise RuntimeError(f'native color/alpha result changed: {path}')
    row = dict(kind=kind, block=block, position=position, warmup=warmup, elapsed_seconds=elapsed, preflight_seconds=preflight_seconds, replay_seconds=elapsed-preflight_seconds, log=str(path.resolve()), sha256=hashlib.sha256(path.read_bytes()).hexdigest())
    rows.append(row)
    print(json.dumps(row), flush=True)

for i, kind in enumerate(executables):
    run(kind, -1, i, True)
for block in range(args.blocks):
    for position, kind in enumerate(['baseline', 'candidate', 'candidate', 'baseline']):
        run(kind, block, position)
ratios = []
for block in range(args.blocks):
    totals = {kind: sum(r['elapsed_seconds'] for r in rows if not r['warmup'] and r['block'] == block and r['kind'] == kind) for kind in executables}
    ratios.append(totals['baseline'] / totals['candidate'])
replay_ratios = []
for block in range(args.blocks):
    totals = {kind: sum(r['replay_seconds'] for r in rows if not r['warmup'] and r['block'] == block and r['kind'] == kind) for kind in executables}
    replay_ratios.append(totals['baseline'] / totals['candidate'])
report = dict(scope='Serial two-pixel native AI capture/preflight/replay benchmark; not whole-page or normal-viewer throughput', blocks=args.blocks, rows=rows, block_baseline_over_candidate=ratios, median_baseline_over_candidate=statistics.median(ratios), block_replay_baseline_over_candidate=replay_ratios, median_replay_baseline_over_candidate=statistics.median(replay_ratios), phase_timing_boundary="Parent observed preflight stdout marker; replay includes conversion, teardown, and process exit", exact_native_results=True, final_credit_bytes=0, executable_sha256={kind: hashlib.sha256(path.read_bytes()).hexdigest() for kind, path in executables.items()}, script_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest())
with args.report.open('x') as output:
    json.dump(report, output, indent=2)
    output.write('\n')
