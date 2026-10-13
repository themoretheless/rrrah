#!/usr/bin/env python3
import hashlib,json,re,shutil
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 out=D/'dedup-pixel-search-information-verification-20261012.json';copies=T/'rrrah-pixel-search-information-executables-20261012';assert not out.exists()and not copies.exists()
 mp=D/'dedup-exposure-recovery-search-snapshot-20261012.json';m=json.loads(mp.read_text());s=Path(m['snapshot']);assert all(sha(s/k)==v for k,v in m['file_hashes'].items())
 for name in ['pixel_search','information']:assert sha(R/f'crates/rrrah-dedup/tests/{name}.rs')==sha(s/f'crates/rrrah-dedup/tests/{name}.rs')
 log=T/'rrrah-pixel-search-information-gate-20261012.log';text=log.read_text();assert re.findall(r'test result: ok\. (\d+) passed; 0 failed; 0 ignored; 0 measured; 0 filtered out',text)==['5','16']
 required=['indexed_final_generation_and_transient_cancel_return_no_report','changed_source_discards_every_stale_decision_and_preserves_healthy_pairs','indexed_changed_source_discards_stale_pairs','indexed_source_changed_before_its_decode_is_not_admitted_as_fresh','indexed_unique_collection_fits_zero_pair_budget_and_decodes_once_per_key','informative_hash_collision_remains_candidate_and_fails_full_resolution_confirmation','standalone_pixel_search_recovers_equal_transparent_files_without_visual_candidates']
 assert all(name+' ... ok'in text for name in required)
 binaries=re.findall(r'Running tests/(information|pixel_search)\.rs \(([^)]+)\)',text);assert len(binaries)==2;copies.mkdir();pins={str(p):sha(p)for p in [mp,log,Path(__file__)]}
 for name,path in binaries:
  p=copies/name;shutil.copy2(path,p);pins[str(p)]=sha(p)
 out.write_text(json.dumps({'status':'verified_21_frozen_source_pixel_search_information_tests','tests':21,'suites':{'information':5,'pixel_search':16},'pins':pins,'scope':'Existing macOS frozen184 production and current-identical test sources: selected-frame exhaustive/indexed source invalidation, final owner/caller cancellation, diagnostics, HDR authority, uniform/collision gates and pixel discovery. Separate21-test gate, not fresh205 full suite or all-platform/RSS/all500 qualification; no novel palette-specific cancellation guarantee claimed.'},indent=2)+'\n');print('Verified21 existing pixel search/information tests')
if __name__=='__main__':main()
