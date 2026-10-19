"""Check normal-input streaming consistency of the pinned original bundled xxHash."""
from pathlib import Path
import ctypes as C
import hashlib
import json
import subprocess

ROOT = Path(__file__).resolve().parents[1]
WORK = ROOT / '.dedup-work/dupd50-hash-streaming-20261019'
SOURCE = ROOT / '.dedup-work/dupd50-20261019/source'
COMMIT = '33907bbcfd5fe8b63f54d4f55438fa222ea880aa'

def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def main():
    assert subprocess.check_output(['git', '-C', str(SOURCE), 'rev-parse', 'HEAD'], text=True).strip() == COMMIT
    pins = {}
    for name in ['src/xxhash.c', 'src/xxhash.h']:
        raw = (SOURCE / name).read_bytes()
        assert subprocess.check_output(['git', '-C', str(SOURCE), 'show', 'HEAD:' + name]) == raw
        pins[name] = sha(SOURCE / name)
    WORK.mkdir(exist_ok=False)
    library = WORK / 'liboriginalxxhash.dylib'
    command = ['cc', '-O2', '-dynamiclib', str(SOURCE / 'src/xxhash.c'), '-o', str(library)]
    with (WORK / 'build.log').open('wb') as output:
        subprocess.run(command, stdout=output, stderr=subprocess.STDOUT, check=True)
    lib = C.CDLL(str(library))
    rows = []
    for bits, word in [(32, C.c_uint), (64, C.c_ulonglong)]:
        one = getattr(lib, f'XXH{bits}')
        create = getattr(lib, f'XXH{bits}_createState')
        free = getattr(lib, f'XXH{bits}_freeState')
        reset = getattr(lib, f'XXH{bits}_reset')
        update = getattr(lib, f'XXH{bits}_update')
        digest = getattr(lib, f'XXH{bits}_digest')
        one.argtypes = [C.c_void_p, C.c_size_t, word]; one.restype = word
        create.argtypes = []; create.restype = C.c_void_p
        free.argtypes = [C.c_void_p]; free.restype = C.c_int
        reset.argtypes = [C.c_void_p, word]; reset.restype = C.c_int
        update.argtypes = [C.c_void_p, C.c_void_p, C.c_size_t]; update.restype = C.c_int
        digest.argtypes = [C.c_void_p]; digest.restype = word
        for size in [*range(130), 255, 256, 257, 1023, 1024, 1025, 65535, 65536, 65537, 1000001]:
            data = bytes(i % 251 for i in range(size))
            buffer = C.create_string_buffer(data)
            for seed in [0, 1, 42]:
                expected = one(buffer, size, seed)
                chunks = [1, 3, 7, 15, 16, 17, 31, 32, 33, 64, 4096] if size <= 1025 else [4095, 4096, 4097, 65536]
                for chunk in chunks:
                    state = create(); assert state
                    try:
                        assert reset(state, seed) == 0
                        assert digest(state) == one(buffer, 0, seed)
                        for start in range(0, size, chunk):
                            count = min(chunk, size - start)
                            assert update(state, C.byref(buffer, start), count) == 0
                        actual = digest(state)
                        assert actual == expected, (bits, size, seed, chunk)
                        assert digest(state) == actual
                        rows.append({'bits': bits, 'size': size, 'seed': seed, 'chunk': chunk, 'hash': f'{actual:0{bits // 4}x}'})
                    finally:
                        assert free(state) == 0
    assert all(sha(SOURCE / name) == pin for name, pin in pins.items())
    result = {'status': 'passed_normal_input_streaming_consistency', 'commit': COMMIT, 'source_pins': pins, 'build_command': command, 'library_sha256': sha(library), 'cases': len(rows), 'rows': rows, 'limits': ['One-shot and streaming use the same original implementation; no independent hash oracle.', 'Normal valid buffers only; no invalid pointers, sanitizers or memory-error probes.', 'No collision-resistance, cross-platform, speed or whole-repository claim.']}
    (WORK / 'result.json').write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps({'status': result['status'], 'cases': len(rows)}))

if __name__ == '__main__':
    main()
