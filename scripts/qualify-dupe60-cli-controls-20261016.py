"""Exercise unchanged complete upstream CLI on owned synthetic files and Git repositories."""
from pathlib import Path
import hashlib,json,os,re,subprocess
R=Path(__file__).resolve().parents[1];W=R/'.dedup-work/dupe60-20261016';S=W/'source';D=R/'docs/research/dedup-500';BIN=W/'cargo-target/debug/dupehound';h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
original=re.search(r'const ORIGINAL: &str = r#"(.*?)"#;', (S/'tests/check.rs').read_text(),re.S)[1]
runs=[];raw=[]
for run in [1,2]:
 root=W/f'cli-controls-run{run}';root.mkdir();tmp=root/'tmp';tmp.mkdir();env=dict(os.environ,TMPDIR=str(tmp),GIT_CEILING_DIRECTORIES=str(W),NO_COLOR='1');observed=[]
 def invoke(key,args,cwd=root):
  p=subprocess.run([str(BIN),*args],cwd=cwd,env=env,capture_output=True,text=True,timeout=45)
  for suffix,body in [('stdout',p.stdout),('stderr',p.stderr)]:
   path=root/f'{key}.{suffix}.log';path.write_text(body);raw.append(path)
  return p
 def scan(key,folder,*args):
  p=invoke(key,['scan','--json','--min-tokens','10',*args,str(folder)]);assert p.returncode==0,p.stderr;return json.loads(p.stdout)
 for n in [100,101]:
  folder=root/f'duplicates{n}';folder.mkdir()
  for i in range(n):(folder/f'f{i:03}.ts').write_text(original.replace('formatPrice',f'function{i}'))
  out=scan(f'duplicates{n}',folder);assert out['stats']['functions']==n
  assert len(out['clusters'])==(1 if n==100 else 0)
  if n==100:assert out['clusters'][0]['copies']==100
  observed.append(dict(control=f'whole_cli_{n}_identical_functions',functions=n,clusters=len(out['clusters']),score=out['score']['slop_percent']))
 folder=root/'duplicates100';out=scan('nan-threshold',folder,'--threshold','NaN');assert not out['clusters'];observed.append(dict(control='whole_cli_nan_threshold',functions=out['stats']['functions'],clusters=0))
 rustbody='\n'.join(f'let v{i}=x.wrapping_add({i}).wrapping_mul({i+2});' for i in range(10))+'\nv0+v1+v2+v3+v4+v5+v6+v7+v8+v9\n'
 for marker in ['', '// #[cfg(test)] mod fake {}\n']:
  key='rust-comment' if marker else 'rust-clean';folder=root/key;folder.mkdir();(folder/'a.rs').write_text(marker+'fn a(x:u32)->u32{'+rustbody+'}\nfn b(x:u32)->u32{'+rustbody+'}\n');out=scan(key,folder);assert len(out['clusters'])==1;c=out['clusters'][0];assert c['test_only']==bool(marker) and all(x['test']==bool(marker) for x in c['members']);observed.append(dict(control=key,test_only=c['test_only'],score=out['score']['slop_percent']))
 repo=root/'history-repo';repo.mkdir()
 def git(args,date='2024-01-15T12:00:00Z'):
  e=dict(env,GIT_AUTHOR_DATE=date,GIT_COMMITTER_DATE=date);p=subprocess.run(['git','-c','core.fsmonitor=false','-C',str(repo),*args],env=e,capture_output=True,text=True);assert p.returncode==0,p.stderr
 git(['init','-q','-b','main']);git(['config','user.email','fixture@example.invalid']);git(['config','user.name','Owned fixture']);(repo/'a.ts').write_text(original);git(['add','a.ts']);git(['commit','-q','-m','typescript']);git(['mv','a.ts','a.py']);git(['commit','-q','-m','same bytes new language'],'2024-02-15T12:00:00Z')
 p=invoke('history-rename',['history','--json','--no-card','--min-tokens','10',str(repo)]);assert p.returncode==0,p.stderr;series=json.loads(p.stdout)['series'];fresh=scan('fresh-python',repo);assert len(series)==2 and [x['functions'] for x in series]==[1,1] and fresh['stats']['functions']==0;observed.append(dict(control='history_cache_missing_language_key',history_functions=[1,1],fresh_latest_functions=0))
 p=invoke('history-one-snapshot',['history','--json','--no-card','--max-snapshots','1',str(repo)]);assert p.returncode!=0 and 'divide by zero' in p.stderr;observed.append(dict(control='history_max_snapshots_one_panics',returncode=p.returncode))
 p=invoke('history-exclude',['history','--json','--no-card','--min-tokens','10','--exclude','*.ts',str(repo)]);assert p.returncode==0,p.stderr;assert json.loads(p.stdout)['series'][0]['functions']==1;observed.append(dict(control='history_custom_exclude_ignored',first_snapshot_functions=1))
 runs.append(observed)
assert runs[0]==runs[1]
paths=[BIN,W/'cargo-build.log',W/'cargo-upstream-run1.log',W/'cargo-upstream-run2.log',D/'dupe60-fetched-source-pins-20261016.json']+raw
out=D/'dupe60-whole-cli-controls-20261016.json';assert not out.exists();out.write_text(json.dumps(dict(status='verified_whole_cli_controls_with_partial_repository_review',whole_repo_complete=False,repository='Rafaelpta/dupehound',commit='35ea7c3743a8fb2205aaef973e67b1c6c5e41474',controls=runs[0],control_runs=2,producer_sha256=h(__file__),evidence_pins={str(p.relative_to(R)):h(p) for p in paths},scope='Actual complete default-card-feature Cargo binary withlocked original dependencies/source; only buildcopyCargo.toml appendedworkspaceboundary. Ownedfiles/repos only; no productionedits/deletes/networkservices. Alloriginaltests runseparatelytwice.',limits=['No fullrepositoryflag: benchmark/docs/remainingmedia notyetreviewed.','No performance/speedup/quality/platform/currentRoot/adoption claim.','Historyprojection counts compared tofreshsamebinaryscan; doesnotclaimgeneralall-languagecachefailure.']),indent=2)+'\n');print(json.dumps({'native_whole_cli_controls':len(runs[0]),'runs':2,'whole_repo_complete':False}))
