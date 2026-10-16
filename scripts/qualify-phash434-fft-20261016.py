from pathlib import Path
import hashlib
import json
import os
import resource
import subprocess

ROOT = Path(__file__).resolve().parents[1]
WORK = ROOT / '.dedup-work/phash434-20261016'
OUTPUT = WORK / 'native-fft-fixtures'
OUTPUT.mkdir(exist_ok=False)
tree = {row['path']: row for row in json.loads((WORK / 'submodule-tree.json').read_text())['tree']}
sha = lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
sources = {}
for name in ['ph_fft.cpp', 'ph_fft.h', 'audiophash.cpp', 'audiophash.h']:
    path = WORK / 'submodule/src' / name
    data = path.read_bytes()
    git_sha = hashlib.sha1(b'blob ' + str(len(data)).encode() + b'\0' + data).hexdigest()
    assert git_sha == tree['src/' + name]['sha']
    sources['src/' + name] = {'sha256': sha(path), 'git_blob': git_sha, 'bytes': len(data)}
binary = WORK / 'native-fft-controls'
results = []
environment = dict(os.environ, ASAN_OPTIONS='detect_leaks=0:abort_on_error=1')


def child_limits():
    resource.setrlimit(resource.RLIMIT_STACK, (8 * 1024 * 1024, 8 * 1024 * 1024))


for repetition in range(2):
    for mode in ['oracle', 'zero']:
        command = [str(binary), mode]
        run = subprocess.run(command, capture_output=True, text=True, env=environment,
                             preexec_fn=child_limits, timeout=30)
        log = OUTPUT / f'{repetition}-{mode}.log'
        log.write_text(run.stdout + run.stderr)
        if mode == 'oracle':
            assert run.returncode == 0
            assert run.stdout.strip() == 'power_two_oracle_passes=64 odd_lengths_silently_wrong=6'
        else:
            assert run.returncode != 0 and 'AddressSanitizer: stack-overflow' in run.stderr
        results.append({'mode': mode, 'repetition': repetition, 'command': command,
                        'exit_code': run.returncode, 'log': str(log), 'log_sha256': sha(log)})
report = ROOT / 'docs/research/dedup-500/phash434-native-fft-20261016.json'
assert not report.exists()
report.write_text(json.dumps({
    'status': 'verified_selected_native_fft_controls',
    'repository': 'aetilius/pHash', 'commit': '887d07b9bdd9e2fb082c932002cefbcb1c8c20a1',
    'full_read_new_sources': sources,
    'binary': str(binary), 'binary_sha256': sha(binary),
    'probe': 'scripts/phash434-fft-controls-20261016.cpp',
    'probe_sha256': sha(ROOT / 'scripts/phash434-fft-controls-20261016.cpp'),
    'producer_sha256': sha(Path(__file__)), 'results': results,
    'oracle': 'Independent direct positive-exponent DFT, all output bins, eight deterministic inputs at each length1,2,4,8,16,32,64,128. Absolute complex tolerance1e-9.',
    'findings': ['64 power-of-two controls pass each of two runs',
                 'Lengths3,5,7,9,15,17 return success with incorrect output each run; no length guard in original API',
                 'Length0 recurses without reaching N==1 and causes ASAN stack-overflow in both fresh child processes'],
    'static_audio_review': ['Complete original audio implementation and header read, but no audio runtime qualification',
                            'readaudio_snd opens SNDFILE without a matching sf_close on its normal return path',
                            'Audio hash framing and BER blocks require separate short-input/allocation/tail/read-error runtime controls'],
    'limits': ['Whole unchanged original ph_fft.cpp and original ph_fft.h compiled; no algorithm extraction or substitute backend',
               'Zero-input safety probe uses child-only8MiB stack limit; ASAN leak detection disabled; no leak/RSS claim',
               'No audio decode/hash/BER runtime or full repository/CImg/bindings/platform qualification',
               'No production code copied or adopted; original GPL3-or-later license retained in source',
               'All500 repository full review remains incomplete']}, indent=2) + '\n')
print(report)
