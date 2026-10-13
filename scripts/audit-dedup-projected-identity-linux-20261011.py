#!/usr/bin/env python3
"""Archive the completed Linux generic identity-candidate gate, not later root changes."""
from pathlib import Path
import hashlib
import json
import re
import shutil

ROOT=Path(__file__).resolve().parents[1]
D=ROOT/'docs/research'
LOG=D/'dedup-projected-identity-linux-20261011.log'
MANIFEST=D/'dedup-projected-loftr-identity-snapshot-20261011.json'
ARCHIVE=Path('/Users/themoretheless/.codex/tmp/rrrah-projected-identity-linux-qualified-20261011')
VALIDATION=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-linux-native')
HELPER=Path('/Users/themoretheless/.codex/tmp/rrrah-projected-identity-linux-20261011.sh')

def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()

def main():
    assert not ARCHIVE.exists();ARCHIVE.mkdir()
    text=LOG.read_text();counts=[int(n) for n in re.findall(r'test result: ok\. (\d+) passed;',text)]
    assert counts==[72,7,11,9,8,8,9,10,6,8],counts
    manifest=json.loads(MANIFEST.read_text());snapshot=Path(manifest['snapshot'])
    assert all(sha(snapshot/p)==h for p,h in manifest['file_hashes'].items())
    binaries=re.findall(r'Running [^\n]*\((/validation/target/debug/deps/[^)]+)\)',text)
    assert len(binaries)==10,binaries
    artifacts=[]
    for name in binaries:
        source=VALIDATION/Path(name).relative_to('/validation');target=ARCHIVE/source.name
        expected=sha(source);assert source.read_bytes()[:4]==b'\x7fELF'
        shutil.copy2(source,target);assert sha(target)==expected
        artifacts.append({'source':str(source),'archive':str(target),'sha256':expected})
    for source in [LOG,MANIFEST,HELPER,Path(__file__)]:
        target=ARCHIVE/source.name;expected=sha(source);shutil.copy2(source,target);assert sha(target)==expected
        artifacts.append({'source':str(source),'archive':str(target),'sha256':expected})
    delta=[p for p,h in manifest['file_hashes'].items() if p.startswith('crates/rrrah-dedup/') and sha(ROOT/p)!=h]
    report={'status':'complete_bounded_frozen_linux_projected_identity_generic_gate','tests':sum(counts),'suite_counts':counts,'artifacts':artifacts,'snapshot_files':len(manifest['file_hashes']),'later_root_delta_not_qualified':delta,'scope':'Linux aarch64 decode-feature generic execution from frozen identity snapshot. No Linux Torch runtime or later insufficient-anchor recovery qualification.'}
    (D/'dedup-projected-identity-linux-audit-20261011.json').write_text(json.dumps(report,indent=2)+'\n');print(report['status'],report['tests'])

if __name__=='__main__':main()
