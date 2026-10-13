#!/usr/bin/env python3
"""Frozen CPU LoFTR controls for projected candidates and original-RGBA proof."""
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import time

ROOT = Path(__file__).resolve().parents[1]
D = ROOT / 'docs/research'
MANIFEST = D / 'dedup-projected-loftr-identity-snapshot-20261011.json'
FIXTURES = D / 'dedup-projected-loftr-fixtures-20261011.json'
OUT = D / 'dedup-projected-loftr-identity-native-20261011.json'
BASE = Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays')
ARCHIVE = BASE / 'projected-loftr-identity-qualified-20261011'
RUNTIME = Path('/Users/themoretheless/.codex/tmp/rrrah-loftr-native-runtime-20261010')
TORCHLIB = RUNTIME / 'lib/python3.12/site-packages/torch/lib'
WEIGHTS = BASE / 'learned-matching-models/loftr-outdoor-native-20261010.safetensors'

def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()

def main():
    assert not OUT.exists() and not ARCHIVE.exists()
    ARCHIVE.mkdir()
    source = json.loads(MANIFEST.read_text())
    fixture = json.loads(FIXTURES.read_text())
    work = Path(fixture['work'])
    def verify_snapshot():
        assert all(sha(Path(source['snapshot']) / p) == h for p, h in source['file_hashes'].items())
    verify_snapshot()
    pins = {str(p): sha(p) for p in [MANIFEST, FIXTURES, Path(__file__), D / 'dedup-projected-loftr-identity-build-20261011.log', D / 'dedup-loftr-native-20261010-runtime-pins.json', WEIGHTS]}
    pins.update(fixture['fixture_pins'])
    pins.update(fixture['source_pins'])
    runtime = json.loads((D / 'dedup-loftr-native-20261010-runtime-pins.json').read_text())
    pins.update({p: h for p, h in runtime['pins'].items() if str(TORCHLIB) in p})
    assert sum(str(TORCHLIB) in p for p in pins) == 7
    for name in ['projected_files', 'projected_input_file']:
        original = ROOT / 'target/debug' / name
        archive = ARCHIVE / name
        shutil.copy2(original, archive)
        assert sha(original) == sha(archive)
        pins[str(archive)] = sha(archive)
    report = {'status': 'running', 'producer_pid': os.getpid(), 'scope': __doc__,
              'snapshot': str(MANIFEST.relative_to(ROOT)), 'pins': pins, 'preparation': [], 'results': [],
              'policy': {'pixel_tolerance': 0.05, 'minimum_coverage': 0.99, 'minimum_agreement': 0.999,
                         'projection': {'recipe': 'CompositeReinhardV1', 'exposure': 1, 'background': [0, 0, 0]},
                         'max_side': 320, 'cumulative_pixel_work_cap': 4000000, 'maximum_pixel_mappings': 2, 'threads': 2, 'seed': 173, 'managed_budget': 512 * 1024 * 1024},
              'limits': ['Synthetic textured controls, not arbitrary HDR/alpha corpus qualification',
                         'No automatic projected tile/portfolio search', 'Torch tensor/workspace RSS not admitted',
                         'Cooperative cancellation, no individual-kernel preemption']}
    def save():
        temp = OUT.with_suffix('.json.tmp')
        temp.write_text(json.dumps(report, indent=2) + '\n')
        temp.replace(OUT)
    def verify_pins():
        assert all(sha(p) == h for p, h in pins.items())
    def run(binary, args):
        started = time.monotonic()
        process = subprocess.run(['/usr/bin/time', '-l', '/usr/bin/env', f'DYLD_LIBRARY_PATH={TORCHLIB}', str(ARCHIVE / binary), *map(str, args)], capture_output=True, text=True)
        row = {'returncode': process.returncode, 'stdout': process.stdout, 'stderr': process.stderr,
               'elapsed_seconds': time.monotonic() - started}
        rss = re.search(r'(\d+)\s+maximum resident set size', process.stderr)
        if rss:
            row['maximum_rss_bytes_macos'] = int(rss.group(1))
        if process.returncode == 0:
            row['evidence'] = json.loads(process.stdout)
        return row
    save()
    try:
        verify_pins()
        gray = {}
        for name in ['hdr.pfm', 'hdr-copy.pfm', 'hdr-collision.pfm', 'hdr-unrelated.pfm', 'alpha.qoi', 'alpha-copy.qoi', 'alpha-collision.qoi']:
            target = ARCHIVE / (name + '.gray')
            row = {'fixture': name, **run('projected_input_file', [work / name, target])}
            report['preparation'].append(row)
            save()
            assert row['returncode'] == 0 and row['evidence']['managed_used'] == 0, row
            assert target.stat().st_size == 320 * 240
            pins[str(target)] = sha(target)
            gray[name] = target.read_bytes()
            verify_pins()
        report['projected_byte_equalities'] = {
            'hdr_copy': gray['hdr.pfm'] == gray['hdr-copy.pfm'],
            'hdr_collision': gray['hdr.pfm'] == gray['hdr-collision.pfm'],
            'alpha_copy': gray['alpha.qoi'] == gray['alpha-copy.qoi'],
            'alpha_collision': gray['alpha.qoi'] == gray['alpha-collision.qoi'],
        }
        assert all(report['projected_byte_equalities'].values())
        save()
        cases = [('hdr_copy', 'hdr.pfm', 'hdr-copy.pfm', True),
                 ('hdr_collision', 'hdr.pfm', 'hdr-collision.pfm', False),
                 ('alpha_copy', 'alpha.qoi', 'alpha-copy.qoi', True),
                 ('alpha_collision', 'alpha.qoi', 'alpha-collision.qoi', False),
                 ('unrelated_hdr', 'hdr.pfm', 'hdr-unrelated.pfm', False)]
        for name, left, right, expected in cases:
            verify_pins()
            row = {'case': name, 'source': left, 'target': right, 'expected_supported': expected, 'status': 'running'}
            report['results'].append(row)
            save()
            row.update(run('projected_files', [work / left, work / right, WEIGHTS, report['policy']['pixel_tolerance']]))
            row['status'] = 'executed' if row['returncode'] == 0 else 'execution_failed'
            if row['returncode'] == 0:
                row['expected_outcome_matches'] = row['evidence']['supported'] == expected
                assert row['evidence']['managed_used'] == 0
            verify_pins()
            save()
            print(name, row.get('evidence', {}).get('supported'), row.get('expected_outcome_matches'), flush=True)
        verify_snapshot()
        verify_pins()
        report['all_expected_outcomes_match'] = all(r.get('expected_outcome_matches', False) for r in report['results'])
        report['status'] = 'complete_bounded_five_native_projected_controls_execution'
    except Exception as error:
        report.update(status='failed_native_projected_controls', error=str(error))
        raise
    finally:
        save()

if __name__ == '__main__':
    main()
