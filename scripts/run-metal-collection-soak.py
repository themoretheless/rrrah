#!/usr/bin/env python3
"""Monitor Metal, mixed real-worker or delayed-swap tests without hiding failures."""
import argparse
import ast
from datetime import datetime, timezone
import hashlib
import json
import math
import os
from pathlib import Path
import re
import runpy
import signal
import subprocess
import time

ROOT = Path(__file__).resolve().parents[1]
TEST = 'long_lived_large_collection_cache_preserves_swap_and_metal_frames'
SLOW_TEST = 'sustained_float_hdr_delayed_swap_pressure_cancel_and_retry'
WORKER_TEST = 'folder_worker_qualification::real_worker_large_collection_pressure_soak'


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


def progress(log, suite='metal'):
    with log.open('rb') as stream:
        stream.seek(max(0, log.stat().st_size - 4096))
        text = stream.read().decode('utf-8', errors='replace')
    pattern = (r'long-lived collection (?:complete: )?cycles=(\d+); elapsed=([\d.]+)s' if suite == 'metal'
               else r'large float delayed (?:progress|swap): cycles=(\d+); (?:elapsed|seconds)=([\d.]+)')
    if suite == 'worker-mixed':
        pattern = r'large soak selections=(\d+);? seconds=([\d.]+)'
    rows = re.findall(pattern, text)
    return {'cycles': int(rows[-1][0]), 'test_elapsed_seconds': float(rows[-1][1])} if rows else {}


def worker_evidence(text, seconds):
    adapters = re.findall(r'^.*large soak adapter: (.+)$', text, re.MULTILINE)
    finals = re.findall(r'^large real worker soak PASS: (.+)$', text, re.MULTILINE)
    windows = re.findall(r'^dynamic prefetch windows=(.+) selections=(.+)$', text, re.MULTILINE)
    if len(adapters) != 1 or not adapters[0].startswith('Metal ') or len(finals) != 1 or len(windows) != 1:
        return False
    try:
        row = dict(re.findall(r'(\w+)=(\S+)', finals[0]))
        expected = dict(sources=64, frame_bytes=16777216, fixed_pressure=67108864,
                        root_limit=134217728, final=0)
        if any(int(row[k]) != v for k, v in expected.items()):
            return False
        total = int(row['selections'])
        elapsed = float(row['seconds'])
        actual_windows = ast.literal_eval(windows[0][0])
        counts = ast.literal_eval(windows[0][1])
        expected_windows = [(0, 0), (3, 0), (0, 3), (1, 2), (2, 5),
                            (0, 2**64-1), (2**64-1, 2**64-1)]
        return (actual_windows == expected_windows and len(counts) == 7
                and all(type(n) is int and n >= 16 for n in counts) and sum(counts) == total
                and total >= 256 and math.isfinite(elapsed) and elapsed >= seconds
                and 0 < int(row['root_peak']) <= 134217728
                and 0 < int(row['largest_settled']) <= 100663296
                and 0 < int(row['nearest_retained']) <= total - counts[0]
                and int(row['farther_refused']) > 0 and row['dimensions'] == '1024x1024'
                and all(row[k] == 'true' for k in ('prefetch', 'dynamic_window', 'mixed_ppm_tga_bmp'))
                and 'swap_objects<=3 swap_bytes<=67108864' in finals[0]
                and 'exact Metal SDR/HDR every selection' in finals[0])
    except (KeyError, ValueError, TypeError, SyntaxError):
        return False


def injected_io_evidence(text, chunk_bytes):
    keys = ('read_calls', 'read_bytes', 'max_read_request', 'write_calls', 'write_bytes',
            'max_write_request', 'completed_spills', 'completed_restores')
    pattern = r'large soak injected swap codec totals: ' + ' '.join(key + r'=(\d+)' for key in keys)
    matches = re.findall(pattern, text)
    if len(matches) != 1:
        return None
    values = dict(zip(keys, map(int, matches[0])))
    if any(value <= 0 for value in values.values()):
        return None
    for direction, completed in [('read', 'completed_restores'), ('write', 'completed_spills')]:
        calls = values[direction + '_calls']
        size = values[direction + '_bytes']
        maximum = values['max_' + direction + '_request']
        if not (maximum <= chunk_bytes and calls >= values[completed]
                and values[completed] * 16777216 <= size <= calls * maximum):
            return None
    return values


