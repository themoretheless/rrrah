from pathlib import Path
import subprocess,tempfile,json,time,hashlib
base=Path('.dedup-work/squeez091-target-20261024/debug/deps');libs=list(base.glob('libsqueez-*.rlib'));assert len(libs)==1
binary=Path('.dedup-work/squeez091-session-control-20261024').resolve();v=subprocess.run(['rustc','+1.98.1','--edition=2021','.dedup-work/squeez091-session-control-20261024.rs','-L',f'dependency={base}','--extern',f'squeez={libs[0]}','-o',str(binary)],capture_output=True,text=True);assert v.returncode==0,v.stderr
root=Path(tempfile.mkdtemp(prefix='squeez091-session-concurrency-',dir='.dedup-work')).resolve();v=subprocess.run([str(binary),'initial',str(root)],capture_output=True,text=True,timeout=30);assert v.returncode==0
children=[subprocess.Popen([str(binary),f'worker{i}',str(root)],stdout=subprocess.PIPE,stderr=subprocess.PIPE) for i in range(1,5)];valid=invalid=missing=0;first=None;deadline=time.monotonic()+30
while True:
 try:
  raw=(root/'current.json').read_bytes()
  try:json.loads(raw);valid+=1
  except (ValueError,UnicodeDecodeError) as e:
   invalid+=1
   if first is None:first={'bytes':len(raw),'sha256':hashlib.sha256(raw).hexdigest(),'parse_error':str(e)}
 except FileNotFoundError:missing+=1
 if all(c.poll() is not None for c in children):break
 if time.monotonic()>deadline:
  for c in children:c.kill()
  raise RuntimeError('owned writers exceeded deadline')
codes=[c.wait() for c in children];assert codes==[0]*4
out={'scope':'Unmodified CurrentSession.save; four owned processes,100 saves each; independent Python JSON reader, one finite schedule only','worker_exit_codes':codes,'valid_reads':valid,'invalid_reads':invalid,'missing_reads':missing,'first_invalid':first,'owned_directory':str(root)};Path('.dedup-work/squeez091-session-concurrency-result-20261024.json').write_text(json.dumps(out,indent=2)+'\n');print(json.dumps(out,indent=2))
