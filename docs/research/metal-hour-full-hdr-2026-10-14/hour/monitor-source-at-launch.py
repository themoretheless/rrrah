#!/usr/bin/env python3
"""Monitor the existing full-size Metal collection test without hiding failures."""
import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import re
import runpy
import signal
import subprocess
import time

ROOT = Path(__file__).resolve().parents[1]
TEST = 'long_lived_large_collection_cache_preserves_swap_and_metal_frames'


def save(path, value):
    temporary = path.with_suffix('.tmp')
    temporary.write_text(json.dumps(value, indent=2) + '\n')
    temporary.replace(path)


def stop_owned_child(child):
    if child.poll() is None:
        child.terminate()
        try:
            child.wait(timeout=15)
        except subprocess.TimeoutExpired:
            child.kill()
            child.wait(timeout=5)


def progress(log):
    with log.open('rb') as stream:
        stream.seek(max(0, log.stat().st_size - 4096))
        text = stream.read().decode('utf-8', errors='replace')
    rows = re.findall(r'long-lived collection cycles=(\d+); elapsed=([\d.]+)s', text)
    return {'cycles': int(rows[-1][0]), 'test_elapsed_seconds': float(rows[-1][1])} if rows else {}


def main():
    def interrupted(signum, _frame):
        raise InterruptedError(f'monitor signal {signum}')

    signal.signal(signal.SIGTERM, interrupted)
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--scratch', type=Path, required=True)
    parser.add_argument('--seconds', type=int, default=3600)
    parser.add_argument('--rss-limit-mib', type=int, default=12288)
    parser.add_argument('--sample-seconds', type=float, default=5)
    args = parser.parse_args()
    if not 10 <= args.seconds <= 86400 or args.rss_limit_mib <= 0 or not 0 < args.sample_seconds <= 60:
        parser.error('duration 10..86400, positive RSS limit, sample interval (0,60] required')
    binary, output, scratch = (p.resolve() for p in (args.binary, args.output, args.scratch))
    if not binary.is_file() or not os.access(binary, os.X_OK):
        parser.error('test binary must exist and be executable')
    output.mkdir(parents=True, exist_ok=True)
    if (output / 'state.json').exists() or (output / 'test.log').exists():
        parser.error('output already has a run; inspect its process/status instead of restarting')
    scratch.mkdir(parents=True, exist_ok=False)
    env = dict(os.environ, RRRAH_GPU_BACKEND='metal', RRRAH_GPU_VENDOR='any',
               RRRAH_GPU_OPTIONAL='0', RRRAH_METAL_COLLECTION_SECONDS=str(args.seconds),
               RRRAH_METAL_COLLECTION_FULL_HDR='1', TMPDIR=str(scratch))
    command = [str(binary), TEST, '--exact', '--ignored', '--nocapture', '--test-threads=1']
    state = dict(status='starting', monitor_pid=os.getpid(), binary=str(binary), command=command,
                 binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),
                 monitor_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
                 requested_seconds=args.seconds, full_hdr_output=[4000, 3000], frames=8,
                 managed_root_limit=576000000, managed_queue_limit=192000000,
                 swap_limit=1537048576, scratch=str(scratch),
                 rss_guard_bytes=args.rss_limit_mib * 1024 * 1024,
                 rss_guard_consecutive_samples=3, sample_seconds=args.sample_seconds,
                 started_utc=datetime.now(timezone.utc).isoformat(), rss_samples=0)
    sources = [ROOT / 'Cargo.lock', ROOT / 'crates/rrrah-app/tests/ttl_swap_metal.rs',
               ROOT / 'crates/rrrah-gpu/tests/common/mod.rs']
    for crate in ('rrrah-cache', 'rrrah-core', 'rrrah-gpu', 'rrrah-memory', 'rrrah-swap'):
        sources.extend((ROOT / 'crates' / crate / 'src').rglob('*.rs'))
    sources.extend((ROOT / 'crates/rrrah-gpu/shaders').glob('*.wgsl'))
    save(output / 'sources.json', {str(p.relative_to(ROOT)): hashlib.sha256(p.read_bytes()).hexdigest()
                                 for p in sources})
    save(output / 'state.json', state)
    child = None
    started = time.monotonic()
    try:
        with (output / 'test.log').open('wb') as log, (output / 'rss.jsonl').open('w') as samples:
            child = subprocess.Popen(command, cwd=ROOT, env=env, stdin=subprocess.DEVNULL,
                                     stdout=log, stderr=subprocess.STDOUT)
            state.update(status='running', child_pid=child.pid)
            save(output / 'state.json', state)
            print(json.dumps({'monitor_pid': os.getpid(), 'child_pid': child.pid,
                              'state': str(output / 'state.json')}), flush=True)
            over_limit = failures = 0
            while child.poll() is None:
                row = dict(elapsed_seconds=time.monotonic() - started)
                try:
                    result = subprocess.run(['ps', '-o', 'rss=', '-p', str(child.pid)],
                                            check=True, capture_output=True, text=True, timeout=3)
                    row['rss_bytes'] = int(result.stdout.strip()) * 1024
                    failures = 0
                    over_limit = over_limit + 1 if row['rss_bytes'] > state['rss_guard_bytes'] else 0
                except (subprocess.SubprocessError, ValueError, OSError) as error:
                    if child.poll() is not None:
                        break
                    row['rss_unavailable'] = str(error)
                    failures += 1
                samples.write(json.dumps(row) + '\n')
                samples.flush()
                state.update(last_sample=row, rss_samples=state['rss_samples'] + 1,
                             elapsed_seconds=time.monotonic() - started,
                             last_progress=progress(output / 'test.log'))
                save(output / 'state.json', state)
                if over_limit >= 3 or failures >= 3:
                    state['stop_reason'] = 'rss_guard' if over_limit >= 3 else 'rss_monitor_unavailable'
                    stop_owned_child(child)
                    break
                time.sleep(args.sample_seconds)
            child.wait()
    except BaseException as error:
        state['stop_reason'] = f'{type(error).__name__}: {error}'
        if child is not None:
            stop_owned_child(child)
    finally:
        state.update(exit_code=child.returncode if child is not None else None,
                     elapsed_seconds=time.monotonic() - started,
                     ended_utc=datetime.now(timezone.utc).isoformat())
        text = (output / 'test.log').read_text(errors='replace') if (output / 'test.log').exists() else ''
        results = re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;', text)
        evidence = runpy.run_path(str(ROOT / 'scripts/qualify-viewer-storage-gpu.py'))['collection_evidence']
        valid = (state['exit_code'] == 0 and not state.get('stop_reason') and len(results) == 1
                 and results[0][1] == '0'
                 and evidence(text, args.seconds, True, int(results[0][0]), int(results[0][2])))
        state.update(status='passed' if valid else 'failed', workload_evidence_valid=valid)
        save(output / 'state.json', state)
        print(json.dumps({'status': state['status'], 'exit_code': state['exit_code'],
                          'stop_reason': state.get('stop_reason')}), flush=True)
    return 0 if state['status'] == 'passed' else 1


if __name__ == '__main__':
    raise SystemExit(main())
