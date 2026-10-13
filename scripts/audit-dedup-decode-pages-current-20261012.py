#!/usr/bin/env python3
import hashlib,json,re,shutil,subprocess
from collections import Counter
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 mp=D/'dedup-decode-pages-current-source-20261012.json';m=json.loads(mp.read_text());s=Path(m['snapshot']);parent=D/m['parent_manifest'];assert sha(parent)==m['parent_sha256'];p=json.loads(parent.read_text());assert all(sha(s/k)==v for k,v in p['file_hashes'].items())
 for k,v in p['file_hashes'].items():
  if k.startswith('crates/rrrah-dedup/src/'):assert sha(R/k)==v,k
 log=T/'rrrah-dedup-decode-pages-current-tests-20261012.log';text=log.read_text();assert 'test result: FAILED'not in text;matches=re.findall(r'Running tests/(\w+)\.rs \(([^)]+)\)',text);assert len(matches)==11 and set(n for n,_ in matches)==set(m['suites']);summaries=re.findall(r'test result: ok\. (\d+) passed; 0 failed; (\d+) ignored;',text);assert len(summaries)==11
 out=D/'dedup-decode-pages-current-verification-20261012.json';saved=T/'rrrah-dedup-decode-pages-current-executables-20261012';assert not saved.exists()and not out.exists();saved.mkdir();pins={str(k):sha(k)for k in [mp,parent,log,Path(__file__)]};passed=Counter();ignored=Counter();suites=[]
 for n,b in matches:
  k=Path('crates/rrrah-dedup/tests')/(n+'.rs');assert sha(s/k)==sha(R/k)==m['test_hashes'][n];binary=Path(b);before=sha(binary);ls=subprocess.check_output([str(binary),'--list'],text=True);ig=subprocess.check_output([str(binary),'--list','--ignored'],text=True);all_names=re.findall(r'^(.*): test$',ls,re.M);ignore=re.findall(r'^(.*): test$',ig,re.M);assert len(all_names)==len(set(all_names))and set(ignore).issubset(all_names);good=[x for x in all_names if x not in ignore];passed.update(good);ignored.update(ignore);dest=saved/n;shutil.copy2(binary,dest);assert sha(binary)==sha(dest)==before;pins[str(dest)]=before;suites.append({'suite':n,'passed':len(good),'ignored':ignore})
 assert Counter(re.findall(r'^test (\S+) \.\.\. ok$',text,re.M))==passed;assert Counter(re.findall(r'^test (\S+) \.\.\. ignored(?:,.*)?$',text,re.M))==ignored;assert sum(int(x)for x,_ in summaries)==sum(passed.values());assert sum(int(x)for _,x in summaries)==sum(ignored.values())
 out.write_text(json.dumps({'status':'verified_current_source_eleven_decode_pages_color_container_suites','passed':sum(passed.values()),'ignored':sum(ignored.values()),'suites':suites,'pins':pins,'scope':'Current-identical dedup production and11selected test files with frozen compatible dependencies. Actual fixture decoding, pages/animation, ICC oracle, linear precision/alpha/orientation, palette mutation, container policies and lifecycle controls represented by exact listed tests. Does not qualify all100formats, every animation/page/layer/ICC case, dirty workspace, Windows/fullLinux, full458positives/heldout precision or full500review.'},indent=2)+'\n');print('Verified',sum(passed.values()),'passes;',sum(ignored.values()),'ignored;11decode/pages/color/container suites')
if __name__=='__main__':main()
