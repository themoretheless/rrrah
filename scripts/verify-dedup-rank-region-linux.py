"""Verify terminal native Linux rank suite and complete snapshot bytes."""
import argparse,hashlib,json,re
from pathlib import Path
parser=argparse.ArgumentParser();parser.add_argument('returncode',type=int);parser.add_argument('output',type=Path);args=parser.parse_args();assert args.returncode==0 and not args.output.exists()
base=Path('docs/research');manifest=base/'dedup-rank-region-linux-snapshot.json';log=base/'dedup-rank-region-linux-native.log';digest=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
r=json.loads(manifest.read_text());snapshot=Path(r['snapshot']);pins={str(p):digest(p) for p in [manifest,log,Path(__file__),Path('scripts/run-dedup-linux-rank-region.sh')]}
assert all(digest(snapshot/k)==v for k,v in r['file_hashes'].items())
text=log.read_text();assert re.findall(r'Running tests/([a-z_]+)\.rs',text)==['rank_region']
assert re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out;',text)==[('4','0','0','0','0')]
current={}
for path in [Path('crates/rrrah-dedup/src/lib.rs'),Path('crates/rrrah-dedup/src/rank_region.rs'),Path('crates/rrrah-dedup/tests/rank_region.rs')]:
 assert digest(path)==r['file_hashes'][str(path)];current[str(path)]=digest(path)
assert all(digest(k)==v for k,v in pins.items())
args.output.write_text(json.dumps({'status':'verified_native_linux_rank_region','returncode':0,'tests':4,'verified_snapshot_files':len(r['file_hashes']),'input_hashes':pins,'current_code_test_hashes':current,'scope':'Native Linux aarch64 no-default-features local rank suite; all snapshot bytes verified. Current rank module, lib export and tests match. Measurement examples/Cargo additions excluded; no automatic classifier, broad precision, Windows or Linux file/decode rank qualification.'},indent=2)+'\n')
