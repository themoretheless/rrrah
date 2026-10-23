from pathlib import Path
import os,tempfile,subprocess,json,hashlib,shlex
binary=Path('.dedup-work/squeez091-target-20261024/debug/squeez').resolve();root=Path(tempfile.mkdtemp(prefix='squeez091-builtin-cli-',dir='.dedup-work')).resolve();cases=[]
fixtures=[('uv sync','error: No solution found when resolving dependencies:\n'),('mypy missing.py',"mypy: can't read file 'missing.py': No such file or directory\n"),('rsync src dst','rsync error: connection unexpectedly closed\n')]
for i,(hint,source) in enumerate(fixtures):
 state=root/str(i)/'state';state.mkdir(parents=True);cwd=root/str(i)/'cwd';cwd.mkdir();(state/'config.ini').write_text('builtin_filters = true\nnet_win_min_tokens = 0\n');env=os.environ.copy();env['SQUEEZ_DIR']=str(state)
 result=subprocess.run([str(binary),'filter',hint],input=source,text=True,capture_output=True,env=env,cwd=cwd,timeout=10)
 cases.append({'hint':hint,'synthetic_input':source,'exit_code':result.returncode,'stdout':result.stdout,'stderr':result.stderr})
assert [c['stdout'].strip() for c in cases]==['uv sync: environment already up to date','mypy: no issues found','rsync: transfer complete']
# Actual wrapper, but an owned stand-in producer; no installed uv is invoked.
wrappers=[]
for label,force,noise_lines in [('forced_small',True,0),('default_small',False,0),('default_below_stash_threshold',False,15),('default_verbose',False,200)]:
 state=root/label/'state';state.mkdir(parents=True);cwd=root/label/'cwd';cwd.mkdir();owned_bin=root/label/'bin';owned_bin.mkdir()
 payload=(''.join(f'resolver trace {i}\n' for i in range(noise_lines)) if noise_lines else '')+fixtures[0][1]
 producer=owned_bin/'uv';producer.write_text("#!/bin/sh\n: > producer-invoked\nprintf '%s\\n' "+shlex.quote(payload.rstrip('\n'))+"\nexit 7\n");producer.chmod(0o700)
 (state/'config.ini').write_text('builtin_filters = true\n'+('net_win_min_tokens = 0\n' if force else ''))
 env=os.environ.copy();env['SQUEEZ_DIR']=str(state);env['PATH']=str(owned_bin)+os.pathsep+env.get('PATH','')
 result=subprocess.run([str(binary),'wrap','uv sync'],text=True,capture_output=True,env=env,cwd=cwd,timeout=10)
 assert result.returncode==7 and (cwd/'producer-invoked').is_file()
 wrappers.append({'case':label,'producer':'Owned stand-in uv writes invocation marker, prints synthetic fixture and exits7; real uv not invoked.','net_win_min_tokens_overridden':force,'input_lines':len(payload.splitlines()),'exit_code':result.returncode,'stdout':result.stdout,'stderr':result.stderr,'contains_failure_diagnostic':'No solution found' in result.stdout,'contains_success_claim':'environment already up to date' in result.stdout,'owned_producer_marker_verified':True,'blob_files':len([p for p in (state/'blobs').glob('*') if len(p.name)==16 and p.is_file()]),'failure_text_recoverable_in_blob':any('No solution found' in p.read_text() for p in (state/'blobs').glob('*') if len(p.name)==16 and p.is_file())})
out={'scope':'Unmodified actual squeez binary with pinned built-in rule pack, owned cwd/state. Standalone filter uses synthetic fixtures; wrapper uses owned failing stand-in producer. No real package tool, network or installation invoked.','binary_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),'standalone_filter':cases,'wrapper_failure_controls':wrappers,'owned_root':str(root)};Path('.dedup-work/squeez091-builtin-cli-result-20261024.json').write_text(json.dumps(out,indent=2)+'\n');print(json.dumps(out,indent=2))
