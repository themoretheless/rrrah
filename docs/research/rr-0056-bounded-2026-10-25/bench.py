"""Sequential bounded real-file CPU/IO qualification, no GPU."""
import hashlib,importlib.util,json,os,re
from pathlib import Path
root=Path.cwd();out=Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location('gate',root/'scripts/qualify-viewer-storage-gpu.py');gate=importlib.util.module_from_spec(spec);spec.loader.exec_module(gate)
rows=[]
for name,selection in [('pairs-1','rr56_qualification::ttl_release_outside_lock_pairs'),('pairs-2','rr56_qualification::ttl_release_outside_lock_pairs'),('scaling','expiry_scaling_qualification::real_file_multi_batch_pairs')]:
 command=['cargo','test','-p','rrrah-cache','--lib',selection,'--','--ignored','--show-output','--test-threads=1']
 with(out/(name+'.log')).open('w')as stream:code,timeout=gate.run_suite(command,stream,os.environ.copy(),240)
 text=(out/(name+'.log')).read_text();binary=re.search(r'Running unittests .*?\(([^)]+)\)',text)
 rows.append(dict(name=name,command=command,exit_code=code,timed_out=timeout,summaries=re.findall(r'test result: (\w+)\. (\d+) passed; (\d+) failed; (\d+) ignored;',text),binary_sha256=hashlib.sha256(Path(binary[1]).read_bytes()).hexdigest()if binary else None))
 (out/'benchmarks.json').write_text(json.dumps(rows,indent=2)+'\n')
 if code or timeout:raise SystemExit(code or 1)
