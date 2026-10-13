#!/usr/bin/env python3
import hashlib,json,re,subprocess
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research/dedup-500';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 mp=D/'fclones-hasher-source-20261012.json';m=json.loads(mp.read_text());s=Path(m['snapshot']);assert all(sha(s/k)==h for k,h in m['file_hashes'].items())
 cp=D/'fclones-public-path-control-20261012.json';c=json.loads(cp.read_text());assert c['returncode']==0 and all(sha(p)==h for p,h in c['pins'].items())
 exe=T/'rrrah-fclones-public-path-control-executable-20261012';run=subprocess.run([str(exe)],capture_output=True,text=True);assert run.returncode==0 and run.stdout==c['stdout'] and 'non_utf8_roundtrip=true' in run.stdout
 upstream=D/'fclones-upstream-verification-20261012.json';u=json.loads(upstream.read_text());b=T/'rrrah-fclones-upstream-executables-20261012/lib';assert sha(b)==u['pins'][str(b)]
 names=[];logs=[]
 for mod,n in [('path',10),('selector',7)]:
  log=T/f'rrrah-fclones-{mod}-tests-20261012.log';text=log.read_text();assert f'{n} passed; 0 failed; 0 ignored' in text
  rows=re.findall(r'^test (\S+) \.\.\. ok$',text,re.M);assert len(rows)==n and all(x.startswith(mod+'::test::') for x in rows);names+=rows;logs.append(log)
 listed=subprocess.check_output([str(b),'--list'],text=True);assert set(names)=={x for x in re.findall(r'^(.*): test$',listed,re.M) if x.startswith(('path::test::','selector::test::'))}
 files={'fclones/src/path.rs':529,'fclones/src/selector.rs':245,'fclones/src/transform.rs':526};assert all(len((s/p).read_text().splitlines())==n for p,n in files.items())
 out=D/'fclones-path-selector-transform-review-20261012.json';assert not out.exists()
 out.write_text(json.dumps({'status':'verified_targeted_three_file_review_public_path_counterexamples','commit':m['commit'],'fully_read_additional_files':files,'original_tests_passed':17,'tests':sorted(names),'public_counterexamples':3,'pins':{str(p):sha(p) for p in [mp,cp,upstream,b,exe,Path(__file__),*logs,*[s/p for p in files]]},'findings':[{'location':'fclones/src/path.rs:150','kind':'executed_public_counterexample','detail':'strip_prefix returns Some(current directory) when the source ends before matching longer base: /foo vs /foo/bar, / vs /foo, a vs a/b. Two ordinary controls agree with std. Non-UTF8 Unix roundtrip succeeds. Current tree has no other call site except strip_root with actual root and tests; no CLI grouping impact claimed.'},{'location':'fclones/src/path.rs:40','kind':'source_observation','detail':'Arc parents share path prefixes, but components, hashing and traversal recursively follow them. No total RSS bound or Windows execution proven; Windows OsStr conversion unwrap requires Unicode.'},{'location':'fclones/src/selector.rs:54','kind':'source_observation','detail':'Pattern selection uses lossy strings for paths and names; byte-distinct invalid UTF8 names can map to the same replacement text. Original seven tests only cover text patterns.'},{'location':'fclones/src/transform.rs:293','kind':'source_observation','detail':'Execution drop drains output and waits on child, with no deadline/cancellation in this module; stderr is read into an unbounded String in background. No hanging process or memory stress control executed.'}],'scope':'Three complete files read and public upstream path API exercised on macOS ARM64. Full42tree/full500 review remains incomplete. No upstream implementation adopted, no deletion/transform commands run by this new control, no Windows qualification.'},indent=2)+'\n');print('Verified17 original tests, three public path counterexamples and three-file source review')
if __name__=='__main__':main()
