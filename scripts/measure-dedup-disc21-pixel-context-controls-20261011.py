#!/usr/bin/env python3
import hashlib,json,os,shutil,subprocess,time
from pathlib import Path
D=Path(__file__).resolve().parents[1]/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 parent=D/'dedup-disc21-pixel-context-native-20261011.json';r=json.loads(parent.read_text());assert r['status']=='complete' and all(sha(k)==v for k,v in r['pins'].items());audit=D/'dedup-disc21-pixel-context-native-audit-20261011.json';a=json.loads(audit.read_text());assert all(sha(k)==v for k,v in a['pins'].items())
 prep=D/'dedup-disc21-cohort-prepared-20261011.json';data=json.loads(prep.read_text());assert all(sha(k)==v for k,v in data['pins'].items());images={x['id']:x['path'] for x in data['images']};plan=[data['plan'][8],data['plan'][6]]
 A=T/'rrrah-disc21-pixel-context-controls-qualified-20261011';out=D/'dedup-disc21-pixel-context-controls-20261011.json';assert not A.exists() and not out.exists();A.mkdir();binary=A/'disc21_pixel_context_native_files';shutil.copy2(r['command'][4],binary);pins=dict(r['pins']);pins.update(data['pins']);pins.update({str(x):sha(x) for x in [parent,audit,prep,binary,Path(__file__)]});result={'status':'running','producer_pid':os.getpid(),'pins':pins,'plan':plan,'required':2,'results':[],'scope':'Same fixed four-context CPU API: published unrelated distractor first, then independently observed second baseline positive gap. No policy adjustment between cases, no general precision qualification.'}
 def save():
  tmp=out.with_suffix('.tmp');tmp.write_text(json.dumps(result,indent=2)+'\n');tmp.replace(out)
 save()
 for i,case in enumerate(plan):
  cmd=r['command'].copy();cmd[4]=str(binary);cmd[5]=images[case['query']];cmd[6]=images[case['reference']];row=dict(case,index=i,command=cmd,status='running');result['results'].append(row);save();start=time.monotonic();p=subprocess.Popen(cmd,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True);row['pid']=p.pid;save();stdout,stderr=p.communicate();log=A/f'{i}.log';log.write_text(stdout+stderr);row.update(returncode=p.returncode,stdout=stdout,stderr=stderr,log=str(log),log_sha256=sha(log),elapsed_seconds=time.monotonic()-start)
  if p.returncode:row['status']='execution_failed'
  else:
   e=json.loads(stdout);assert e['attempts']==25 and e['managed_used']==0 and type(e['supported']) is bool;row.update(status='supported' if e['supported'] else 'inconclusive',evidence=e)
  assert all(sha(k)==v for k,v in pins.items());save();print(i,row['status'],flush=True)
 result['status']='complete_bounded_four_context_controls';save()
if __name__=='__main__':main()
