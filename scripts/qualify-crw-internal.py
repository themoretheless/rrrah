#!/usr/bin/env python3
"""Pinned CRW sensor, public EOS 10D routing and optional Metal acceptance."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--corpus', type=Path, required=True)
parser.add_argument('--tables', type=Path, required=True)
parser.add_argument('--target-dir', type=Path, required=True)
parser.add_argument('--report', type=Path, required=True)
parser.add_argument('--metal', action='store_true', help='Require the actual-GPU cache/TTL/swap integration test.')
args = parser.parse_args()
root = Path(__file__).resolve().parents[1]
manifest_path = root / 'tests/fixtures/crw-sensor-oracles.json'
manifest = json.loads(manifest_path.read_text())

def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

# Verify all required inputs before launching any native test.
cases = []
for case in manifest['cases']:
    camera, table = {1307: ('d30', 1), 2027: ('10d', 0)}[case['id']]
    source = args.corpus / f'rrrah-{camera}.crw'
    oracle = args.corpus / f'rrrah-{camera}-current-libraw.u16le'
    tables = args.tables / f'rrrah-crw-table-{table}.bin'
    expected_table = manifest['format_tables'][str(table)]['sha256']
    for path, digest, length in [(source, case['source_sha256'], case['source_bytes']),
                                 (oracle, case['pixels_sha256'], case['pixels_bytes']),
                                 (tables, expected_table, manifest['format_tables'][str(table)]['bytes'])]:
        if path.stat().st_size != length or sha(path) != digest:
            raise SystemExit(f'pinned input mismatch: {path}')
    cases.append((case, source, oracle, tables))

report = {'scope': 'Pinned D30/10D sensor plus public EOS 10D route acceptance',
          'manifest_sha256': sha(manifest_path), 'cases': [],
          'limitations': ['D30 sensor-only with external table-one; public route supports qualified EOS 10D bundled table-zero.',
                          'No universal CRW camera, physical display or live navigation qualification.']}
for case, source, oracle, tables in cases:
    environment = os.environ.copy()
    environment.update(RRRAH_CRW_SOURCE=str(source.resolve()), RRRAH_CRW_ORACLE=str(oracle.resolve()),
                       RRRAH_CRW_TABLES=str(tables.resolve()))
    command = ['cargo', 'test', '-p', 'rrrah-decode', '--lib', 'real_full_sensor_entropy',
               '--locked', '--target-dir', str(args.target_dir.resolve()), '--', '--ignored', '--nocapture']
    completed = subprocess.run(command, cwd=root, env=environment, capture_output=True, text=True, timeout=180)
    output = completed.stdout + completed.stderr
    log = args.report.with_name(args.report.stem + f'-{case["id"]}.log')
    log.write_text(output)
    passed = completed.returncode == 0 and '1 passed; 0 failed; 0 ignored' in output
    report['cases'].append({'id': case['id'], 'passed': passed, 'source_sha256': sha(source),
                            'oracle_sha256': sha(oracle), 'table_sha256': sha(tables),
                            'log': str(log), 'log_sha256': sha(log), 'command': command})
# The public route and allocation-pressure checks use bundled table-zero.
case, source, oracle, tables = next(row for row in cases if row[0]['id'] == 2027)
environment = os.environ.copy()
environment['RRRAH_CRW_SOURCE'] = str(source.resolve())
command = ['cargo', 'test', '-p', 'rrrah-decode', '--lib', 'crw::', '--locked',
           '--target-dir', str(args.target_dir.resolve()), '--', '--include-ignored']
completed = subprocess.run(command, cwd=root, env=environment, capture_output=True, text=True, timeout=180)
output = completed.stdout + completed.stderr
log = args.report.with_name(args.report.stem + '-public.log')
log.write_text(output)
report['public_route'] = {'passed': completed.returncode == 0 and '4 passed; 0 failed; 0 ignored' in output,
                          'command': command, 'log': str(log), 'log_sha256': sha(log)}
if args.metal:
    case, source, oracle, tables = next(row for row in cases if row[0]['id'] == 2027)
    environment = os.environ.copy()
    environment.update(RRRAH_CRW_SOURCE=str(source.resolve()), RRRAH_CRW_TABLES=str(tables.resolve()))
    command = ['cargo', 'test', '-p', 'rrrah', '--test', 'crw_swap_metal', '--locked',
               '--target-dir', str(args.target_dir.resolve()), '--', '--ignored', '--nocapture']
    completed = subprocess.run(command, cwd=root, env=environment, capture_output=True, text=True, timeout=300)
    output = completed.stdout + completed.stderr
    log = args.report.with_name(args.report.stem + '-metal.log')
    log.write_text(output)
    passed = (completed.returncode == 0 and '1 passed; 0 failed; 0 ignored' in output
              and 'CRW adapter: Metal ' in output)
    report['metal'] = {'passed': passed, 'command': command, 'log': str(log), 'log_sha256': sha(log)}
    report['limitations'] = ['Public EOS 10D router uses bundled table-zero; D30 remains sensor-only with external table-one.',
                            'Metal proof covers native-output preservation; no independent color or physical display oracle.']

args.report.write_text(json.dumps(report, indent=2) + '\n')
print(json.dumps({'cases': len(cases), 'passed': sum(row['passed'] for row in report['cases']), 'metal': report.get('metal', {}).get('passed')}))
if not report['public_route']['passed'] or not all(row['passed'] for row in report['cases']) or (args.metal and not report['metal']['passed']):
    raise SystemExit(1)
