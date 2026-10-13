#!/usr/bin/env python3
"""Exhaust every eligible original target; diagnostic, not labelled precision."""
import hashlib,json,os,shutil,struct,subprocess,time,runpy
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def dimensions(p):
 data=Path(p).read_bytes();assert data[:2]==b'\xff\xd8';i=2
 while i<len(data):
  assert data[i]==255;i+=1
  while data[i]==255:i+=1
  marker=data[i];i+=1
  if marker in [1,*range(208,218)]:continue
  n=int.from_bytes(data[i:i+2],'big');assert n>=2
  if marker in [192,193,194,195,197,198,199,201,202,203,205,206,207]:
   h,w=struct.unpack('>HH',data[i+3:i+7]);return [w,h]
  i+=n
 raise ValueError('missing JPEG SOF')
def main():
 out=D/'dedup-anchor-scale-oriented-controls-20261011.json';A=T/'rrrah-anchor-scale-oriented-controls-qualified-20261011';assert not out.exists() and not A.exists();A.mkdir()
 rp=D/'dedup-model-joint-refit-20261011.json';base=json.loads(rp.read_text());assert all(sha(p)==h for p,h in base['pins'].items());old=base['results'][1];original=json.loads(Path(old['command'][1]).read_text());binary=A/'sampling_trace';source=T/'rrrah-anchor-invalid-trace-qualified-20261011/sampling_trace';shutil.copy2(source,binary);audit=D/'dedup-anchor-scale-trace-audit-20261011.json';verified=json.loads(audit.read_text());assert all(sha(p)==h for p,h in verified['pins'].items())
 pins={str(p):sha(p) for p in [rp,Path(old['command'][1]),source,binary,audit,Path(__file__),Path(original['source'])]};plan=[];inventory=[]
 for target in sorted((T/'rrrah-dedup-copydays/original-resolution-negative-200201').glob('*.jpg')):
  raw_dims=dimensions(target);orientation=runpy.run_path(str(R/'scripts/audit-dedup-control-oriented-domains-20261011.py'))['orientation'](target);dims=raw_dims[::-1] if orientation>=5 else raw_dims;entry={'target':str(target),'dimensions':dims,'sha256':sha(target)};inventory.append(entry)
  if dims==raw_dims:entry['role']='excluded_not_rotated';continue
  entry['role']='different_origin_normalized_coordinate_control_not_independently_labelled'
  inp=json.loads(json.dumps(original));inp['target']=str(target);inp['target_dimensions']=dims
  scale=[dims[i]/original['target_dimensions'][i] for i in range(2)]
  for point in inp['points']:
   xy=[struct.unpack('>d',struct.pack('>Q',v))[0] for v in point['target_bits']]
   point['target']=[xy[i]*scale[i] for i in range(2)]
   point['target_bits']=[struct.unpack('>Q',struct.pack('>d',v))[0] for v in point['target']]
  inp['diagnostic_target_scale']=scale
  dest=A/(target.stem+'.json');dest.write_text(json.dumps(inp,indent=2)+'\n');pins[str(target)]=sha(target);pins[str(dest)]=sha(dest)
  for radius in [8,12,16]:plan.append({'target':str(target),'input':str(dest),'filter_radius':radius,'role':entry['role']})
 report={'status':'running','producer_pid':os.getpid(),'pins':pins,'inventory':inventory,'plan':plan,'results':[],'scope':'Ten EXIF-rotated originals, corrected oriented dimensions; additional bounded decode logs verify actual domains on each run. Previous raw-domain controls retained and superseded for these ten files; target coordinates independently scaled by actual width/2560 and height/1920, source coordinates unchanged. Original files not resized. Joint fit repeated with actual full domains, unchanged residual thresholds and1m seed173. This deliberately supplied normalized geometry is a pixel stress control, not real model extraction or corpus recovery. Eight/12/16 smoothing, all windows,90 percent and evidence floor unchanged. Different publisher IDs are candidate controls, not independently annotated negatives; no precision, full search or production adoption claim.'}
 def save():
  temp=out.with_suffix('.tmp');temp.write_text(json.dumps(report,indent=2)+'\n');temp.replace(out)
 save()
 for index,row in enumerate(plan):
  assert sha(row['input'])==pins[row['input']] and sha(row['target'])==pins[row['target']];v={**row,'index':index,'status':'running'};report['results'].append(v);save();start=time.monotonic();cmd=[str(binary),row['input'],'1000000','173',str(row['filter_radius'])];p=subprocess.Popen(cmd,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True);v['pid']=p.pid;save();stdout,stderr=p.communicate();log=A/f'{index:04d}.log';log.write_text(stdout+stderr);v.update(returncode=p.returncode,elapsed_seconds=time.monotonic()-start,stdout=stdout,stderr=stderr,log=str(log),log_sha256=sha(log));save();assert p.returncode==0,stderr;value=json.loads(stdout);assert value['managed_used']==0;v['status']='supported' if value['anchor_result'] and value['anchor_result'].get('supported') else 'not_supported';save();print(index,len(plan),Path(row['target']).name,row['filter_radius'],v['status'],flush=True)
 assert all(sha(p)==h for p,h in pins.items());report['status']='complete_bounded_oriented_scale_controls';save()
if __name__=='__main__':main()
