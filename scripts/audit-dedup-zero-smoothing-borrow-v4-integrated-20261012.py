#!/usr/bin/env python3
import hashlib,json,re,shutil,subprocess
from collections import Counter
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
EXPECTED=set('lib candidate_union_files gradient_candidate_smoothing local_collection'.split())
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 mp=D/'dedup-zero-smoothing-borrow-source-v4-20261012.json';m=json.loads(mp.read_text());s=Path(m['snapshot']);assert all(sha(s/k)==v for k,v in m['file_hashes'].items());log=T/'rrrah-zero-smoothing-borrow-v4-integrated-tests-20261012.log';text=log.read_text();assert 'test result: FAILED'not in text
 matches=re.findall(r'Running (?:unittests src/lib\.rs|tests/([^\s]+)\.rs) \(([^)]+)\)',text);names=[name or'lib'for name,_ in matches];assert len(names)==len(EXPECTED)and set(names)==EXPECTED
 summaries=re.findall(r'test result: ok\. (\d+) passed; 0 failed; (\d+) ignored;',text);assert len(summaries)==len(EXPECTED)
 out=D/'dedup-zero-smoothing-borrow-v4-integrated-verification-20261012.json';saved=T/'rrrah-zero-smoothing-borrow-v4-integrated-executables-20261012';assert not out.exists()and not saved.exists();saved.mkdir();expected_passed=Counter();expected_ignored=Counter();suites=[];pins={str(p):sha(p)for p in [mp,log,Path(__file__)]}
 for name,binary in matches:
  name=name or'lib';binary=Path(binary);before=sha(binary);listed=subprocess.check_output([str(binary),'--list'],text=True);ignored=subprocess.check_output([str(binary),'--list','--ignored'],text=True);assert sha(binary)==before
  all_names=re.findall(r'^(.*): test$',listed,re.M);ignored_names=re.findall(r'^(.*): test$',ignored,re.M);assert len(all_names)==len(set(all_names))and set(ignored_names).issubset(all_names);passed=[x for x in all_names if x not in ignored_names];expected_passed.update(passed);expected_ignored.update(ignored_names)
  dest=saved/name;shutil.copy2(binary,dest);assert sha(dest)==before;pins[str(dest)]=before;suites.append(dict(suite=name,passed=len(passed),ignored=ignored_names,binary=str(dest),sha256=before))
 assert Counter(re.findall(r'^test (\S+) \.\.\. ok$',text,re.M))==expected_passed;assert Counter(re.findall(r'^test (\S+) \.\.\. ignored(?:,.*)?$',text,re.M))==expected_ignored
 assert sum(int(x)for x,_ in summaries)==sum(expected_passed.values())and sum(int(x)for _,x in summaries)==sum(expected_ignored.values())
 assert all(sha(s/k)==v for k,v in m['file_hashes'].items())
 out.write_text(json.dumps({'status':'verified_isolated_zero_borrow_v4_integrated_gate','passed':sum(expected_passed.values()),'ignored':sum(expected_ignored.values()),'suites':suites,'pins':pins,'scope':'Isolated v4 candidate, not adopted. All decode-feature units and three existing integration suites; exact test names, frozen source and preserved binaries verified. Five ignored manual controls are not passes. Includes explicit bothzero collection regression, both input orders and positive/unrelated pairs. No wholepairRSS/Linuxcandidate/fullcorpus/full500qualification.'},indent=2)+'\n');print('Verified',sum(expected_passed.values()),'passes;',sum(expected_ignored.values()),'ignored;',len(suites),'suites')
if __name__=='__main__':main()
