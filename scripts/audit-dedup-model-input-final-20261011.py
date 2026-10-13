#!/usr/bin/env python3
"""Audit model preparation and verification source-guard regression on macOS."""
import gzip
import hashlib
import json
from pathlib import Path
import re
import shutil
import sys

ROOT = Path(__file__).resolve().parents[1]
D = ROOT / 'docs/research'
MANIFEST = D / 'dedup-model-input-final-snapshot-20261011.json'
COMMON = {'model_compare': 11, 'model_input_file': 10,
          'model_projected_compare': 8, 'model_tiles': 13,
          'portfolio_collection': 10, 'search_portfolio': 13}
EXTRA = {}


def sha(path):
    digest = hashlib.sha256()
    with Path(path).open('rb') as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b''):
            digest.update(chunk)
    return digest.hexdigest()

def main():
    platform = sys.argv[1]
    assert platform == 'macos'
    log = D / ('dedup-model-input-final-fixed-20261011.log' if platform == 'macos'
               else 'dedup-native-terminal-linux-20261011.log')
    output = D / f'dedup-model-input-final-{platform}-audit-20261011.json'
    archive = Path(f'/Users/themoretheless/.codex/tmp/rrrah-model-input-final-{platform}-qualified-20261011')
    assert not output.exists() and not archive.exists()
    expected = COMMON if platform == 'macos' else COMMON | EXTRA
    text = log.read_text()
    assert 'test result: FAILED' not in text
    runs = re.findall(r'Running tests/([^\s]+)\.rs \(([^)]+)\)(.*?)(?=\n\s*Running |\Z)', text, re.S)
    observed = {}
    for name, binary, body in runs:
        result = re.findall(r'test result: ok\. (\d+) passed; 0 failed;', body)
        assert len(result) == 1, (name, result)
        assert name not in observed
        observed[name] = int(result[0])
    assert observed == expected, observed
    manifest = json.loads(MANIFEST.read_text())
    snapshot = Path(manifest['snapshot'])
    def verify_snapshot():
        assert all(sha(snapshot / name) == digest for name, digest in manifest['file_hashes'].items())
    verify_snapshot()
    assert all(sha(ROOT / name) == manifest['file_hashes'][name] for name in manifest['changes'])
    before = D / 'dedup-model-input-final-before-20261011.log'
    before_text = before.read_text()
    assert 'test result: FAILED. 1 passed; 2 failed;' in before_text
    assert 'stale preparation Ok(PreparedModelFile' in before_text
    assert 'stale verification Ok(())' in before_text
    archive.mkdir()
    artifacts = []
    for name, binary, _ in runs:
        path = Path(binary)
        source = (ROOT / path if platform == 'macos' else
                  Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-linux-native') /
                  path.relative_to('/validation'))
        digest = sha(source)
        with source.open('rb') as stream:
            assert stream.read(4) == (b'\xcf\xfa\xed\xfe' if platform == 'macos' else b'\x7fELF')
        target = archive / (source.name + '.gz')
        with source.open('rb') as stream, target.open('wb') as output_stream:
            with gzip.GzipFile(filename='', mode='wb', fileobj=output_stream,
                               compresslevel=1, mtime=0) as packed:
                shutil.copyfileobj(stream, packed, 1024 * 1024)
        roundtrip = hashlib.sha256()
        with gzip.open(target, 'rb') as stream:
            for chunk in iter(lambda: stream.read(1024 * 1024), b''):
                roundtrip.update(chunk)
        assert roundtrip.hexdigest() == digest == sha(source)
        artifacts.append({'suite': name, 'source': str(source), 'archive': str(target),
                          'executable_sha256': digest, 'archive_sha256': sha(target),
                          'executable_bytes': source.stat().st_size,
                          'executable_mode': source.stat().st_mode & 0o7777,
                          'archive_bytes': target.stat().st_size})
    files = [log, MANIFEST, before, Path(__file__),
             Path('/Users/themoretheless/.codex/tmp/rrrah-model-input-final-before-20261011/model_input_file.rs')]
    if platform == 'linux':
        files.append(Path('/Users/themoretheless/.codex/tmp/rrrah-native-terminal-linux-20261011.sh'))
    pins = {}
    for source in files:
        digest = sha(source)
        target = archive / source.name
        shutil.copy2(source, target)
        assert sha(target) == digest
        pins[str(source)] = digest
    verify_snapshot()
    report = {'status': f'complete_bounded_model_input_final_{platform}_gate',
              'tests': sum(observed.values()), 'suite_counts': observed,
              'pins': pins, 'artifacts': artifacts,
              'source_manifest_sha256': sha(MANIFEST), 'frozen_source_files': len(manifest['file_hashes']),
              'scope': ('Root macOS compatibility with frozen two-file model-input guard delta; '
                        'other root dependencies have concurrent changes.' if platform == 'macos' else
                        'Linux aarch64 decode-feature execution on exact frozen dependency tree and guard delta.'),
              'limits': ['Does not qualify all repaired229 pairs or all-query negative precision',
                         'No new native Torch or Windows execution',
                         'No total tensor/workspace RSS admission claim',
                         'Continuation/exhaustion and cross-stage recovery are qualified; native later-recipe success after insufficiency and automatic projected tile search are not yet qualified']}
    output.write_text(json.dumps(report, indent=2) + '\n')
    print(report['status'], report['tests'])

if __name__ == '__main__':
    main()
