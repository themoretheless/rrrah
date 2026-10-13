#!/usr/bin/env python3
import hashlib
import json
import os
import shutil
import subprocess
from pathlib import Path

R = Path(__file__).resolve().parents[1]
D = R / 'docs/research'
T = Path('/Users/themoretheless/.codex/tmp')

def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()

def main():
    parent = D / 'dedup-zero-smoothing-borrow-source-v4-20261012.json'
    m = json.loads(parent.read_text())
    source = T / 'rrrah-disc21-heldout-negative-source-20261012'
    harness = T / 'rrrah-disc21-heldout-negative-harness-20261012'
    manifest = D / 'dedup-disc21-heldout-negative-source-20261012.json'
    report = D / 'dedup-disc21-heldout-negative-build-20261012.json'
    assert not any(p.exists() for p in [source, harness, manifest, report])
    for k, value in m['file_hashes'].items():
        original = Path(m['snapshot']) / k
        assert sha(original) == value
        if k.startswith('crates/rrrah-dedup/src/'):
            assert sha(R / k) == value, k
        path = source / k
        path.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(original, path)
    shutil.copy2(R / 'crates/rrrah-dedup/tests/exact_files.rs', source / 'crates/rrrah-dedup/tests/exact_files.rs')
    old = T / 'rrrah-disc21-bidirectional-exposure-collection-harness-20261012'
    old_source = T / 'rrrah-disc21-bidirectional-exposure-collection-source-20261012'
    harness.mkdir()
    (harness / 'Cargo.toml').write_text((old / 'Cargo.toml').read_text().replace(str(old_source), str(source)))
    shutil.copy2(old / 'Cargo.lock', harness / 'Cargo.lock')
    code = (old / 'src/bin/disc21_bidirectional_exposure_collection.rs').read_text()
    unused = '    let mut negative=rrrah_decode::DecodeRequest::new(&args[2]);negative.assume_untagged_linear_srgb=true;\n'
    assert code.count(unused) == 1
    code = code.replace(unused, '').replace('args.len() != 6', 'args.len() != 5')
    code = code.replace('usage: disc21_bidirectional_exposure_collection source target negative weights max_side source_tolerance',
                        'usage: disc21_heldout_negative_collection source target weights max_side source_tolerance')
    for old_index in [3, 4, 5]:
        code = code.replace(f'args[{old_index}]', f'args[{old_index - 1}]')
    code = code.replace('Actual public bidirectional collection with reference assigned lower ID than dark query. One cached CPU provider spans both directions. Development control, not general precision/RSS/platform/full500 qualification.',
                        'Actual public bidirectional collection on precommitted unused publisher distractor query/reference pair. Reference has lower ID; one CPU provider spans both directions. Fixed previous caller policy, not full precision/RSS/platform/full500 qualification.')
    probe = harness / 'src/bin/disc21_heldout_negative_collection.rs'
    probe.parent.mkdir(parents=True)
    probe.write_text(code)
    hashes = {k: sha(source / k) for k in m['file_hashes']}
    manifest.write_text(json.dumps({
        'status': 'frozen_current_dedup_production_heldout_negative_probe',
        'snapshot': str(source), 'parent_manifest': str(parent), 'parent_sha256': sha(parent),
        'file_hashes': hashes, 'deltas_from_parent': [k for k in hashes if hashes[k] != m['file_hashes'][k]],
        'scope': 'All dedup production equals adopted root v4; only test-only exact block-order regression copied over frozen compatible dependencies. Probe retains archived bidirectional CPU policy and public API, removes unused negative argument and changes diagnostic text. No thresholds changed.',
    }, indent=2) + '\n')
    pins = {str(p): sha(p) for p in [parent, manifest, old / 'src/bin/disc21_bidirectional_exposure_collection.rs',
                                    harness / 'Cargo.toml', harness / 'Cargo.lock', probe, Path(__file__)]}
    result = {'status': 'building', 'producer_pid': os.getpid(), 'pins': pins}
    def save():
        temp = report.with_suffix('.tmp')
        temp.write_text(json.dumps(result, indent=2) + '\n')
        temp.replace(report)
    save()
    runtime = T / 'rrrah-loftr-native-runtime-20261010'
    env = dict(os.environ, LIBTORCH_USE_PYTORCH='1')
    env['PATH'] = str(runtime / 'bin') + ':' + env['PATH']
    log = T / 'rrrah-disc21-heldout-negative-build-20261012.log'
    command = ['cargo', 'build', '--manifest-path', str(harness / 'Cargo.toml'), '--bin',
               'disc21_heldout_negative_collection', '--locked', '--offline', '--target-dir',
               str(T / 'rrrah-dedup-exact-portfolio-20261011')]
    result['build_command'] = command
    with log.open('w') as stream:
        child = subprocess.Popen(command, cwd='/tmp', env=env, stdout=stream, stderr=subprocess.STDOUT)
        result['build_pid'] = child.pid
        save()
        code = child.wait()
    result.update(build_returncode=code, build_log=str(log), build_log_sha256=sha(log))
    pins[str(log)] = sha(log)
    if code:
        result['status'] = 'terminal_build_failure'
        save()
        raise RuntimeError('Probe build failed; log preserved')
    original = T / 'rrrah-dedup-exact-portfolio-20261011/debug/disc21_heldout_negative_collection'
    binary = T / 'rrrah-disc21-heldout-negative-executable-20261012'
    assert not binary.exists()
    shutil.copy2(original, binary)
    assert sha(original) == sha(binary)
    pins[str(binary)] = sha(binary)
    assert all(sha(k) == v for k, v in pins.items())
    result.update(status='compiled_probe_not_executed', executable=str(binary))
    save()
    print('Built and preserved current-source bidirectional CPU probe', flush=True)

if __name__ == '__main__':
    main()
