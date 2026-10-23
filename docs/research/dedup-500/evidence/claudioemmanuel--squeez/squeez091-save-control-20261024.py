from pathlib import Path
import hashlib,json,resource,signal,subprocess,tempfile
base=Path('.dedup-work/squeez091-target-20261024/debug/deps');libs=list(base.glob('libsqueez-*.rlib'));assert len(libs)==1
binary=Path('.dedup-work/squeez091-save-control-20261024').resolve()
v=subprocess.run(['rustc','+1.98.1','--edition=2021','.dedup-work/squeez091-save-control-20261024.rs','-L',f'dependency={base}','--extern',f'squeez={libs[0]}','-o',str(binary)],capture_output=True,text=True);assert v.returncode==0,v.stderr
dir=Path(tempfile.mkdtemp(prefix='squeez091-save-state-',dir='.dedup-work')).resolve();path=dir/'context.json'
v=subprocess.run([str(binary),'initial',str(dir)],capture_output=True,text=True);assert v.returncode==0,v.stderr
before=path.read_bytes();assert json.loads(before)['session_file']=='previous-valid-state'
def limit():
 signal.signal(signal.SIGXFSZ,signal.SIG_IGN)
 resource.setrlimit(resource.RLIMIT_FSIZE,(512,512))
v=subprocess.run([str(binary),'replace',str(dir)],preexec_fn=limit,capture_output=True,text=True,timeout=10)
after=path.read_bytes()
try:json.loads(after);valid=True
except (ValueError,UnicodeDecodeError):valid=False
result={'scope':'Unmodified pinned SessionContext.save public API in a subprocess; per-child file size limit512 bytes, SIGXFSZ ignored; owned directory only','child_exit_code':v.returncode,'child_stderr':v.stderr,'previous_bytes':len(before),'published_bytes':len(after),'previous_sha256':hashlib.sha256(before).hexdigest(),'published_sha256':hashlib.sha256(after).hexdigest(),'previous_preserved':before==after,'published_json_valid':valid,'owned_directory':str(dir)}
assert v.returncode==0 and len(after)==512 and before!=after and not valid
Path('.dedup-work/squeez091-save-result-20261024.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result,indent=2))
