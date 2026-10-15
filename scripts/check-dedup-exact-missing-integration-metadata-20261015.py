#!/usr/bin/env python3
"""Check the complete changed integration target against pinned library metadata."""
import hashlib
import json
import os
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]
RESEARCH = ROOT / 'docs/research'
PRIOR = RESEARCH / 'dedup-current-test-metadata-v2-20261015.json'
prior = json.loads(PRIOR.read_text())
WORK = Path('/private/tmp/rrrah-exact-missing-integration-metadata-20261015')
WORK.mkdir(exist_ok=False)
OUTPUT = RESEARCH / 'dedup-exact-missing-integration-metadata-20261015.json'
assert not OUTPUT.exists()

def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()

rows = prior['reused_passed_rows'] + prior['rerun_rows']
row, = [item for item in rows if item['target'] == 'projected_portfolio']
assert row['returncode'] == 0
production_sources = {
    path: digest for path, digest in prior['source_pins'].items()
    if path.startswith('crates/rrrah-dedup/src/')
}
assert production_sources
for path, digest in production_sources.items():
    assert sha(ROOT / path) == digest, path
for path, digest in prior['dependency_pins'].items():
    assert sha(path) == digest, path
command = list(row['command'])
command[command.index('-o') + 1] = str(WORK / 'projected_portfolio.rmeta')
assert '-Dwarnings' in command or ('-D' in command and 'warnings' in command)
metadata_arguments = [value for value in command if value.startswith('rrrah_dedup=')]
assert len(metadata_arguments) == 1
metadata = Path(metadata_arguments[0].split('=', 1)[1])
assert sha(metadata) == prior['production_metadata_sha256']
target = ROOT / 'crates/rrrah-dedup/tests/projected_portfolio.rs'
target_pin = sha(target)
environment = os.environ.copy()
environment['CARGO_MANIFEST_DIR'] = str(ROOT / 'crates/rrrah-dedup')
environment['RRRAH_BUILT_RECIPE_ID'] = 'diagnostic-current-test-metadata-not-for-cache'
result = subprocess.run(command, cwd=ROOT, env=environment, capture_output=True, text=True)
log = WORK / 'compile.log'
log.write_text(result.stdout + result.stderr)
assert sha(target) == target_pin
assert sha(metadata) == prior['production_metadata_sha256']
for path, digest in production_sources.items():
    assert sha(ROOT / path) == digest, path
report = dict(
    status='strict_complete_target_metadata_passed' if result.returncode == 0 else 'metadata_failed',
    producer_sha256=sha(__file__), prior_sha256=sha(PRIOR),
    production_source_pins=production_sources,
    production_metadata_sha256=sha(metadata), target_sha256=target_pin,
    dependency_pins=prior['dependency_pins'], command=command,
    returncode=result.returncode, log=str(log), log_sha256=sha(log),
    scope='Entire changed projected_portfolio integration target including missing-source exact-collection regression (12missing cases and6recoveries); actual pinned current production metadata and cached baseline decoder/Core dependencies. Strict compiler metadata only, no linking/execution/fullCargo/currentdecoder/platform/performance proof.'
)
OUTPUT.write_text(json.dumps(report, indent=2) + '\n')
print(report['status'])
raise SystemExit(result.returncode)
