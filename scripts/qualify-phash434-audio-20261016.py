"""Actual whole-source audio controls with real libsndfile/libsamplerate."""
from pathlib import Path
import hashlib
import json
import os
import subprocess

ROOT = Path(__file__).resolve().parents[1]
WORK = ROOT / '.dedup-work/phash434-20261016'
FIXTURES = WORK / 'native-audio-fixtures'
REPORT = ROOT / 'docs/research/dedup-500/phash434-native-audio-20261016.json'
assert not REPORT.exists()
sha = lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
tree = {row['path']: row for row in json.loads((WORK / 'submodule-tree.json').read_text())['tree']}
sources = {}
for name in ['src/audiophash.cpp', 'src/audiophash.h', 'src/ph_fft.cpp', 'src/ph_fft.h', 'src/pHash.h.cmake', 'third-party/CImg/CImg.h']:
    path = WORK / 'submodule' / name
    data = path.read_bytes()
    blob = hashlib.sha1(b'blob ' + str(len(data)).encode() + b'\0' + data).hexdigest()
    assert blob == tree[name]['sha']
    sources[name] = {'sha256': sha(path), 'git_blob': blob, 'bytes': len(data)}
environment = dict(os.environ, ASAN_OPTIONS='detect_leaks=0:abort_on_error=1',
                   UBSAN_OPTIONS='halt_on_error=1:print_stacktrace=1')
results = []
for repetition in range(2):
    for mode in ['positive', 'short-block', 'zero-block', 'short-hash']:
        command = [str(FIXTURES / 'probe'), mode]
        if mode == 'positive':
            command.append(str(FIXTURES / 'silence.wav'))
        run = subprocess.run(command, env=environment, capture_output=True, text=True, timeout=30)
        log = FIXTURES / f'{mode}-qualified-{repetition}.log'
        assert not log.exists()
        log.write_text(run.stdout + run.stderr)
        if mode == 'positive':
            assert run.returncode == 0
            assert run.stdout.strip() == 'block_oracle_passes=65536 single_bits=32 silent_hashes=3 descriptor_growth=16'
        else:
            marker = {'short-block': 'AddressSanitizer: stack-buffer-overflow',
                      'zero-block': 'runtime error: division by zero',
                      'short-hash': 'exceeds maximum supported size'}[mode]
            assert run.returncode != 0 and marker in run.stderr
        results.append({'mode': mode, 'repetition': repetition, 'command': command,
                        'exit_code': run.returncode, 'log': str(log), 'log_sha256': sha(log)})
links = subprocess.check_output(['otool', '-L', str(FIXTURES / 'probe')], text=True)
(FIXTURES / 'linked-libraries.log').write_text(links)
assert 'libsndfile' in links and 'libsamplerate' in links
paths = [FIXTURES / name for name in ['probe', 'build-plan.json', 'build.log', 'generated/pHash.h', 'silence.wav', 'linked-libraries.log']]
paths += [ROOT / 'scripts/phash434-audio-controls-20261016.cpp', Path(__file__),
          Path('/opt/homebrew/lib/libsndfile.dylib'), Path('/opt/homebrew/lib/libsamplerate.dylib')]
REPORT.write_text(json.dumps({
    'status': 'verified_selected_actual_native_audio_boundary_controls',
    'repository': 'aetilius/pHash', 'commit': '887d07b9bdd9e2fb082c932002cefbcb1c8c20a1',
    'source_pins': sources, 'artifact_pins': {str(p): sha(p) for p in paths}, 'results': results,
    'findings': ['131072 byte-pair BER block oracle checks pass across two fresh processes; each normalized by fixed32bits',
                 '64 single-bit popcounts and6 silent-signal audio hash framing cases pass',
                 '16 normal WAV reads leave16 additional open descriptors in each fresh process; original readaudio_snd lacks sf_close',
                 'One-element input with block_size2 causes ASAN stack-buffer-overflow in original ph_compare_blocks, twice',
                 'block_size0 causes UBSAN division by zero in original BER comparator, twice',
                 'Zero-sample audiohash requests oversized allocation from negative frame count converted to unsigned size, twice'],
    'build_scope': 'Whole unchanged original audiophash.cpp and ph_fft.cpp, original headers and configured original pHash.h.cmake. Real installed libsndfile/libsamplerate, ASAN+UBSAN, no extracted algorithm bodies or fake providers. Synthetic800sample mono8000Hz16bit silence WAV generated locally.',
    'limits': ['Selected audio functions only; not full audio codec/resample/multichannel/MP3/BER tail/malformed-file qualification',
               'Descriptor growth measured by fcntl F_GETFD across0..4095; no heap leak/LSAN/RSS cap claim',
               'CImg pinned and compiled transitively, not fully read',
               'No production adoption, no current rrrah-dedup defect inferred, GPL3+ source retained outside production',
               'pHash434 full source and all500 full repository review remain incomplete']}, indent=2) + '\n')
print(REPORT)
