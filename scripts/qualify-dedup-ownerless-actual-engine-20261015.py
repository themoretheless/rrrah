#!/usr/bin/env python3
"""Compile actual collection/identity/scheduling modules; keep full-library gate separate."""
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[1]
DIRECTORY = ROOT / 'docs/research'
PREPARED = DIRECTORY / 'dedup-ownerless-collection-full-v2-prepared-20261015.json'
prepared = json.loads(PREPARED.read_text())
SOURCE = Path(prepared['source'])
WORK = Path('/private/tmp/rrrah-ownerless-actual-engine-20261015')
WORK.mkdir(exist_ok=False)
OUTPUT = DIRECTORY / 'dedup-ownerless-actual-engine-20261015.json'
assert not OUTPUT.exists()

def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()

manifest_path = Path(prepared['baseline_manifest'])
assert sha(manifest_path) == prepared['baseline_manifest_sha256']
manifest = json.loads(manifest_path.read_text())
expected = {row['path']: row['sha256'] for row in manifest['source_files']}
expected.update({row['path']: row['candidate_sha256'] for row in prepared['changed']})
source_pins = {}
for name in ['collection.rs', 'exact.rs', 'pair_schedule.rs', 'decode.rs']:
    path = 'crates/rrrah-dedup/src/' + name
    original = SOURCE / path
    assert sha(original) == expected[path]
    source_pins[str(original)] = sha(original)
    if name != 'decode.rs':
        shutil.copyfile(original, WORK / name)
decode = (SOURCE / 'crates/rrrah-dedup/src/decode.rs').read_text()
start = decode.index('/// Source observations retained')
end = decode.index('#[derive(Debug, thiserror::Error)]\npub enum FileError')
fragment = WORK / 'decode-observations.rs'
fragment.write_text(decode[start:end])
harness = WORK / 'engine.rs'
harness.write_text('''pub mod local { pub use rrrah_dedup::local::LocalError; }
#[path="exact.rs"] pub mod exact;
pub mod decode { use rrrah_decode::DecodeRequest; include!("decode-observations.rs"); }
#[path="pair_schedule.rs"] pub mod pair_schedule;
#[path="collection.rs"] pub mod collection;
''')
prior = json.loads((DIRECTORY / 'dedup-model-compare-test-warning-v1-20261015.json').read_text())
dependency_pins = prior['dependency_pins']
for path, digest in dependency_pins.items():
    assert sha(path) == digest
target = Path('/private/tmp/rrrah-consensus-collection-target-20261014/debug')
command = ['rustc', '+stable', '--edition=2024', '--test', str(harness), '--cfg', 'feature="decode"', '-C', 'opt-level=3', '-C', 'debuginfo=0', '-L', 'dependency=' + str(target / 'deps'), '-o', str(WORK / 'engine')]
for path in dependency_pins:
    crate = Path(path).name.removeprefix('lib').split('-', 1)[0]
    command += ['--extern', crate + '=' + path]
for pattern, subdirectory in [('blake3-*', 'out'), ('astcenc-sys-*', 'out'), ('basis-universal-sys-*', 'out'), ('charls-sys-*', 'out/lib'), ('link-cplusplus-*', 'out'), ('openjpeg-sys-*', 'out'), ('jpegxr-*', 'out')]:
    for directory in sorted((target / 'build').glob(pattern)):
        native = directory / subdirectory
        if native.is_dir():
            command += ['-L', 'native=' + str(native)]
command += ['-L', 'native=/opt/homebrew/opt/dav1d/lib', '-L', 'native=/opt/homebrew/opt/libheif/lib']
report = dict(status='starting', producer_pid=os.getpid(), producer_sha256=sha(__file__), prepared_sha256=sha(PREPARED), source_pins=source_pins, harness_sha256=sha(harness), decode_fragment_sha256=sha(fragment), dependency_pins=dependency_pins, commands=[], scope='Whole original experimental collection module including original and owner-guard tests/actual-engine benchmark, whole original exact and pair_schedule modules, unchanged actual decode-source/dependency observation fragment. Real cached DecodeRequest/GenerationToken/Core/decoder/LocalError types; no fake snapshots/IO/credits/scheduler. Selected-module composition, not complete decoder/dedup library or fresh current Root/release/platform/corpus/image/provider quality qualification. Full all-target baseline/candidate gate remains required before adoption.')

def save():
    temporary = OUTPUT.with_suffix('.tmp')
    temporary.write_text(json.dumps(report, indent=2) + '\n')
    temporary.replace(OUTPUT)

save()
for name, args in [('compile', command), ('list', [str(WORK / 'engine'), '--list']), ('run', [str(WORK / 'engine'), '--test-threads', '1'])]:
    log = WORK / (name + '.log')
    with log.open('x') as stream:
        child = subprocess.Popen(args, cwd=ROOT, stdout=stream, stderr=subprocess.STDOUT)
        report.update(status=name, child_pid=child.pid)
        save()
        code = child.wait()
    report['commands'].append(dict(name=name, command=args, returncode=code, log=str(log), log_sha256=sha(log)))
    save()
    if code:
        report['status'] = 'failed_' + name
        save()
        print(log.read_text()[-8000:])
        raise SystemExit(code)
for path, digest in source_pins.items():
    assert sha(path) == digest
assert all(sha(path) == digest for path, digest in dependency_pins.items())
inventory = set(re.findall(r'^(.+): test$', (WORK / 'list.log').read_text(), re.M))
body = (WORK / 'run.log').read_text()
results = dict(re.findall(r'^test (.+) \.\.\. (ok|ignored[^\n]*)$', body, re.M))
assert set(results) == inventory
summary = re.search(r'test result: ok\. (\d+) passed; 0 failed; (\d+) ignored; 0 measured; 0 filtered out;', body)
assert summary
report.update(status='complete_selected_modules_pending_independent_audit', executable=str(WORK / 'engine'), executable_sha256=sha(WORK / 'engine'), inventory=sorted(inventory), passed=int(summary[1]), ignored=int(summary[2]))
save()
print(report['passed'], 'passed;', report['ignored'], 'ignored; no full-library/adoption claim')
