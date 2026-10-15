#!/usr/bin/env python3
from pathlib import Path
import hashlib
import json
import os
import subprocess

ROOT = Path(__file__).resolve().parents[1]
RESEARCH = ROOT / 'docs/research'
WORK = Path('/private/tmp/rrrah-region-grid-test-warning-v1-20261015')
WORK.mkdir(exist_ok=False)
SOURCE = ROOT / 'crates/rrrah-dedup/tests/region_grid_files.rs'
BASELINE = Path('/private/tmp/rrrah-committed-build-source-415443172-v2-20261015') / SOURCE.relative_to(ROOT)
REPORT = RESEARCH / 'dedup-region-grid-test-warning-v1-20261015.json'
assert not REPORT.exists()

def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()

prior = json.loads((RESEARCH / 'dedup-current-strict-test-metadata-v1-20261015.json').read_text())
dependency = Path('/private/tmp/rrrah-consensus-collection-target-20261014/debug/deps/librrrah_dedup-004a9eeaafb57be4.rlib')
pins = dict(prior['dependency_pins'])
pins[str(dependency)] = sha(dependency)
for path, digest in pins.items():
    assert sha(path) == digest

command = list(prior['command'])
command[command.index('--crate-name') + 1] = 'region_grid_files'
source_index = command.index('crates/rrrah-dedup/src/lib.rs')
command += ['--extern', 'rrrah_dedup=' + str(dependency)]
environment = os.environ.copy()
environment['CARGO_MANIFEST_DIR'] = str(SOURCE.parent.parent)
environment['RRRAH_BUILT_RECIPE_ID'] = 'metadata-only-reflection-warning-probe'
source_digest = sha(SOURCE)
results = []
for label, source in [('before', BASELINE), ('after', SOURCE)]:
    copy = WORK / (label + '.rs')
    copy.write_bytes(source.read_bytes())
    args = list(command)
    args[source_index] = str(copy)
    args[args.index('-o') + 1] = str(WORK / (label + '.rmeta'))
    result = subprocess.run(args, cwd=ROOT, env=environment, capture_output=True, text=True)
    log = WORK / (label + '.log')
    log.write_text(result.stdout + result.stderr)
    if label == 'before':
        assert result.returncode != 0 and 'unused import' in result.stderr
    else:
        assert result.returncode == 0, result.stderr
    results.append({'label': label, 'returncode': result.returncode, 'source_sha256': sha(copy), 'log': str(log), 'log_sha256': sha(log)})
assert sha(SOURCE) == source_digest
for path, digest in pins.items():
    assert sha(path) == digest
REPORT.write_text(json.dumps({
    'status': 'verified_before_strict_warning_failure_after_metadata_success',
    'producer_sha256': sha(__file__),
    'source': str(SOURCE.relative_to(ROOT)),
    'source_sha256': source_digest,
    'dependency_pins': pins,
    'results': results,
    'scope': 'Complete current region-grid integration source typechecks against actual cached baseline library/core/decoder types with decode/raster and strict warnings. Original frozen415source reproduces unused-import failures; removal of only two unused imports passes, with all test bodies unchanged. No execution, pixel/geometry quality, Cargo resolver or full currentRoot CI proof.'
}, indent=2) + '\n')
print('Region-grid test: strict metadata fails before import cleanup, passes after')