def main():
    def interrupted(signum, _frame):
        raise InterruptedError(f'monitor signal {signum}')

    signal.signal(signal.SIGTERM, interrupted)
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--scratch', type=Path, required=True)
    parser.add_argument('--seconds', type=int, default=3600)
    parser.add_argument('--suite', choices=('metal', 'float-delayed', 'worker-mixed'), default='metal')
    parser.add_argument('--swap-delay-ms', type=int,
                        help='worker-only qualification delay per swap codec I/O call')
    parser.add_argument('--swap-chunk-bytes', type=int,
                        help='worker-only maximum swap codec I/O request, 1..65536')
    parser.add_argument('--rss-limit-mib', type=int, default=12288)
    parser.add_argument('--sample-seconds', type=float, default=5)
    args = parser.parse_args()
    if not 10 <= args.seconds <= 86400 or args.rss_limit_mib <= 0 or not 0 < args.sample_seconds <= 60:
        parser.error('duration 10..86400, positive RSS limit, sample interval (0,60] required')
    worker = args.suite == 'worker-mixed'
    if (args.swap_delay_ms is None) != (args.swap_chunk_bytes is None):
        parser.error('swap delay and chunk limit must be specified together')
    if args.swap_delay_ms is not None and (not worker or not 1 <= args.swap_delay_ms <= 1000
                                           or not 1 <= args.swap_chunk_bytes <= 65536):
        parser.error('swap injection requires worker-mixed, delay 1..1000 ms, chunk 1..65536 bytes')
    if worker and args.seconds < 60:
        parser.error('worker duration must be at least 60 seconds')
    binary, output, scratch = (p.resolve() for p in (args.binary, args.output, args.scratch))
    if not binary.is_file() or not os.access(binary, os.X_OK):
        parser.error('test binary must exist and be executable')
    output.mkdir(parents=True, exist_ok=True)
    if (output / 'state.json').exists() or (output / 'test.log').exists():
        parser.error('output already has a run; inspect its process/status instead of restarting')
    scratch.mkdir(parents=True, exist_ok=False)
    metal = args.suite == 'metal'
    env = dict(os.environ, TMPDIR=str(scratch))
    if worker:
        for name in ('RRRAH_QUALIFY_SWAP_DELAY_MS', 'RRRAH_QUALIFY_SWAP_CHUNK_BYTES'):
            env.pop(name, None)
        if args.swap_delay_ms is not None:
            env.update(RRRAH_QUALIFY_SWAP_DELAY_MS=str(args.swap_delay_ms),
                       RRRAH_QUALIFY_SWAP_CHUNK_BYTES=str(args.swap_chunk_bytes))
        env.update(RRRAH_GPU_BACKEND='metal', RRRAH_GPU_VENDOR='any', RRRAH_GPU_OPTIONAL='0',
                   RRRAH_SOAK_METAL='1', RRRAH_SOAK_PREFETCH='1', RRRAH_SOAK_MIXED='1',
                   RRRAH_SOAK_DYNAMIC_WINDOW='1', RRRAH_SOAK_SECONDS=str(args.seconds))
    elif metal:
        env.update(RRRAH_GPU_BACKEND='metal', RRRAH_GPU_VENDOR='any', RRRAH_GPU_OPTIONAL='0',
                   RRRAH_METAL_COLLECTION_SECONDS=str(args.seconds), RRRAH_METAL_COLLECTION_FULL_HDR='1')
    else:
        env.update(RRRAH_STRESS_SECONDS=str(args.seconds), RRRAH_SLOW_SWAP_PROGRESS='1')
    command = [str(binary), WORKER_TEST if worker else TEST if metal else SLOW_TEST,
               '--exact', '--ignored', '--nocapture', '--test-threads=1']
    state = dict(status='starting', monitor_pid=os.getpid(), binary=str(binary), command=command,
                 binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),
                 monitor_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
                 suite=args.suite, requested_seconds=args.seconds,
                 full_hdr_output=[4000, 3000] if metal else None, frames=8 if metal else None,
                 dimensions=[4000, 3000] if metal else [2048, 2048],
                 managed_root_limit=576000000 if metal else 134217728,
                 managed_queue_limit=192000000 if metal else 67108864,
                 swap_limit=1537048576 if metal else 134217856, scratch=str(scratch),
                 io_delay_ms=None if metal else 2, max_io_request_bytes=None if metal else 65536,
                 rss_guard_bytes=args.rss_limit_mib * 1024 * 1024,
                 rss_guard_consecutive_samples=3, sample_seconds=args.sample_seconds,
                 # Allocator diagnostics can materially change RSS without changing
                 # the binary; preserve them so such a run cannot look like defaults.
                 allocator_environment={key: value for key, value in env.items()
                                        if key.startswith('Malloc')},
                 started_utc=datetime.now(timezone.utc).isoformat(), rss_samples=0)
    if worker:
        state.update(full_hdr_output=[63, 65], frames=64, dimensions=[1024, 1024],
                     managed_root_limit=134217728, managed_queue_limit=16777216,
                     swap_limit=67108864, fixed_pressure_bytes=67108864,
                     restore_limit=33554432, io_delay_ms=args.swap_delay_ms,
                     max_io_request_bytes=args.swap_chunk_bytes,
                     io_injection_scope='swap_payload_codec' if args.swap_delay_ms is not None else None)
    sources = [ROOT / 'Cargo.lock', ROOT / 'crates/rrrah-app/Cargo.toml',
               ROOT / 'crates/rrrah-cache/Cargo.toml', ROOT / ('crates/rrrah-app/tests/ttl_swap_metal.rs' if metal
                                         else 'crates/rrrah-cache/tests/sustained_swap.rs'),
               ROOT / 'crates/rrrah-gpu/tests/common/mod.rs']
    if worker:
        sources.extend((ROOT / 'crates/rrrah-app/src').rglob('*.rs'))
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
                             last_progress=progress(output / 'test.log', args.suite))
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
        if (output / 'test.log').exists():
            state['last_progress'] = progress(output / 'test.log', args.suite)
        results = re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;', text)
        valid = False
        try:
            validators = runpy.run_path(str(ROOT / 'scripts/qualify-viewer-storage-gpu.py'))
            valid = (state['exit_code'] == 0 and not state.get('stop_reason')
                     and results == [('1', '0', '0')]
                     and (worker_evidence(text, args.seconds) if worker else validators['collection_evidence'](text, args.seconds, True, 1, 0) if metal
                          else validators['large_float_storage_evidence'](text, args.seconds)))
        except Exception as error:
            state['validation_error'] = f'{type(error).__name__}: {error}'
        if worker and args.swap_delay_ms is not None:
            injections = re.findall(r'large soak injected swap codec I/O: delay_ms=(\d+) chunk_bytes=(\d+)', text)
            state['io_injection_confirmed'] = injections == [(str(args.swap_delay_ms), str(args.swap_chunk_bytes))]
            if not state['io_injection_confirmed']:
                valid = False
                state['validation_error'] = 'requested swap I/O injection was not confirmed by the worker'
            state['io_totals'] = injected_io_evidence(text, args.swap_chunk_bytes)
            state['io_directions_confirmed'] = state['io_totals'] is not None
            if not state['io_directions_confirmed']:
                valid = False
                state['validation_error'] = 'completed swap I/O directions or bounded byte totals were not confirmed'
        state.update(status='passed' if valid else 'failed', workload_evidence_valid=valid)
        save(output / 'state.json', state)
        print(json.dumps({'status': state['status'], 'exit_code': state['exit_code'],
                          'stop_reason': state.get('stop_reason')}), flush=True)
    return 0 if state['status'] == 'passed' else 1


if __name__ == '__main__':
    raise SystemExit(main())
