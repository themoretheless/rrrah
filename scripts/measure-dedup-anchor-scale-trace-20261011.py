#!/usr/bin/env python3
import hashlib,json,os,shutil,subprocess,time
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 out=D/'dedup-anchor-scale-trace-20261011.json';A=T/'rrrah-anchor-scale-trace-qualified-20261011';assert not out.exists() and not A.exists();A.mkdir();mp=D/'dedup-anchor-scale-trace-harness-20261011.json';m=json.loads(mp.read_text());assert all(sha(Path(m['harness'])/f)==h for f,h in m['files'].items());parent=json.loads(Path(m['parent_manifest']).read_text());assert sha(m['parent_manifest'])==m['parent_sha256'];assert all(sha(Path(parent['snapshot'])/f)==h for f,h in parent['file_hashes'].items())
 build=T/'dedup-anchor-scale-trace-build-20261011.jsonl';entries=[json.loads(x) for x in build.read_text().splitlines()];assert entries[-1]['reason']=='build-finished' and entries[-1]['success'];bins=[e['executable'] for e in entries if e.get('reason')=='compiler-artifact' and e.get('executable')];assert len(bins)==1;binary=A/'sampling_trace';shutil.copy2(bins[0],binary)
 rp=D/'dedup-model-joint-refit-20261011.json';base=json.loads(rp.read_text());assert all(sha(p)==h for p,h in base['pins'].items());old=base['results'][1];pins=dict(base['pins']);pins.update({str(p):sha(p) for p in [mp,build,T/'dedup-anchor-scale-trace-build-20261011.log',binary,rp,Path(__file__)]});report={'status':'running','producer_pid':os.getpid(),'pins':pins,'radii':[0,2,4,8,12,16,24,32],'results':[],'scope':'Single positive blur-scale diagnostic; same full point set and1m seed173 geometry, all windows retained, agreement90 percent, contrast and evidence floors unchanged. Radius8 must reproduce old output; no negative precision, automatic selection or production adoption claim.'}
 def save():out.write_text(json.dumps(report,indent=2)+'\n')
 save()
 for radius in report['radii']:
  assert all(sha(p)==h for p,h in pins.items());row={'filter_radius':radius,'status':'running'};report['results'].append(row);save();start=time.monotonic();cmd=[str(binary),old['command'][1],'1000000','173',str(radius)];p=subprocess.Popen(cmd,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True);row['pid']=p.pid;save();stdout,stderr=p.communicate();log=A/f'{radius}.log';log.write_text(stdout+stderr);row.update(returncode=p.returncode,elapsed_seconds=time.monotonic()-start,stdout=stdout,stderr=stderr,log=str(log),log_sha256=sha(log));assert p.returncode==0,stderr;v=json.loads(stdout);assert v.pop('filter_radius')==radius;previous=json.loads(old['stdout']);assert v['best']==previous['best'];assert v['managed_used']==0
  if radius==8:assert v==previous
  row['status']='supported' if v['anchor_result'].get('supported') else 'not_supported';save();print(radius,row['status'],flush=True)
 assert all(sha(p)==h for p,h in pins.items());report['status']='complete_bounded_anchor_scale_trace';save()
if __name__=='__main__':main()
