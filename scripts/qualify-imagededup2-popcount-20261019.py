"""Compare original selected popcount and original portable macro expansion to Python oracle."""
from pathlib import Path
import ctypes
import hashlib
import json
import random
import subprocess

R = Path(__file__).resolve().parents[1]
S = R / '.dedup-work/imagededup2-20261019/source'
W = R / '.dedup-work/imagededup2-popcount-20261019'
W.mkdir(exist_ok=False)
header = S / 'imagededup/handlers/search/builtin/builtin.h'
source = W / 'probe.cpp'
source.write_text('''#include "builtin/builtin.h"
PSNIP_BUILTIN__POPCOUNT_DEFINE_PORTABLE(review_popcountll, unsigned long long)
extern "C" int selected_count(unsigned long long x) { return psnip_builtin_popcountll(x); }
extern "C" int portable_count(unsigned long long x) { return psnip_builtin_review_popcountll(x); }
''')
library = W / 'probe.dylib'
command = ['clang++','-dynamiclib','-O2','-std=c++11','-I'+str(header.parent.parent),str(source),'-o',str(library)]
build = subprocess.run(command,capture_output=True,text=True)
(W/'build.stderr.log').write_text(build.stderr)
assert build.returncode == 0, build.stderr
loaded = ctypes.CDLL(str(library))
functions = [loaded.selected_count, loaded.portable_count]
for fn in functions:
    fn.argtypes=[ctypes.c_ulonglong]
    fn.restype=ctypes.c_int
rng = random.Random(20261019)
values = list(range(65536)) + [1<<i for i in range(64)] + [((1<<64)-1) ^ (1<<i) for i in range(64)] + [rng.getrandbits(64) for _ in range(4096)] + [0,(1<<64)-1]
for value in values:
    expected=value.bit_count()
    for fn in functions:
        assert fn(value)==expected
result=dict(status='passed_actual_selected_and_original_portable_popcount',
    header_sha256=hashlib.sha256(header.read_bytes()).hexdigest(),
    probe_sha256=hashlib.sha256(source.read_bytes()).hexdigest(),build_command=command,
    values_per_implementation=len(values),implementations=2,comparisons=2*len(values),
    domain='all16bit values,64singlebit,64one-zero-bit,4096seeded64bit random,zero/allones',
    findings=['Full1500line header manually read, including compiler/width selection, fallback functions, intrinsics and CC0 origin notice.',
              'Original portable popcount macro instantiated under distinct name, avoiding edits to bundled source or builtin selection.',
              'Selected compiler builtin and original portable expansion agree with independent int.bit_count on complete control domain.'],
    limits=['Only popcountll implementations executed; other header functions and platform-specific branches not runtime-qualified.',
            'MacARM clang only; no Windows/Linux/otherCPU portability, timing or memory-safety proof.',
            'No production code adopted; deferred native pHash investigation untouched.'])
(W/'result.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps({'values_per_implementation':len(values),'comparisons':2*len(values)}))
