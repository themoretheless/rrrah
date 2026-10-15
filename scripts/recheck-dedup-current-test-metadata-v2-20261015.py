#!/usr/bin/env python3
from pathlib import Path
import hashlib
import json
import os
import re
import subprocess

ROOT = Path(__file__).resolve().parents[1]
RESEARCH = ROOT / 'docs/research'
PRIOR_PATH = RESEARCH / 'dedup-current-test-metadata-v1-20261015.json'
prior = json.loads(PRIOR_PATH.read_text())
assert len(prior['failed_targets']) == 9 and not prior['source_changes']
WORK = Path('/private/tmp/rrrah-current-test-metadata-v2-20261015')
WORK.mkdir(exist_ok=False)
OUTPUT = RESEARCH / 'dedup-current-test-metadata-v2-20261015.json'
assert not OUTPUT.exists()

def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()

def save():
    temporary = OUTPUT.with_suffix('.tmp')
    temporary.write_text(json.dumps(report, indent=2) + '\n')
    temporary.replace(OUTPUT)

expected = {f'crates/rrrah-dedup/tests/{name}.rs' for name in prior['failed_targets']}
changed = {path for path, digest in prior['source_pins'].items() if sha(ROOT / path) != digest}
assert changed == expected, changed
for path, digest in prior['dependency_pins'].items():
    assert sha(path) == digest
backup = Path('/private/tmp/rrrah-strict-test-warning-cleanup-20261015')
source_pins = {path: sha(ROOT / path) for path in prior['source_pins']}
for path in expected:
    before = (backup / Path(path).name).read_text()
    after = (ROOT / path).read_text()
    pattern = r'\bfn\s+(\w+)\s*(?:<[^\n]*>)?\s*\('
    assert re.findall(pattern, before) == re.findall(pattern, after)
    assert before.count('#[test]') == after.count('#[test]')
rows = [row for row in prior['rows'] if row['returncode'] == 0]
assert len(rows) == 147
for row in rows:
    assert sha(row['log']) == row['log_sha256']
production_metadata = Path(rows[0]['command'][rows[0]['command'].index('-o') + 1])
production_pin = sha(production_metadata)
report = {
    'status': 'running', 'producer_sha256': sha(__file__),
    'prior_report': str(PRIOR_PATH.relative_to(ROOT)), 'prior_report_sha256': sha(PRIOR_PATH),
    'source_pins': source_pins, 'changed_test_sources': sorted(expected),
    'dependency_pins': prior['dependency_pins'],
    'production_metadata_sha256': production_pin,
    'reused_passed_rows': rows, 'rerun_rows': [],
    'planned_targets': prior['planned_targets'],
    'required_feature_skips': prior['required_feature_skips'],
    'scope': '147 prior successful production-library/unit/integration metadata checks retained; only nine changed warning-failing integrations rerun. All other source/manifests/dependency pins unchanged, same current production metadata and strict compiler flags. Matching decode/raster targets only; no execution, Clippy, actual current decoder/Core Cargo rebuild, allfeatures, workspace, platform or performance proof. Shared captured legacy fixture data retained with narrow expected-dead-code annotations.'
}
save()
environment = os.environ.copy()
environment['CARGO_MANIFEST_DIR'] = str(ROOT / 'crates/rrrah-dedup')
environment['RRRAH_BUILT_RECIPE_ID'] = 'diagnostic-current-test-metadata-not-for-cache'
for row in prior['rows']:
    if row['returncode'] == 0:
        continue
    command = list(row['command'])
    command[command.index('-o') + 1] = str(WORK / (row['target'] + '.rmeta'))
    log = WORK / (row['target'] + '.log')
    with log.open('x') as stream:
        child = subprocess.Popen(command, cwd=ROOT, env=environment, stdout=stream, stderr=subprocess.STDOUT)
        report['active'] = {'target': row['target'], 'child_pid': child.pid}
        save()
        code = child.wait()
    report['rerun_rows'].append({'target': row['target'], 'command': command, 'returncode': code, 'log': str(log), 'log_sha256': sha(log)})
    save()
changes = [path for path, digest in source_pins.items() if sha(ROOT / path) != digest]
assert sha(production_metadata) == production_pin
for path, digest in prior['dependency_pins'].items():
    assert sha(path) == digest
failed = [row['target'] for row in report['rerun_rows'] if row['returncode'] != 0]
report.update(status='strict_metadata_passed' if not failed and not changes else 'terminal_failures_or_drift', active=None, failed_targets=failed, source_changes=changes)
save()
print('Strict metadata:', len(rows) + len(report['rerun_rows']), 'accounted;', len(failed), 'failed')
raise SystemExit(bool(failed or changes))
