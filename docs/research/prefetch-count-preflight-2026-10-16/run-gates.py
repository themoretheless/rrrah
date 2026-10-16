import os,json,subprocess
from pathlib import Path
out=Path(__file__).resolve().parent
root=out.parents[2]
env=dict(os.environ,CARGO_TARGET_DIR='/tmp/rrrah-alpha-isolated-target-20261016')
r={'status':'running','stages':[]}
for name,cmd in [('memory-cache',['cargo','+stable','test','-p','rrrah-memory','-p','rrrah-cache','--lib','--locked','--offline']),('viewer',['cargo','+stable','test','-p','rrrah','--bin','rrrah','--locked','--offline']),('build',['cargo','+stable','build','-p','rrrah','--bin','rrrah','--locked','--offline'])]:
 r['current']=name;(out/'gates.json').write_text(json.dumps(r,indent=2)+'\n')
 with (out/(name+'.log')).open('w') as log:code=subprocess.run(cmd,cwd=root,env=env,stdout=log,stderr=subprocess.STDOUT).returncode
 r['stages'].append({'stage':name,'command':cmd,'exit_code':code});print(name,code,flush=True)
 if code:r['status']='failed';break
else:r['status']='passed'
r.pop('current',None);(out/'gates.json').write_text(json.dumps(r,indent=2)+'\n')
