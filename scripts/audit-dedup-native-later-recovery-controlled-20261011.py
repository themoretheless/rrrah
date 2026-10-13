#!/usr/bin/env python3
"""Execute isolated frozen before/after controls and bind outcomes to their binaries."""
import gzip
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[1]
D = ROOT / 'docs/research'
OUT = D / 'dedup-native-later-recovery-controlled-audit-20261011.json'
ARCHIVE = Path('/Users/themoretheless/.codex/tmp/rrrah-native-later-recovery-controlled-qualified-20261011')

def sha(p):
    return hashlib.sha256(p.read_bytes()).hexdigest()

def main():
    assert not OUT.exists() and not ARCHIVE.exists()
    before_manifest = D / 'dedup-native-later-recovery-before-snapshot-20261011.json'
    after_manifest = D / 'dedup-native-later-recovery-snapshot-20261011.json'
    before = json.loads(before_manifest.read_text())
    after = json.loads(after_manifest.read_text())
    assert set(before['file_hashes']) == set(after['file_hashes'])
    changed = [name for name in before['file_hashes'] if before['file_hashes'][name] != after['file_hashes'][name]]
    assert changed == ['crates/rrrah-dedup/src/anchor_rank_file.rs'], changed
    def verify():
        for manifest in [before, after]:
            base = Path(manifest['snapshot'])
            assert all(sha(base / name) == digest for name, digest in manifest['file_hashes'].items())
    verify()
    before_log = D / 'dedup-native-later-recipe-before-20261011.log'
    after_log = D / 'dedup-native-later-recipe-frozen-after-20261011.log'
    assert 'test result: FAILED. 0 passed; 2 failed;' in before_log.read_text()
    assert 'test result: ok. 2 passed; 0 failed;' in after_log.read_text()
    invalidated_path = D / 'dedup-native-later-recovery-macos-audit-20261011.json'
    invalidated = json.loads(invalidated_path.read_text())
    assert invalidated['status'] == 'invalidated_executable_association'
    previous_archive = Path(invalidated['binary_archive'])
    assert sha(previous_archive) == invalidated['binary_archive_sha256']
    ARCHIVE.mkdir()
    old = ARCHIVE / 'before-test'
    with gzip.open(previous_archive, 'rb') as src, old.open('wb') as dst:
        shutil.copyfileobj(src, dst)
    assert sha(old) == invalidated['binary_sha256']
    raw_path = re.search(r'Running tests/native_anchor_recovery.rs \(([^)]+)\)', after_log.read_text()).group(1)
    source = ROOT / raw_path
    digest = sha(source)
    new = ARCHIVE / 'after-test'
    shutil.copy2(source, new)
    assert digest == sha(source) == sha(new)
    # The test resolves fixtures with compile-time CARGO_MANIFEST_DIR. These
    # embedded roots disambiguate the reused Cargo output filename.
    assert str(Path(before['snapshot']) / 'crates/rrrah-dedup').encode() in old.read_bytes()
    assert str(Path(after['snapshot']) / 'crates/rrrah-dedup').encode() in new.read_bytes()
    executions = []
    for label, binary, code in [('before', old, 101), ('after', new, 0)]:
        binary.chmod(0o755)
        expected = sha(binary)
        run = subprocess.run([str(binary), '--nocapture'], capture_output=True, text=True)
        output = ARCHIVE / f'{label}-execution.log'
        output.write_text(run.stdout + run.stderr)
        assert run.returncode == code, (label, run.returncode, run.stdout, run.stderr)
        assert sha(binary) == expected
        if label == 'before':
            assert 'test result: FAILED. 0 passed; 2 failed;' in run.stdout
            assert run.stderr.count('Rank(InsufficientGeometry)') == 2
        else:
            assert 'test result: ok. 2 passed; 0 failed;' in run.stdout
            for six in ['false', 'true']:
                assert f'six={six} angle-17.png recovered recipe 2: anchors=254/250 minimum=240' in run.stdout
                assert f'six={six} angle-63.png recovered recipe 2: anchors=226/237 minimum=220' in run.stdout
        packed = ARCHIVE / f'{label}-test.gz'
        with binary.open('rb') as src, packed.open('wb') as dst:
            with gzip.GzipFile(filename='', mode='wb', fileobj=dst, compresslevel=1, mtime=0) as stream:
                shutil.copyfileobj(src, stream)
        with gzip.open(packed, 'rb') as stream:
            assert hashlib.sha256(stream.read()).hexdigest() == expected
        executions.append({'variant': label, 'binary': str(binary), 'binary_sha256': expected,
                           'returncode': run.returncode, 'log': str(output), 'log_sha256': sha(output),
                           'gzip': str(packed), 'gzip_sha256': sha(packed)})
    verify()
    pins = {}
    for p in [before_manifest, after_manifest, before_log, after_log, invalidated_path, Path(__file__)]:
        target = ARCHIVE / p.name
        shutil.copy2(p, target)
        assert sha(target) == sha(p)
        pins[str(p)] = sha(p)
    report = {'status': 'complete_bounded_frozen_native_later_recipe_recovery',
              'frozen_files_each': len(after['file_hashes']), 'changed_source_files': changed,
              'executions': executions, 'pins': pins,
              'fixtures': ['angle-17.png', 'angle-63.png'], 'recipe_sizes': [3, 6],
              'first_recipe': [0, 0], 'supported_recipe': [1, 0], 'attempted_recipes': 2,
              'minimum_witnesses': [240, 220], 'verified_anchor_counts': [[254, 250], [226, 237]],
              'exact_direct_second_recipe_geometry_points_and_pixel_evidence': True,
              'managed_credit_released_before_recovery_and_after_result_drop': True,
              'baseline_scope': 'Both wrapper tests fail at angle-17 before reaching angle-63. Corrected tests each complete both fixtures.',
              'limits': ['Configured smoothing/strict anchor-count controls, not default-policy or broad-corpus recall',
                         'No Linux/Torch/Windows qualification of these new controls',
                         'Full repaired229, negative precision, automatic projected search and total model RSS remain separate requirements']}
    OUT.write_text(json.dumps(report, indent=2)+'\n')
    print(report['status'])

if __name__ == '__main__': main()
