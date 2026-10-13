#!/usr/bin/env python3
import hashlib,json,re,shutil
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 out=D/'dedup-palette-collection-verification-20261012.json';copy=T/'rrrah-palette-collection-qualified-20261012';assert not out.exists()and not copy.exists()
 parent=D/'dedup-exposure-recovery-search-snapshot-20261012.json';m=json.loads(parent.read_text());s=Path(m['snapshot']);assert all(sha(s/k)==v for k,v in m['file_hashes'].items())
 h=T/'rrrah-palette-collection-harness-20261012';root_test=R/'crates/rrrah-dedup/tests/palette_transparency.rs';assert sha(root_test)==sha(h/'tests/palette_transparency.rs')
 previous=D/'dedup-palette-transparency-snapshot-20261012.json';old=json.loads(previous.read_text());old_frozen=T/'rrrah-palette-transparency-harness-20261012/tests/palette_transparency.rs';assert sha(old_frozen)==old['pins'][str(root_test)]
 fixture=R/'crates/rrrah-dedup/tests/fixtures/palette-transparency';assert all(sha(h/'tests/fixtures/palette-transparency'/p.name)==sha(p)==old['pins'][str(p)] for p in fixture.glob('*.png'))
 log=T/'rrrah-palette-collection-tests-20261012.log';text=log.read_text();assert 'test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out'in text;assert 'indexed_palette_collection_matches_exhaustive_without_uniform_false_edges ... ok'in text
 executable=Path(re.search(r'Running tests/palette_transparency\.rs \(([^)]+)\)',text).group(1));shutil.copy2(executable,copy)
 paths=[parent,previous,old_frozen,h/'Cargo.toml',h/'Cargo.lock',h/'tests/palette_transparency.rs',log,copy,Path(__file__)]+list((h/'tests/fixtures/palette-transparency').glob('*.png'));pins={str(p):sha(p)for p in paths}
 out.write_text(json.dumps({'status':'verified_three_real_png_tests_including_indexed_exhaustive_collection_parity','tests':3,'pins':pins,'collection_expected_equal_pairs':[[1,2],[1,3],[2,3]],'exhaustive_different_pairs':12,'index_candidate_pairs':3,'orders':2,'historical_root_test_pin':'Prior2-test manifest root test path has intentionally changed by adding third test; original2-test bytes remain verified in prior immutable harness. Historical manifest/log/report are preserved; new3-test frozen harness supersedes current-root qualification.','scope':'Original6 PNG collection, direct exhaustive vs indexed parity in both orders, no false uniform/visible-alpha edges and released managed credit. Production184 source unchanged. Not a fresh187 full gate, broad PNG/platform/RSS/all500 qualification.'},indent=2)+'\n');print('Verified3 tests, two orders,3equal and12different exhaustive pairs')
if __name__=='__main__':main()
