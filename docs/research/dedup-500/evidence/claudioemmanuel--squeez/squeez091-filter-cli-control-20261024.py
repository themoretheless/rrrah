from pathlib import Path
import os,tempfile,subprocess,json,hashlib
binary=Path('.dedup-work/squeez091-target-20261024/debug/squeez').resolve();root=Path(tempfile.mkdtemp(prefix='squeez091-filter-cli-',dir='.dedup-work')).resolve();source="ERROR: unsuccessful tests 0%\n[assert] x => error\n";cases=[]
for mode in ['filter','wrap']:
 state=root/mode/'state';state.mkdir(parents=True);cwd=root/mode/'cwd';cwd.mkdir();(state/'config.ini').write_text('builtin_filters = false\nnet_win_min_tokens = 0\n')
 env=os.environ.copy();env['SQUEEZ_DIR']=str(state)
 cmd=[str(binary),'filter','owned-unknown-command'] if mode=='filter' else [str(binary),'wrap',"printf '%s\\n' 'ERROR: unsuccessful tests 0%' '[assert] x => error'"]
 result=subprocess.run(cmd,input=source if mode=='filter' else None,text=True,capture_output=True,env=env,cwd=cwd,timeout=10)
 blobs=state/'blobs';data=[p for p in blobs.glob('*') if len(p.name)==16 and p.is_file()] if blobs.exists() else [];recoverable=any(p.read_text()==source or p.read_text()==source.rstrip('\n') for p in data)
 cases.append({'mode':mode,'command':cmd,'exit_code':result.returncode,'stdout':result.stdout,'stderr':result.stderr,'contains_first_diagnostic':'ERROR: unsuccessful tests 0%' in result.stdout,'contains_second_diagnostic':'[assert] x => error' in result.stdout,'original_recoverable_in_blob_store':recoverable,'blob_files':len(data)})
assert cases[0]['exit_code']==0 and cases[0]['stdout']=='' and not cases[0]['original_recoverable_in_blob_store']
a={'scope':'Unmodified actual binary, fixed synthetic diagnostics; filter stdin versus wrap executing only printf in isolated cwd/state; not all wrapper branches.','binary_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),'owned_root':str(root),'cases':cases};Path('.dedup-work/squeez091-filter-cli-result-20261024.json').write_text(json.dumps(a,indent=2)+'\n');print(json.dumps(a,indent=2))
