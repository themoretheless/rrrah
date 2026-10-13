#!/usr/bin/env python3
import hashlib,json,re,shutil
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 mp=D/'dedup-exact-empty-growth-source-20261012.json';m=json.loads(mp.read_text());s=Path(m['snapshot']);assert all(sha(s/k)==v for k,v in m['file_hashes'].items())
 current=R/'crates/rrrah-dedup/src/exact.rs';frozen=s/'src/exact.rs';assert sha(current)==sha(frozen)
 before=T/'rrrah-bidirectional-exposure-atomic-qualified-20261012/crates/rrrah-dedup/src/exact.rs';text=frozen.read_text();start=text.index('    #[test]\n    fn formerly_empty_entries_are_reopened_before_hash_or_equality()');end=text.index('    #[test]',start+10);assert text[:start]+text[end:]==before.read_text()
 log=T/'rrrah-exact-empty-growth-tests-20261012.log';value=log.read_text();assert 'test exact::collision_tests::formerly_empty_entries_are_reopened_before_hash_or_equality ... ok' in value
 results=re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;',value);assert results==[('32','0','2'),('17','0','0')]
 saved=T/'rrrah-exact-empty-growth-executables-20261012';assert not saved.exists();saved.mkdir();binaries=[]
 for pattern in ['rrrah_dedup-*','exact_files-*']:
  matches=[p for p in (s/'target/debug/deps').glob(pattern) if p.is_file() and p.stat().st_mode&0o111 and p.suffix!='.d'];assert len(matches)==1;dest=saved/pattern[:-2];shutil.copy2(matches[0],dest);binaries.append(dest)
 pins={str(s/k):v for k,v in m['file_hashes'].items()};pins.update({str(p):sha(p)for p in [mp,current,before,log,s/'Cargo.lock',Path(__file__),*binaries]})
 out=D/'dedup-exact-empty-growth-verification-20261012.json';assert not out.exists();out.write_text(json.dumps({'status':'verified_empty_growth_refusal_and_49_default_feature_tests','passed':49,'ignored':2,'ignored_scope':'Two pre-existing manual alternating matcher timing controls, not correctness tests.','production_unchanged_except_new_test':True,'pins':pins,'scope':'Standalone current default-feature dedup source:32 unit passes plus17 exact-file integration passes. Stale zero-size metadata is rejected by sampled/full hash and either side of equality; fresh scan retains only healthy empties. No decode/LoFTR/full workspace/platform/RSS/full500 coverage claim.'},indent=2)+'\n');print('Verified 49 default-feature tests; only new private unit test changed exact.rs')
if __name__=='__main__':main()
