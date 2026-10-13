#!/usr/bin/env python3
import hashlib,json,re,shutil
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 out=D/'dedup-exposure-file-guards-verification-20261012.json';assert not out.exists();mp=D/'dedup-exposure-file-guards-snapshot-20261011.json';m=json.loads(mp.read_text());assert all(sha(p)==h for p,h in m['pins'].items());parent=Path(m['library_manifest']);assert sha(parent)==m['library_manifest_sha256'];p=json.loads(parent.read_text());s=Path(p['snapshot']);assert all(sha(s/f)==h for f,h in p['file_hashes'].items());old=D/'dedup-exposure-anchor-files-verification-20261011.json';qualified=json.loads(old.read_text());assert qualified['tests']==172 and all(sha(p)==h for p,h in qualified['pins'].items())
 log=D/'dedup-exposure-file-guards-tests-20261011.log';text=log.read_text();assert re.findall(r'test result: ok\. (\d+) passed; 0 failed;',text)==['6'];source=Path(m['harness'])/'tests/exposure_files.rs';assert sha(source)==sha(R/'crates/rrrah-dedup/tests/exposure_files.rs')
 binary=Path(re.search(r'Running tests/exposure_files\.rs \(([^\n]+)\)',text).group(1));folder=T/'rrrah-exposure-file-guards-qualified-20261012';assert not folder.exists();folder.mkdir();saved=folder/'exposure_files';shutil.copy2(binary,saved)
 paths=[mp,parent,old,log,source,saved,Path(__file__)];report={'status':'verified_six_file_tests_three_additional_resource_owner_controls','tests':6,'additional_tests':3,'pins':{str(p):sha(p)for p in paths},'scope':'Test-only standalone harness against unchanged172-gate production source. Three existing file tests repeated; three additional tests verify managed calibration scratch refusal after legacy decode succeeds under same cap, zero-exposure refusal, both request-owner cancellations mid-operation and final publication. All returns release managed credit. No full175-test rerun, independent corpus/RSS/format/platform/all500 qualification.'};out.write_text(json.dumps(report,indent=2)+'\n');print(report['status'])
if __name__=='__main__':main()
