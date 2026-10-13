"""Close the reproduced entry-cancellation defect only after frozen runtime gates."""
import hashlib,json,re,shutil
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1];D=ROOT/'docs/research'
OUT=D/'dedup-owner-entry-cancellation-audit-20261011.json';assert not OUT.exists()
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
mp=D/'dedup-owner-entry-snapshot-20261011.json';m=json.loads(mp.read_text());assert len(m['file_hashes'])==3057
assert all(sha(Path(m['snapshot'])/p)==h for p,h in m['file_hashes'].items())
prefixp=D/'dedup-owner-entry-macos-prefix-audit-20261011.json';prefix=json.loads(prefixp.read_text())
assert prefix['status']=='complete_macos_entry_cancellation_prefix_only'
assert all(sha(p)==h for p,h in prefix['pins'].items())
assert all(sha(r['path'])==r['sha256'] for r in prefix['artifacts'])
assert sha(ROOT/'crates/rrrah-dedup/src/local_index.rs')==m['file_hashes']['crates/rrrah-dedup/src/local_index.rs']
current_delta=[p for p,h in m['file_hashes'].items() if p.startswith('crates/rrrah-dedup/') and sha(ROOT/p)!=h]
current_delta+=sorted(str(p.relative_to(ROOT)) for p in (ROOT/'crates/rrrah-dedup').rglob('*') if p.is_file() and str(p.relative_to(ROOT)) not in m['file_hashes'])
beforep=D/'dedup-owner-entry-before-20261011.json';before=json.loads(beforep.read_text());assert before['command_exit_code']==101
assert sha(before['source'])==before['source_sha256']
assert sha(ROOT/before['failure_log'])==before['failure_log_sha256']
assert 'pre-cancelled operation consumed the input iterator' in (ROOT/before['failure_log']).read_text()
def inspect(log,expected):
    current=None;found={};binaries=[]
    for line in log.read_text().splitlines():
        running=re.search(r'Running (unittests src/lib.rs|tests/([^ ]+)\.rs) \(([^)]+)\)',line)
        if running:
            current=running[2] or 'lib';binaries.append(running[3])
        result=re.search(r'test result: ok\. (\d+) passed; 0 failed;',line)
        if result:
            assert current is not None and current not in found
            found[current]=int(result[1]);current=None
    assert found==expected,(log,found,expected)
    assert 'descriptor_pair_entry_cancellation_does_not_consume_input ... ok' in log.read_text()
    return binaries
raster=D/'dedup-owner-entry-cancellation-raster-20261011.log'
mac=D/'dedup-owner-entry-cancellation-macos-frozen-20261011.log'
linux=D/'dedup-owner-entry-cancellation-linux-20261011.log'
raster_bins=inspect(raster,{'lib':25,'local_index':6})
mac_bins=inspect(mac,{'lib':65,'local_index':7})
linux_counts={'lib':65,'local_index':7,'model_compare':11,'model_input':6,'model_input_file':5,'model_tiles':9,'search_portfolio':8,'proposals':6,'portfolio_collection':10}
linux_bins=inspect(linux,linux_counts)
archive=Path('/Users/themoretheless/.codex/tmp/rrrah-owner-entry-qualified-20261011');assert not archive.exists()
artifacts=[]
for variant,binaries in [('linux-frozen-decode',linux_bins)]:
    for binary in binaries:
        if variant.startswith('linux'):
            p=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-linux-native')/Path(binary).relative_to('/validation')
        else:
            p=Path(binary) if Path(binary).is_absolute() else ROOT/binary
        assert p.is_file() and not p.is_symlink(),p
        artifacts.append((variant,p,sha(p)))
archive.mkdir();records=list(prefix['artifacts'])
for variant,p,h in artifacts:
    dest=archive/variant/p.name;dest.parent.mkdir(exist_ok=True);shutil.copy2(p,dest);assert sha(dest)==sha(p)==h
    records.append({'variant':variant,'path':str(dest),'sha256':h})
files=[prefixp,mp,beforep,raster,mac,linux,Path(__file__),Path('/Users/themoretheless/.codex/tmp/rrrah-owner-entry-linux-20261011.sh')]
a={'status':'complete_bounded_frozen_macos_linux_entry_cancellation_qualification','current_root_delta_not_qualified':current_delta,'reproduced_failure_exit':101,'snapshot_files':3057,'macos_raster_tests':31,'macos_decode_tests':72,'linux_decode_tests':sum(linux_counts.values()),'linux_suite_counts':linux_counts,'artifacts':records,'pins':{str(p):sha(p) for p in files},'scope':'Already-cancelled valid descriptor-pair operations stop before consuming their input iterator in legacy and managed paths. The decode regression observes zero managed peak. Checkpoints follow materialization and final pair sorting. Existing managed first/middle/final cancellation, capacity and independent exhaustive-index parity gates also pass.','limits':'Finite macOS and native aarch64 Linux generic decode gates; no Linux Torch or Windows native runtime. Sort/allocator operations remain indivisible. Full 229-positive producer runs an earlier frozen source and needs separate numerical-equivalence qualification, not attribution to these cancellation tests.'}
OUT.write_text(json.dumps(a,indent=2)+'\n');print(a['status'])
