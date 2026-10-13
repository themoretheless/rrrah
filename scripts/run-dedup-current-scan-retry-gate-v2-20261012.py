#!/usr/bin/env python3
"""Run the new regression against dependencies used by the live frozen gate."""
import hashlib
import json
import shutil
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DOC = ROOT / 'docs/research'
TMP = Path('/Users/themoretheless/.codex/tmp')


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def main():
    captured = TMP / 'rrrah-dedup-scan-retry-live-compiler-dependencies-v2-20261012.json'
    selected = json.loads(captured.read_text())
    manifest = DOC / 'dedup-current-full-fixtures-source-20261012.json'
    frozen = json.loads(manifest.read_text())
    source = Path(frozen['source'])
    assert all(sha(source / name) == digest for name, digest in frozen['source_hashes'].items())
    target = TMP / 'rrrah-dedup-current-scan-retry-qualified-v2-20261012'
    output = DOC / 'dedup-current-scan-retry-gate-v2-20261012.json'
    assert not target.exists() and not output.exists()
    target.mkdir()
    root_test = ROOT / 'crates/rrrah-dedup/tests/scan_retry.rs'
    copied = target / 'scan_retry.rs'
    shutil.copy2(root_test, copied)
    original = selected['captured_args']
    assert '--test' in original
    command = []
    index = 0
    sources = 0
    dependencies = []
    while index < len(original):
        value = original[index]
        if value == '--crate-name':
            command.extend([value, 'rrrah_scan_retry_current_20261012'])
            index += 2
        elif value == '--out-dir':
            index += 2
        elif value == '-C' and original[index + 1].startswith('extra-filename='):
            index += 2
        elif value.startswith('--emit='):
            command.append('--emit=link')
            index += 1
        elif value == '--extern':
            command.extend(original[index:index + 2])
            dependencies.append(Path(original[index + 1].split('=', 1)[1]))
            index += 2
        elif value.endswith('.rs'):
            sources += 1
            command.append(str(copied))
            index += 1
        else:
            command.append(value)
            index += 1
    assert sources == 1 and len(dependencies) == 7
    binary = target / 'scan_retry_test'
    command.extend(['-o', str(binary)])
    pins = {str(path): sha(path) for path in [captured, manifest, root_test, copied,
            Path(__file__), Path(command[0]), *dependencies]}
    data = dict(status='building', command=command, pins=pins,
        provenance='Dependency paths and full compiler flags captured from a live child of the frozen complete decode Cargo gate. Rustc output is private; no Cargo lock bypass writes to the shared target.',
        scope='New scan retry regression only, linked to compiled current frozen decode dependencies. Complements but does not replace the140target full suite; new test is outside its frozen inventory.')

    def save():
        output.write_text(json.dumps(data, indent=2) + '\n')

    save()
    build_log = target / 'build.log'
    with build_log.open('x') as stream:
        child = subprocess.Popen(command, cwd=source, stdout=stream, stderr=subprocess.STDOUT)
        data['child_pid'] = child.pid
        save()
        code = child.wait()
    data['build_returncode'] = code
    pins[str(build_log)] = sha(build_log)
    if code == 0:
        pins[str(binary)] = sha(binary)
        data['status'] = 'running'
        save()
        log = target / 'test.log'
        with log.open('x') as stream:
            child = subprocess.Popen([str(binary)], stdout=stream, stderr=subprocess.STDOUT)
            data['child_pid'] = child.pid
            save()
            code = child.wait()
        data['returncode'] = code
        pins[str(log)] = sha(log)
        data['test_log'] = str(log)
    assert all(sha(path) == digest for path, digest in pins.items())
    assert all(sha(source / name) == digest for name, digest in frozen['source_hashes'].items())
    data['status'] = 'complete_pending_audit' if code == 0 else 'terminal_failure'
    save()
    print(data['status'], code)


if __name__ == '__main__':
    main()
