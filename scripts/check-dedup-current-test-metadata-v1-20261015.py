#!/usr/bin/env python3
from pathlib import Path
import hashlib
import json
import os
import subprocess
import time

ROOT = Path(__file__).resolve().parents[1]
RESEARCH = ROOT / 'docs/research'
WORK = Path('/private/tmp/rrrah-current-test-metadata-v1-20261015')
WORK.mkdir(exist_ok=False)
OUTPUT = RESEARCH / 'dedup-current-test-metadata-v1-20261015.json'
assert not OUTPUT.exists()

def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()

def save():
    temporary = OUTPUT.with_suffix('.tmp')
    temporary.write_text(json.dumps(report, indent=2) + '\n')
    temporary.replace(OUTPUT)

prior_path = RESEARCH / 'dedup-current-strict-test-metadata-v1-20261015.json'
prior = json.loads(prior_path.read_text())
assert prior['returncode'] == 0
for path, digest in prior['dependency_pins'].items():
    assert sha(path) == digest
metadata_command = ['cargo', '+stable', 'metadata', '--no-deps', '--locked', '--offline', '--format-version', '1']
metadata = subprocess.run(metadata_command, cwd=ROOT, capture_output=True, text=True)
(WORK / 'cargo-metadata.stderr').write_text(metadata.stderr)
assert metadata.returncode == 0, metadata.stderr
packages = json.loads(metadata.stdout)['packages']
package = next(p for p in packages if p['name'] == 'rrrah-dedup')
features = {'default', 'raster', 'decode'}
targets = []
skipped = []
for target in package['targets']:
    if not set(target['kind']) & {'lib', 'test'}:
        continue
    if not set(target.get('required-features', [])).issubset(features):
        skipped.append({'name': target['name'], 'required_features': target['required-features']})
    else:
        targets.append(target)
assert any('lib' in t['kind'] for t in targets)
paths = list((ROOT / 'crates/rrrah-dedup').rglob('*.rs'))
paths += [ROOT / 'Cargo.toml', ROOT / 'Cargo.lock', ROOT / 'crates/rrrah-dedup/Cargo.toml', ROOT / '.cargo/config.toml']
source_pins = {str(p.relative_to(ROOT)): sha(p) for p in paths}
report = {
    'status': 'running', 'producer_pid': os.getpid(), 'producer_sha256': sha(__file__),
    'source_pins': source_pins, 'dependency_pins': prior['dependency_pins'],
    'prior_command_report_sha256': sha(prior_path), 'features': sorted(features),
    'planned_targets': [{'name': t['name'], 'kind': t['kind'], 'source': t['src_path']} for t in targets],
    'required_feature_skips': skipped, 'rows': [],
    'scope': 'Current dedup library and all matching integration-target source metadata with decode/raster, strict Rust warnings, real cached core/decoder dependencies. A current production library metadata artifact is used by integrations. No machine-code generation/test execution, Clippy, allfeatures/loftr, actual current decoder/Core Cargo build, complete workspace, platform or performance qualification. cfg-gated test functions can remain excluded on Mac or without loftr; target enumeration is not an executed-test inventory.'
}
environment = os.environ.copy()
environment['CARGO_MANIFEST_DIR'] = str(ROOT / 'crates/rrrah-dedup')
environment['RRRAH_BUILT_RECIPE_ID'] = 'diagnostic-current-test-metadata-not-for-cache'
base = list(prior['command'])
base.remove('--test')
base[base.index('-o') + 1] = str(WORK / 'librrrah_dedup.rmeta')
base += ['--crate-type', 'rlib', '--deny=unsafe_code']

commands = [('production_library', base)]
for target in targets:
    args = list(prior['command'])
    args[args.index('--crate-name') + 1] = target['name'].replace('-', '_')
    args[args.index('crates/rrrah-dedup/src/lib.rs')] = target['src_path']
    args[args.index('-o') + 1] = str(WORK / (target['name'] + '.rmeta'))
    args += ['--deny=unsafe_code']
    if 'lib' not in target['kind']:
        args += ['--extern', 'rrrah_dedup=' + str(WORK / 'librrrah_dedup.rmeta')]
    commands.append((target['name'], args))
save()
for index, (label, command) in enumerate(commands):
    log = WORK / (str(index) + '-' + label + '.log')
    began = time.monotonic()
    with log.open('x') as stream:
        child = subprocess.Popen(command, cwd=ROOT, env=environment, stdout=stream, stderr=subprocess.STDOUT)
        report['active'] = {'target': label, 'child_pid': child.pid}
        save()
        code = child.wait()
    report['rows'].append({'target': label, 'command': command, 'returncode': code, 'elapsed_seconds': time.monotonic() - began, 'log': str(log), 'log_sha256': sha(log)})
    save()
    if index == 0 and code != 0:
        break
changes = [path for path, digest in source_pins.items() if sha(ROOT / path) != digest]
for path, digest in prior['dependency_pins'].items():
    assert sha(path) == digest
failed = [row['target'] for row in report['rows'] if row['returncode'] != 0]
report.update(status='strict_metadata_passed' if not failed and not changes and len(report['rows']) == len(commands) else 'terminal_failures_or_source_drift', active=None, failed_targets=failed, source_changes=changes)
save()
print('Strict metadata:', len(report['rows']), 'completed;', len(failed), 'failed;', len(changes), 'source changes')
raise SystemExit(bool(failed or changes))
