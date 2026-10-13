"""Qualify the isolated owner-count cancellation delta on current macOS source."""
import hashlib,json,re,shutil
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1];D=ROOT/'docs/research'
OUT=D/'dedup-owner-count-cancellation-audit-20261011.json';assert not OUT.exists()
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
mp=D/'dedup-owner-count-cancellation-source-20261011.json';m=json.loads(mp.read_text())
assert all(sha(ROOT/p)==h for p,h in m['dedup_source_hashes'].items())
assert m['changes']==['crates/rrrah-dedup/src/local_index.rs']
assert sha(m['archived_delta'])==m['dedup_source_hashes'][m['changes'][0]]
assert sha(ROOT/m['parent_manifest'])==m['parent_manifest_sha256']
logs=[D/'dedup-owner-count-cancellation-raster-current-20261011.log',D/'dedup-owner-count-cancellation-decode-20261011.log']
archive=Path('/Users/themoretheless/.codex/tmp/rrrah-owner-count-cancellation-qualified-20261011');assert not archive.exists()
artifacts=[]
for log,expected in zip(logs,[[24,6],[64,7]]):
 text=log.read_text();assert [int(x) for x in re.findall(r'test result: ok\. (\d+) passed; 0 failed;',text)]==expected
 assert 'legacy_owner_count_stops_when_iterator_requests_cancellation ... ok' in text
 assert 'legacy_owner_materialization_honors_cancellation ... ok' in text
 bins=re.findall(r'Running (?:unittests|tests/[^\s]+)[^\n]*\((target/debug/deps/[^)]+)\)',text)
 assert len(bins)==2,(log,bins)
 for binary in bins:
  p=ROOT/binary;assert p.is_file() and not p.is_symlink();artifacts.append((p,sha(p)))
archive.mkdir()
records=[]
for p,h in artifacts:
 dest=archive/p.name;shutil.copy2(p,dest);assert sha(dest)==h==sha(p);records.append({'path':str(dest),'sha256':h})
a={'status':'complete_bounded_macos_owner_count_cancellation_qualification','source_manifest':str(mp),'raster_tests':30,'decode_tests':71,'raster_suite_counts':[24,6],'decode_suite_counts':[64,7],'archived_test_binaries':records,'pins':{str(p):sha(p) for p in [mp,*logs,Path(__file__)]},'scope':'Legacy owner materialization and both ordered count variants observe the supplied cancellation callback. Mid-iteration regression stops before the third update. Existing exhaustive-retrieval and capacity suites pass. The only library delta from the parent feature-gate snapshot is local_index.rs.','limits':'Current macOS runtime qualification only; earlier Linux and Windows gates predate this delta. Sort/allocator calls are indivisible. Full 229-pair native corpus uses its own earlier frozen binary and does not qualify this later cancellation change.'}
OUT.write_text(json.dumps(a,indent=2)+'\n');print(a['status'])
