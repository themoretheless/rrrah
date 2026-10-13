#!/usr/bin/env python3
import hashlib,json,re,shutil,subprocess
from collections import Counter
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
EXPECTED=set('lib gradient_candidate_smoothing gradient gradient_scale gradient_scale_files candidate_union_files correspondence_union native_anchor_recovery anchor_reflection_fallback anchor_fallback_collection_lifecycle anchor_reflection_collection_lifecycle search_portfolio portfolio_collection model_compare model_tiles pixel_search joint_projective projective_domains'.split())
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 mp=D/'dedup-native-row-ring-snapshot-20261012.json';m=json.loads(mp.read_text());s=Path(m['snapshot']);assert all(sha(s/k)==v for k,v in m['file_hashes'].items());log=T/'rrrah-native-row-ring-integrated-tests-20261012.log';text=log.read_text();assert 'test result: FAILED'not in text
 matches=re.findall(r'Running (?:unittests src/lib\.rs|tests/([^\s]+)\.rs) \(([^)]+)\)',text);names=[name or'lib'for name,_ in matches];assert len(names)==len(EXPECTED)and set(names)==EXPECTED
 summaries=re.findall(r'test result: ok\. (\d+) passed; 0 failed; (\d+) ignored;',text);assert len(summaries)==len(EXPECTED)
 out=D/'dedup-native-row-ring-integrated-verification-20261012.json';saved=T/'rrrah-native-row-ring-integrated-executables-20261012';assert not out.exists()and not saved.exists();saved.mkdir();expected_passed=Counter();expected_ignored=Counter();suites=[];pins={str(p):sha(p)for p in [mp,log,Path(__file__)]}
 for name,binary in matches:
  name=name or'lib';binary=Path(binary);before=sha(binary);listed=subprocess.check_output([str(binary),'--list'],text=True);ignored=subprocess.check_output([str(binary),'--list','--ignored'],text=True);assert sha(binary)==before
  all_names=re.findall(r'^(.*): test$',listed,re.M);ignored_names=re.findall(r'^(.*): test$',ignored,re.M);assert len(all_names)==len(set(all_names))and set(ignored_names).issubset(all_names);passed=[x for x in all_names if x not in ignored_names];expected_passed.update(passed);expected_ignored.update(ignored_names)
  if name!='lib':assert sha(R/'crates/rrrah-dedup/tests'/f'{name}.rs')==sha(s/'crates/rrrah-dedup/tests'/f'{name}.rs')
  dest=saved/name;shutil.copy2(binary,dest);assert sha(dest)==before;pins[str(dest)]=before;suites.append(dict(suite=name,passed=len(passed),ignored=ignored_names,binary=str(dest),sha256=before))
 assert Counter(re.findall(r'^test (\S+) \.\.\. ok$',text,re.M))==expected_passed;assert Counter(re.findall(r'^test (\S+) \.\.\. ignored(?:,.*)?$',text,re.M))==expected_ignored
 assert sum(int(x)for x,_ in summaries)==sum(expected_passed.values())and sum(int(x)for _,x in summaries)==sum(expected_ignored.values())
 for k,v in m['file_hashes'].items():
  if k.startswith('crates/rrrah-dedup/src/'):assert sha(R/k)==v,k
 assert all(sha(s/k)==v for k,v in m['file_hashes'].items())
 out.write_text(json.dumps({'status':'verified_complete_native_row_ring_integration_gate','passed':sum(expected_passed.values()),'ignored':sum(expected_ignored.values()),'suites':suites,'pins':pins,'scope':'Current-identical dedup production source and selected17 integration suites plus all decode-feature units on compatible frozen dependencies. Covers native smoothing/scales/union/anchor fallback and reflected collection source/owner/caller cancellation/mutation/retry plus portfolio/model-bank/collection/geometry/pixel gates. Five ignored pre-existing controls are listed per suite; no all-feature/dirty workspace/Windows/full corpus/heldout precision/process cap/full500 completion claim.'},indent=2)+'\n');print('Verified',sum(expected_passed.values()),'passes;',sum(expected_ignored.values()),'ignored;',len(suites),'suites')
if __name__=='__main__':main()
