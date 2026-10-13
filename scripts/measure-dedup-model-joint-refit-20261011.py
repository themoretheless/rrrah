#!/usr/bin/env python3
import hashlib,json,os,re,shutil,struct,subprocess,time
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 out=D/'dedup-model-joint-refit-20261011.json';A=T/'rrrah-model-joint-refit-qualified-20261011';assert not out.exists() and not A.exists();A.mkdir();hp=D/'dedup-model-points-trace-harvest-20261011.json';h=json.loads(hp.read_text());assert all(sha(p)==v for p,v in h['pins'].items());log=T/'rrrah-model-points-trace-qualified-20261011/201300.log';matches=re.findall(r'ModelTileAttempt \{ left_rectangle: (\[[^]]+\]), right_rectangle: (\[[^]]+\]), evidence: (InsufficientAnchorGeometry|Comparison)',log.read_text());assert len(matches)==25;source=T/'rrrah-joint-geometry-sampling-patched-qualified-20261011/sampling_trace';binary=A/'sampling_trace';shutil.copy2(source,binary);ga=D/'dedup-joint-geometry-sampling-audit-20261011.json';g=json.loads(ga.read_text());assert all(sha(p)==v for p,v in g['pins'].items());pins={str(p):sha(p) for p in [hp,log,ga,source,binary,Path(__file__)]};plan=[]
 for index,((left,right,kind),trace) in enumerate(zip(matches,h['trace'])):
  assert json.loads(left)==trace['left_rectangle'] and json.loads(right)==trace['right_rectangle']
  if kind!='InsufficientAnchorGeometry':continue
  pts=[]
  for bits in trace['coordinate_bits']:
   values=[struct.unpack('>d',struct.pack('>Q',b))[0] for b in bits];pts.append({'source':values[:2],'target':values[2:],'source_bits':bits[:2],'target_bits':bits[2:]})
  inp={'source':h['plan']['left'],'target':h['plan']['right'],'source_tolerance':h['plan']['source_tolerance'],'source_dimensions':trace['source_dimensions'],'target_dimensions':trace['target_dimensions'],'points':pts,'trace_index':index};p=A/f'201300-{index}.json';p.write_text(json.dumps(inp,indent=2)+'\n');pins[str(p)]=sha(p)
  for trials in [20000,1000000]:plan.append({'trace_index':index,'input':str(p),'trials':trials,'seed':173})
 for key in ['left','right']:pins[h['plan'][key]]=sha(h['plan'][key])
 report={'status':'running','producer_pid':os.getpid(),'pins':pins,'plan':plan,'results':[],'scope':'Joint refit of three actual mapped/selected model point banks that previously yielded insufficient anchor geometry. Same calibrated public fitter/decoder and original source/pixel thresholds;20k and1m explicitly admitted trials. No new inference or model API adoption.'}
 def save():out.write_text(json.dumps(report,indent=2)+'\n')
 save()
 for row in plan:
  assert all(sha(p)==v for p,v in pins.items());cmd=[str(binary),row['input'],str(row['trials']),str(row['seed'])];v={'trace_index':row['trace_index'],'trials':row['trials'],'status':'running','command':cmd};report['results'].append(v);save();start=time.monotonic();p=subprocess.Popen(cmd,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True);v['pid']=p.pid;save();stdout,stderr=p.communicate();log=A/f"{row['trace_index']}-{row['trials']}.log";log.write_text(stdout+stderr);v.update(returncode=p.returncode,stdout=stdout,stderr=stderr,elapsed_seconds=time.monotonic()-start,log=str(log),log_sha256=sha(log));assert p.returncode==0,stderr;value=json.loads(stdout);assert value['managed_used']==0 and value['trials']==row['trials'];v['status']='supported' if value['anchor_result'] and value['anchor_result'].get('supported') is True else 'inconclusive';save();print(row['trace_index'],row['trials'],v['status'],flush=True)
 assert all(sha(p)==v for p,v in pins.items());report['status']='complete_bounded_model_joint_refit';save()
if __name__=='__main__':main()
