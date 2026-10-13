#!/usr/bin/env python3
import hashlib,json,re,shutil
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 out=D/'dedup-exposure-recovery-search-verification-20261012.json';assert not out.exists();mp=D/'dedup-exposure-recovery-search-snapshot-20261012.json';m=json.loads(mp.read_text());s=Path(m['snapshot']);assert all(sha(s/f)==h for f,h in m['file_hashes'].items());assert sha(m['parent_manifest'])==m['parent_sha256']
 log=D/'dedup-exposure-recovery-search-tests-20261012.log';text=log.read_text();assert 'FAILED' not in text
 expected={'anchor_rank':5,'exposure':5,'exposure_files':6,'model_compare':39,'model_tiles':21,'search_portfolio':35,'portfolio_collection':36,'joint_witnesses':10,'geometry':9,'projective_domains':4,'joint_projective':4,'projective_pruning':3,'rank_region':7}
 entries=re.findall(r'Running tests/(\w+)\.rs \(([^\n]+)\)\n(.*?)(?=\n\s*Running tests/|\Z)',text,re.S);assert len(entries)==len(expected)
 folder=T/'rrrah-exposure-recovery-search-executables-20261012';assert not folder.exists();folder.mkdir();suites=[]
 for name,binary,body in entries:
  count=re.findall(r'test result: ok\. (\d+) passed; 0 failed;',body);assert count==[str(expected[name])],(name,count);saved=folder/name;shutil.copy2(binary,saved);suites.append({'suite':name,'tests':expected[name],'binary':str(saved),'binary_sha256':sha(saved)})
 for f in ['src/anchor_rank_file.rs','src/exposure.rs','src/model_banks.rs','src/model_loftr.rs','tests/model_compare.rs','tests/exposure_files.rs','tests/exposure.rs']:assert sha(R/'crates/rrrah-dedup'/f)==sha(s/'crates/rrrah-dedup'/f)
 assert sum(expected.values())==184
 report={'status':'verified_184_tests_guarded_primary_then_exposure_recovery','tests':184,'suites':suites,'pins':{str(mp):sha(mp),str(log):sha(log),str(Path(__file__)):sha(__file__)},'scope':'Pinned compatible source child; ordinary bank then calibrated overlap recovery plus legacy/file/model/portfolio/collection gate. Five new controls verify primary support skips recovery, shared-provider34-call recovery after primary miss, complete suffix/summed work admission beforeIO, atomic final-secondary mutation/both owners and final outer weight/owner guards on primary early support. Existing nine-region and file-resource controls included. No default search adopts calibration. Actual CPU/negative precision/allformats/platforms/RSS/full500 qualification separate.'};out.write_text(json.dumps(report,indent=2)+'\n');print(report['status'])
if __name__=='__main__':main()
