"""Compile unchanged sampled-offset expression from pinned Windows source with portable scaffolding."""
from pathlib import Path
import ctypes
import hashlib
import json
import random
import re
import subprocess

R=Path(__file__).resolve().parents[1]
S=R/'.dedup-work/windirstat3-20261019/source'
W=R/'.dedup-work/windirstat3-sample-offsets-20261019'
W.mkdir(exist_ok=False)
source=S/'windirstat/Item.Extended.cpp'
text=source.read_text(encoding='utf-8-sig')
expression=re.search(r'offset = (span / 3 \* block[^;]+);',text).group(1)
probe=W/'probe.cpp'
probe.write_text('#include <cstdint>\nextern "C" uint64_t sampled_offset(uint64_t size,uint64_t total) {\n'
    'const uint64_t Mi=1024*1024; const auto span=size-Mi; const auto block=total/Mi; const auto withinBlock=total%Mi;\n'
    'return '+expression+';\n}\n')
command=['clang++','-dynamiclib','-O2','-std=c++17',str(probe),'-o',str(W/'probe.dylib')]
build=subprocess.run(command,capture_output=True,text=True)
(W/'build.stderr.log').write_text(build.stderr)
assert build.returncode==0,build.stderr
library=ctypes.CDLL(str(W/'probe.dylib'))
fn=library.sampled_offset;fn.argtypes=[ctypes.c_uint64,ctypes.c_uint64];fn.restype=ctypes.c_uint64
Mi=1024*1024;rng=random.Random(20261019)
sizes=[64*Mi+1,65*Mi,4*1024**3+1,1024**4,(1<<64)-1]+[rng.randrange(64*Mi+1,1<<64) for _ in range(1000)]
checks=0
for size in sizes:
    starts=[(size-Mi)*b//3 for b in range(4)]
    assert starts[0]==0 and starts[-1]+Mi==size
    assert all(starts[i]+Mi<=starts[i+1] for i in range(3))
    for block in range(4):
        for within in [0,1,Mi//2,Mi-1]:
            assert fn(size,block*Mi+within)==starts[block]+within
            checks+=1
names=['windirstat/ItemDupe.cpp','windirstat/ItemDupe.h',
       'windirstat/Controls/FileDupeControl.cpp','windirstat/Controls/FileDupeControl.h']
paths=subprocess.check_output(['git','-C',str(S),'ls-tree','-r','--name-only','HEAD'],text=True).splitlines()
result=dict(status='passed_extracted_original_sample_offset_expression',repository='windirstat/windirstat',
    commit='a92886ff41934946d306909c01ee4447f3b60580',tracked_blobs=len(paths),
    whole_files_read={n:hashlib.sha256((S/n).read_bytes()).hexdigest() for n in names},
    partial_file={'path':'windirstat/Item.Extended.cpp','sha256':hashlib.sha256(source.read_bytes()).hexdigest(),
                  'scope':'Entire GetFileHash method from line946; not whole file'},
    original_expression=expression,independent_offset_checks=checks,synthetic_sizes=len(sizes),
    findings=['Size buckets promote candidates through4KiB,1MiB,fullhash levels; selection keys include logical size plus hash.',
              'Optional sampled final level applies above64MiB and reads4MiB total: prefix plus three spaced1MiB blocks ending atEOF.',
              'CItemDupe caption explicitly labels sampled groups probable duplicates; displayed group alone is not exact-byte comparison.',
              'GetFileHash returns XXH3_64digest or up to16bytes from selected BCrypt hash; no byte-comparison step in examined publication path.',
              'Open comment says avoid active writers, but actual flags permit FILE_SHARE_WRITE/DELETE; no concurrent-mutation test yet.'],
    limits=['Only original arithmetic expression compiled, types/cursor inputs supplied by portable harness; not full GetFileHash or Windows API execution.',
            'No original upstream tests, physical file reads, locking/cancellation/GUI/platform/performance or whole repository qualification.',
            'Four whole files read; GetFileHash excerpt does not grant whole Item.Extended.cpp credit.',
            'No GPL code copied into production; pHash native investigation remains deferred.'])
(W/'result.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps({'tracked_blobs':len(paths),'whole_files':len(names),'offset_checks':checks}))
