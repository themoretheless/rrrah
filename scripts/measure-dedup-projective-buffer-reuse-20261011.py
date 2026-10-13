#!/usr/bin/env python3
import hashlib,json,os,shutil,subprocess,time
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 out=D/'dedup-projective-buffer-reuse-parity-20261011.json';A=T/'rrrah-projective-buffer-reuse-parity-qualified-20261011';assert not out.exists() and not A.exists();A.mkdir();mp=D/'dedup-projective-buffer-reuse-harness-20261011.json';m=json.loads(mp.read_text());assert all(sha(Path(m['harness'])/f)==h for f,h in m['files'].items());parent=json.loads(Path(m['parent_manifest']).read_text());assert all(sha(Path(parent['snapshot'])/f)==h for f,h in parent['file_hashes'].items());build=T/'dedup-projective-buffer-reuse-build-20261011.jsonl';entries=[json.loads(x) for x in build.read_text().splitlines()];assert entries[-1]['reason']=='build-finished' and entries[-1]['success'];exe=next(e['executable'] for e in entries if e.get('executable'));binary=A/'sampling_trace';shutil.copy2(exe,binary);pins={str(p):sha(p) for p in [mp,build,T/'dedup-projective-buffer-reuse-build-20261011.log',binary,Path(__file__)]};plan=[]
 for rp,indices in [(D/'dedup-joint-geometry-sampling-patched-20261011.json',[0,3,4]),(D/'dedup-model-joint-refit-20261011.json',[1])]:
  r=json.loads(rp.read_text());assert all(sha(p)==h for p,h in r['pins'].items());pins[str(rp)]=sha(rp)
  for index in indices:
   old=r['results'][index];cmd=old['command'];inp=json.loads(Path(cmd[1]).read_text());pins[cmd[1]]=sha(cmd[1]);pins[inp['source']]=sha(inp['source']);pins[inp['target']]=sha(inp['target']);plan.append({'input':cmd[1],'trials':cmd[2],'seed':cmd[3],'reference_report':str(rp),'reference_index':index,'expected':json.loads(old['stdout'])})
 report={'status':'running','producer_pid':os.getpid(),'pins':pins,'plan':plan,'results':[],'scope':'Four exact-bit real point controls, identical whole JSON/H/inliers/error/original pixel outcomes required vs old allocating fitter. No native model inference or universal speed/RSS claim.'}
 def save():out.write_text(json.dumps(report,indent=2)+'\n')
 save()
 for index,row in enumerate(plan):
  v={'index':index,'status':'running'};report['results'].append(v);save();cmd=[str(binary),row['input'],row['trials'],row['seed']];start=time.monotonic();p=subprocess.Popen(cmd,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True);v['pid']=p.pid;save();stdout,stderr=p.communicate();log=A/f'{index}.log';log.write_text(stdout+stderr);v.update(returncode=p.returncode,stdout=stdout,stderr=stderr,elapsed_seconds=time.monotonic()-start,log=str(log),log_sha256=sha(log));save();assert p.returncode==0,stderr;assert json.loads(stdout)==row['expected'];v['status']='exact_whole_json_parity';save();print(index,v['status'],flush=True)
 assert all(sha(p)==h for p,h in pins.items());report['status']='complete_four_real_point_buffer_parity';save()
if __name__=='__main__':main()
