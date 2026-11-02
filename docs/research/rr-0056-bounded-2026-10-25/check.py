import importlib.util,os,json,re
from pathlib import Path
root=Path.cwd();out=Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location('gate',root/'scripts/qualify-viewer-storage-gpu.py');gate=importlib.util.module_from_spec(spec);spec.loader.exec_module(gate)
rows=[]
for name,cmd in [('memory',['cargo','test','-p','rrrah-memory','--lib']),('cache',['cargo','test','-p','rrrah-cache','--lib'])]:
 cmd+=['--','--show-output','--test-threads=1']
 with(out/(name+'.log')).open('w')as stream:code,timeout=gate.run_suite(cmd,stream,os.environ.copy(),300)
 rows.append(dict(name=name,command=cmd,exit_code=code,timed_out=timeout,summaries=re.findall(r'test result: (\w+)\. (\d+) passed; (\d+) failed; (\d+) ignored;',(out/(name+'.log')).read_text())))
 (out/'checks.json').write_text(json.dumps(rows,indent=2)+'\n')
 if code or timeout:raise SystemExit(code or 1)
