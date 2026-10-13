#!/usr/bin/env python3
"""Execute the complete decode-feature integration suite on pinned source."""
import hashlib
import json
import os
import subprocess
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DOC = ROOT / 'docs/research'
TMP = Path('/Users/themoretheless/.codex/tmp')


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def main():
    report = DOC / 'dedup-current-full-fixtures-integration-gate-20261012.json'
    assert not report.exists(), 'Inspect existing producer before any restart'
    manifest = DOC / 'dedup-current-full-fixtures-source-20261012.json'
    qualified = json.loads(manifest.read_text())
    source = Path(qualified['source'])
    hashes = qualified['source_hashes']
    assert all(sha(source / name) == digest for name, digest in hashes.items())
    log = TMP / 'rrrah-dedup-current-full-fixtures-integration-gate-20261012.log'
    command = ['cargo', 'test', '-p', 'rrrah-dedup', '--features', 'decode',
               '--locked', '--offline', '--tests', '--jobs', '2', '--target-dir',
               str(TMP / 'rrrah-dedup-exact-portfolio-20261011')]
    data = dict(status='running', producer_pid=os.getpid(), started_at=time.time(),
                source=str(source), source_manifest=str(manifest), command=command,
                log=str(log), pins={str(manifest): sha(manifest), str(Path(__file__)): sha(__file__)},
                scope='All rrrah-dedup test targets enabled by decode on frozen current source. No Torch-feature, other-platform, full500 review or universal format/RSS qualification.')

    def save():
        temporary = report.with_suffix('.tmp')
        temporary.write_text(json.dumps(data, indent=2) + '\n')
        temporary.replace(report)

    save()
    with log.open('x') as output:
        child = subprocess.Popen(command, cwd=source, stdout=output, stderr=subprocess.STDOUT)
        data['child_pid'] = child.pid
        save()
        code = child.wait()
    unchanged = all(sha(source / name) == digest for name, digest in hashes.items())
    data.update(returncode=code, source_unchanged=unchanged, finished_at=time.time(),
                status='complete_pending_audit' if code == 0 and unchanged else 'terminal_failure')
    data['pins'][str(log)] = sha(log)
    save()
    print(data['status'], code)


if __name__ == '__main__':
    main()
