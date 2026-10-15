#!/usr/bin/env python3
"""Execute a complete current integration file against explicitly pinned cached dependencies."""
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parents[1]
DIRECTORY = ROOT / 'docs/research'
WORK = Path('/private/tmp/rrrah-projected-integration-cached-library-20261015')
WORK.mkdir(exist_ok=False)
OUTPUT = DIRECTORY / 'dedup-projected-integration-cached-library-20261015.json'
assert not OUTPUT.exists()

def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()

prior = json.loads((DIRECTORY / 'dedup-exact-missing-integration-metadata-20261015.json').read_text())
library_report = DIRECTORY / 'dedup-model-compare-test-warning-v1-20261015.json'
library_pins = json.loads(library_report.read_text())['dependency_pins']
library, = [path for path in library_pins if Path(path).name.startswith('librrrah_dedup-')]
for path, digest in library_pins.items():
    assert sha(path) == digest
target = ROOT / 'crates/rrrah-dedup/tests/projected_portfolio.rs'
assert sha(target) == prior['target_sha256']
target_pin = sha(target)
command = list(prior['command'])
command.remove('--emit=metadata')
command[command.index('-o') + 1] = str(WORK / 'tests')
command = ['rrrah_dedup=' + library if arg.startswith('rrrah_dedup=') else arg for arg in command]
build = Path('/private/tmp/rrrah-consensus-collection-target-20261014/debug/build')
for pattern, subdirectory in [('blake3-*', 'out'), ('astcenc-sys-*', 'out'), ('basis-universal-sys-*', 'out'), ('charls-sys-*', 'out/lib'), ('link-cplusplus-*', 'out'), ('openjpeg-sys-*', 'out'), ('jpegxr-*', 'out')]:
    for directory in sorted(build.glob(pattern)):
        native = directory / subdirectory
        if native.is_dir():
            command += ['-L', 'native=' + str(native)]
command += ['-L', 'native=/opt/homebrew/opt/dav1d/lib', '-L', 'native=/opt/homebrew/opt/libheif/lib']
environment = os.environ.copy()
environment['CARGO_MANIFEST_DIR'] = str(ROOT / 'crates/rrrah-dedup')
environment['RRRAH_BUILT_RECIPE_ID'] = 'diagnostic-current-test-metadata-not-for-cache'
report = dict(status='starting', producer_pid=os.getpid(), producer_sha256=sha(__file__), target_sha256=target_pin, library_pin_report_sha256=sha(library_report), dependency_pins=library_pins, commands=[], scope='Complete current projected_portfolio integration source including missing-source regression, opt-level0 test binary against explicitly pinned cached dedup/Core/decoder libraries. Cached dedup library is not a freshly rebuilt current Root artifact; passing results do not qualify full current Root production, all decoder/dependency versions, alltargets/release/platform/native corpus or performance. No test filter or skipped integration functions beyond original cfg conditions.')

def save():
    temporary = OUTPUT.with_suffix('.tmp')
    temporary.write_text(json.dumps(report, indent=2) + '\n')
    temporary.replace(OUTPUT)

save()
for name, args in [('compile', command), ('list', [str(WORK / 'tests'), '--list']), ('run', [str(WORK / 'tests'), '--test-threads', '1'])]:
    log = WORK / (name + '.log')
    with log.open('x') as stream:
        child = subprocess.Popen(args, cwd=ROOT, env=environment, stdout=stream, stderr=subprocess.STDOUT)
        report.update(status=name, child_pid=child.pid)
        save()
        code = child.wait()
    report['commands'].append(dict(name=name, command=args, returncode=code, log=str(log), log_sha256=sha(log)))
    save()
    if code:
        report['status'] = 'failed_' + name
        save()
        print(log.read_text()[-6000:])
        raise SystemExit(code)
assert sha(target) == target_pin
assert all(sha(path) == digest for path, digest in library_pins.items())
inventory = set(re.findall(r'^(.+): test$', (WORK / 'list.log').read_text(), re.M))
body = (WORK / 'run.log').read_text()
results = dict(re.findall(r'^test (.+) \.\.\. (ok|ignored[^\n]*)$', body, re.M))
assert set(results) == inventory
summary = re.search(r'test result: ok\. (\d+) passed; 0 failed; (\d+) ignored; 0 measured; 0 filtered out;', body)
assert summary
assert 'exact_collection_missing_sources_never_publish_partial_duplicates' in inventory
report.update(status='complete_inventory_executed_pending_independent_audit', executable=str(WORK / 'tests'), executable_sha256=sha(WORK / 'tests'), inventory=sorted(inventory), passed=int(summary[1]), ignored=int(summary[2]))
save()
print('Executed full integration inventory:', report['passed'], 'passed;', report['ignored'], 'ignored')
