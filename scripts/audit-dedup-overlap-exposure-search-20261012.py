#!/usr/bin/env python3
import hashlib,json,re,shutil
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 out=D/'dedup-overlap-exposure-search-verification-20261012.json';assert not out.exists();mp=D/'dedup-overlap-exposure-search-snapshot-20261012.json';m=json.loads(mp.read_text());s=Path(m['snapshot']);assert all(sha(s/f)==h for f,h in m['file_hashes'].items());assert sha(m['parent_manifest'])==m['parent_sha256']
 log=D/'dedup-overlap-exposure-search-tests-20261012.log';text=log.read_text();assert 'FAILED' not in text
 expected={'anchor_rank':5,'exposure':5,'exposure_files':6,'model_compare':34,'model_tiles':21,'search_portfolio':35,'portfolio_collection':36,'joint_witnesses':10,'geometry':9,'projective_domains':4,'joint_projective':4,'projective_pruning':3,'rank_region':7}
 entries=re.findall(r'Running tests/(\w+)\.rs \(([^\n]+)\)\n(.*?)(?=\n\s*Running tests/|\Z)',text,re.S);assert len(entries)==len(expected)
 folder=T/'rrrah-exposure-anchor-files-executables-20261012';assert not folder.exists();folder.mkdir();suites=[]
 for name,binary,body in entries:
  count=re.findall(r'test result: ok\. (\d+) passed; 0 failed;',body);assert count==[str(expected[name])],(name,count);saved=folder/name;shutil.copy2(binary,saved);suites.append({'suite':name,'tests':expected[name],'binary':str(saved),'binary_sha256':sha(saved)})
 for f in ['src/anchor_rank_file.rs','src/exposure.rs','src/model_banks.rs','src/model_loftr.rs','tests/model_compare.rs','tests/exposure_files.rs','tests/exposure.rs']:assert sha(R/'crates/rrrah-dedup'/f)==sha(s/'crates/rrrah-dedup'/f)
 assert sum(expected.values())==179
 report={'status':'verified_179_tests_guarded_overlap_exposure_bank_search','tests':179,'suites':suites,'pins':{str(mp):sha(mp),str(log):sha(log),str(Path(__file__)):sha(__file__)},'scope':'Pinned compatible source child; explicit nine-region exposure bank search plus legacy/file/model/portfolio/collection gate. New generic controls verify exact nine provider calls and crop dimensions, supported metadata, cumulative calibration preflight, final-provider mutation/both owners, flat/inverted refusal. File resource controls included. No default search adopts calibration. Actual CPU/negative precision/allformats/platforms/RSS/full500 qualification separate.'};out.write_text(json.dumps(report,indent=2)+'\n');print(report['status'])
if __name__=='__main__':main()
