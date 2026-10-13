#!/usr/bin/env python3
import hashlib,json,re,shutil
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 out=D/'dedup-bidirectional-exposure-integrated-verification-20261012.json';assert not out.exists();mp=D/'dedup-bidirectional-exposure-atomic-snapshot-20261012.json';m=json.loads(mp.read_text());s=Path(m['snapshot']);assert all(sha(s/f)==h for f,h in m['file_hashes'].items());assert sha(m['parent_manifest'])==m['parent_sha256']
 log=D/'dedup-bidirectional-exposure-integrated-tests-20261012.log';text=log.read_text();assert 'FAILED' not in text
 expected={'anchor_rank':5,'exposure':5,'exposure_files':6,'model_compare':47,'model_tiles':21,'search_portfolio':35,'portfolio_collection':36,'joint_witnesses':10,'geometry':9,'projective_domains':4,'joint_projective':4,'projective_pruning':3,'rank_region':7,'linear':4,'pixels':6,'information':5,'pixel_search':16,'palette_transparency':3,'palette_mutation':1}
 entries=re.findall(r'Running tests/(\w+)\.rs \(([^\n]+)\)\n(.*?)(?=\n\s*Running tests/|\Z)',text,re.S);assert len(entries)==len(expected)
 folder=T/'rrrah-bidirectional-exposure-integrated-executables-20261012';assert not folder.exists();folder.mkdir();suites=[]
 for name,binary,body in entries:
  count=re.findall(r'test result: ok\. (\d+) passed; 0 failed;',body);assert count==[str(expected[name])],(name,count);saved=folder/name;shutil.copy2(binary,saved);suites.append({'suite':name,'tests':expected[name],'binary':str(saved),'binary_sha256':sha(saved)})
 for f in ['src/anchor_rank_file.rs','src/exposure.rs','src/model_banks.rs','src/model_loftr.rs','tests/model_compare.rs','tests/exposure_files.rs','tests/exposure.rs']:assert sha(R/'crates/rrrah-dedup'/f)==sha(s/'crates/rrrah-dedup'/f)
 assert all(sha(R/k)==h for k,h in m['file_hashes'].items()if k.startswith('crates/rrrah-dedup/src/'))
 assert sum(expected.values())==227
 report={'status':'verified_227_tests_integrated_bidirectional_exposure_collection','tests':227,'suites':suites,'pins':{str(mp):sha(mp),str(log):sha(log),str(Path(__file__)):sha(__file__)},'scope':'Fresh19-suite227-test pinned compatible4-crate gate with current-identical rrrah-dedup production source. Includes explicit exhaustive/scheduled two-stage exposure collection, both directed attempts with prepaid reverse work, native-direction traces and final reverse prior-source/owner/caller/provider atomic failure after earlier supported pairs, cumulative8-field plus calibration batch admission before iterator consumption, shared provider, retained report credits and final prior-source/owner atomicity; existing portfolio/collection/geometry/rank/resource/owner/source controls, pixel search/information, canonical linear/pixels and new real PNG palette collection/same-length mutation tests. Core/decode foreign HDR changes excluded. No CPU full-cohort/heldout precision/native-platform/RSS/full500 completion.'};out.write_text(json.dumps(report,indent=2)+'\n');print(report['status'])
if __name__=='__main__':main()
