#!/usr/bin/env python3
from pathlib import Path
import hashlib
import json
import re
import subprocess

ROOT = Path(__file__).resolve().parents[1]
DATA = ROOT / 'docs/research/dedup-500'
WORK = Path('/private/tmp/rrrah-kiwi387-controls-20261015')
SOURCE = DATA / 'evidence/Fingertips--FTpHashKiwiMatcher'
MANIFEST = DATA / 'kiwi387-complete-source-20261015.json'
manifest = json.loads(MANIFEST.read_text())

def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()

for row in manifest['files']:
    assert sha(ROOT / row['local']) == row['sha256']
assert sha(WORK / 'FTpHashKiwiMatcher.h') == sha(SOURCE / '02-FTpHashKiwiMatcher.h')
assert sha(WORK / 'FTpHashKiwiMatcher.mm') == sha(SOURCE / '03-FTpHashKiwiMatcher.mm')
for local, archive in [('KWMatcher.h','kiwi387-KWMatcher-adapter-20261015.h'), ('pHash.h','kiwi387-pHash-adapter-20261015.h'), ('probe.mm','kiwi387-controls-20261015.mm')]:
    assert sha(WORK / local) == sha(DATA / archive)
text = (WORK / 'run.log').read_text()
assert 'CHECKS=34 DEFAULT_THRESHOLD=0.999 STALE_POSITIVE_SCORE=0.25' in text
run = subprocess.run([str(WORK / 'probe')], capture_output=True, text=True)
assert run.returncode == 0 and run.stdout == text
syntax = {}
for label, command in [('installer', ['/bin/sh','-n',str(SOURCE / '06-install-dependencies')]), ('podspec', ['/usr/bin/ruby','-c',str(SOURCE / '04-matcher.podspec')])]:
    result = subprocess.run(command, capture_output=True, text=True)
    assert result.returncode == 0
    syntax[label] = {'command':command,'returncode':result.returncode}
OUTPUT = DATA / 'kiwi387-controls-audit-20261015.json'
assert not OUTPUT.exists()
OUTPUT.write_text(json.dumps({
    'status':'verified34_original_objcxx_wrapper_controls_with_real_foundation',
    'auditor_sha256':sha(__file__), 'source_manifest_sha256':sha(MANIFEST),
    'executable':str(WORK / 'probe'),'executable_sha256':sha(WORK / 'probe'),
    'source_adapters':{str(p.relative_to(ROOT)):sha(p) for p in [DATA/'kiwi387-controls-20261015.mm',DATA/'kiwi387-KWMatcher-adapter-20261015.h',DATA/'kiwi387-pHash-adapter-20261015.h']},
    'logs':{str(p):sha(p) for p in WORK.glob('*.log')},
    'checks':34, 'compiler':subprocess.check_output(['/usr/bin/clang++','--version'],text=True),
    'compiler_warnings':'10 existing original-source semicolon-before-method-body warnings retained; no -Werror or warning suppression used',
    'syntax_only_checks':syntax,
    'observations':['Actual default forwarded threshold0.999 differs from header0.999999','Negative score0.25 retained in ccp property after later positive backend result1.0, so shouldNot diagnostic prints stale score','Real Foundation class validation/exceptions and both digest-error checkpoints exercised','Invalid -1/2/NaN/infinite thresholds delegated unchanged to scripted backend','Nil path forwarded as null C pointer, mutable NSString path remains aliased, non-file NSURL wrapper forwards path component'],
    'scope':'Both pinned Objective-C++ implementation and header compiled unchanged with actual Apple Foundation/ARC and explicit authored KWMatcher base/pHash API scripted adapters. No actual Kiwi registration, pHash ABI/hash algorithm, image decode, memory ownership, complete package/install/Homebrew/platform or perceptual quality qualification. Synthetic path strings only; no image files, downloads, deletion or source adoption. Fresh replay confirms same34checks.'
},indent=2)+'\n')
print('Original matcher:34realFoundation controls verified; dependency adapters explicit')
