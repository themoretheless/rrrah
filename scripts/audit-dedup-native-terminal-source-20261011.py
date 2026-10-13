#!/usr/bin/env python3
"""Audit and archive terminal-source regression gates, without merging source scopes."""
import gzip
import hashlib
import json
from pathlib import Path
import re
import shutil
import sys

ROOT = Path(__file__).resolve().parents[1]
D = ROOT / 'docs/research'
MANIFEST = D / 'dedup-native-terminal-source-snapshot-20261011.json'
COMMON = {'anchor_candidate_search': 1, 'anchor_fallback_search': 1,
          'anchor_geometry_search': 1, 'anchor_rank_file': 3,
          'anchor_reflected_geometry': 1, 'anchor_reflection_fallback': 12,
          'search_portfolio': 13}
EXTRA = {'anchor_geometry_rank_fallback': 1, 'anchor_fallback_collection_lifecycle': 1,
         'anchor_reflection_collection_lifecycle': 1, 'model_tiles': 13,
         'portfolio_collection': 10}

def sha(path):
    digest = hashlib.sha256()
    with Path(path).open('rb') as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b''):
            digest.update(chunk)
    return digest.hexdigest()

def main():
    platform = sys.argv[1]
    assert platform in ['macos', 'linux']
    log = D / ('dedup-native-terminal-source-fixed-20261011.log' if platform == 'macos'
               else 'dedup-native-terminal-linux-20261011.log')
    output = D / f'dedup-native-terminal-{platform}-audit-20261011.json'
    archive = Path(f'/Users/themoretheless/.codex/tmp/rrrah-native-terminal-{platform}-qualified-20261011')
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
    before = D / 'dedup-native-standalone-final-before-20261011.log'
    before_text = before.read_text()
    assert 'test result: FAILED. 0 passed; 5 failed;' in before_text
    assert all(f'case={i}: stale publication Ok(())' in before_text for i in range(5))
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
             D / 'dedup-native-to-model-recovery-20261011.log',
             D / 'dedup-native-inconclusive-final-source-20261011.log']
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
    report = {'status': f'complete_bounded_native_terminal_{platform}_gate',
              'tests': sum(observed.values()), 'suite_counts': observed,
              'pins': pins, 'artifacts': artifacts,
              'source_manifest_sha256': sha(MANIFEST), 'frozen_source_files': len(manifest['file_hashes']),
              'scope': ('Root macOS compatibility with frozen three-file native guard delta; '
                        'other root dependencies have concurrent changes.' if platform == 'macos' else
                        'Linux aarch64 decode-feature execution on exact frozen dependency tree and guard delta.'),
              'limits': ['Does not qualify all repaired229 pairs or all-query negative precision',
                         'No new native Torch or Windows execution',
                         'No total tensor/workspace RSS admission claim',
                         'Continuation/exhaustion and cross-stage recovery are qualified; native later-recipe success after insufficiency is not yet qualified']}
    output.write_text(json.dumps(report, indent=2) + '\n')
    print(report['status'], report['tests'])

if __name__ == '__main__':
    main()
