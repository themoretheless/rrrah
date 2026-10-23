from pathlib import Path
import subprocess,tempfile,json,resource,signal,hashlib
base=Path('.dedup-work/squeez091-target-20261024/debug/deps');libs=list(base.glob('libsqueez-*.rlib'));assert len(libs)==1
binary=Path('.dedup-work/squeez091-session-control-20261024').resolve();v=subprocess.run(['rustc','+1.98.1','--edition=2021','.dedup-work/squeez091-session-control-20261024.rs','-L',f'dependency={base}','--extern',f'squeez={libs[0]}','-o',str(binary)],capture_output=True,text=True);assert v.returncode==0,v.stderr
root=Path(tempfile.mkdtemp(prefix='squeez091-session-state-',dir='.dedup-work')).resolve();keys=root/'keys';keys.mkdir();v=subprocess.run([str(binary),'keys',str(keys)],capture_output=True,text=True,timeout=5);assert v.returncode==0,v.stderr;key_result=json.loads(v.stdout)
state=root/'current';state.mkdir();v=subprocess.run([str(binary),'initial',str(state)],capture_output=True,text=True,timeout=5);assert v.returncode==0,v.stderr;before=(state/'current.json').read_bytes();assert json.loads(before)['session_file']=='previous-valid-state'
def limited():
 signal.signal(signal.SIGXFSZ,signal.SIG_IGN);resource.setrlimit(resource.RLIMIT_FSIZE,(512,512))
v=subprocess.run([str(binary),'replace',str(state)],preexec_fn=limited,capture_output=True,text=True,timeout=5);assert v.returncode==0,v.stderr;after=(state/'current.json').read_bytes()
try:json.loads(after);valid=True
except ValueError:valid=False
assert len(after)==512 and after!=before and not valid
out={'scope':'Original public APIs, owned directory; file-size limit affects only replacement child, not parent','key_control':key_result,'current_save_failure':{'child_exit_code':v.returncode,'previous_bytes':len(before),'published_bytes':len(after),'previous_preserved':before==after,'published_json_valid':valid,'previous_sha256':hashlib.sha256(before).hexdigest(),'published_sha256':hashlib.sha256(after).hexdigest()},'owned_root':str(root)};Path('.dedup-work/squeez091-session-result-20261024.json').write_text(json.dumps(out,indent=2)+'\n');print(json.dumps(out,indent=2))
