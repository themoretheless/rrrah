#!/usr/bin/env python3
import hashlib,json,os,re,shutil,subprocess,time
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 out=D/'dedup-disc21-pilot-native-20261011.json';A=T/'rrrah-disc21-pilot-native-qualified-20261011';assert not out.exists() and not A.exists();A.mkdir()
 prepared=D/'dedup-disc21-pilot-prepared-20261011.json';data=json.loads(prepared.read_text());assert all(sha(k)==v for k,v in data['pins'].items());images={x['id']:x for x in data['images']}
 mp=D/'dedup-native-bank-outcome-native-snapshot-20261011.json';m=json.loads(mp.read_text());assert all(sha(Path(m['snapshot'])/k)==v for k,v in m['file_hashes'].items())
 ca=D/'dedup-native-bank-outcome-native-compile-audit-20261011.json';c=json.loads(ca.read_text());assert all(sha(k)==v for k,v in c['pins'].items())
 full=json.loads((D/'dedup-native-bank-all-positives-20261011.json').read_text());src=Path(next(k for k in full['pins'] if k.endswith('/native_bank_outcome_files') and 'all-positives-qualified' in k));assert sha(src)==full['pins'][str(src)];binary=A/src.name;shutil.copy2(src,binary);weights=T/'rrrah-dedup-copydays/learned-matching-models/loftr-outdoor-native-20261010.safetensors';runtime=D/'dedup-loftr-native-20261010-runtime-pins.json';rt=json.loads(runtime.read_text());libs={k:v for k,v in rt['pins'].items() if k.endswith('.dylib')};assert all(sha(k)==v for k,v in libs.items())
 pins=dict(data['pins']);pins.update({str(p):sha(p) for p in [prepared,mp,ca,binary,src,weights,runtime,Path(__file__)]});pins.update(libs)
 r={'status':'running','producer_pid':os.getpid(),'pins':pins,'plan':data['plan'],'required':2,'results':[],'policy':'Fixed frozen native/opposite/bank two-file collection;4096 native;1m opposite;25banks;2/6radii;maxside640;source tolerance2.0 for BOTH cases, set before observing results. Probe assumes untagged linear sRGB.','scope':'DISC21 deterministic pilot: published copy Q00061/R020965 and distractor Q00000/R020965. Published annotations may err; image inspection sees building collage versus source, and unrelated decorated-card query. No general semantic/burst precision, full DISC21 or model recall qualification.'}
 def save():
  tmp=out.with_suffix('.tmp');tmp.write_text(json.dumps(r,indent=2)+'\n');tmp.replace(out)
 save()
 for index,case in enumerate(data['plan']):
  left=images[case['query']]['path'];right=images[case['reference']]['path'];cmd=['/usr/bin/time','-l','/usr/bin/env','DYLD_LIBRARY_PATH='+str(T/'rrrah-loftr-native-runtime-20261010/lib/python3.12/site-packages/torch/lib'),str(binary),left,right,str(weights),'640','2.0','1000000'];v={'index':index,'query':case['query'],'reference':case['reference'],'published_label':case['published_label'],'status':'running','command':cmd};r['results'].append(v);save();start=time.monotonic();p=subprocess.Popen(cmd,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True);v['pid']=p.pid;save();stdout,stderr=p.communicate();log=A/f'{index:02d}.log';log.write_text(stdout+stderr);v.update(returncode=p.returncode,stdout=stdout,stderr=stderr,elapsed_seconds=time.monotonic()-start,log=str(log),log_sha256=sha(log));save()
  if p.returncode:v['status']='execution_failed'
  else:
   value=json.loads(stdout);assert value['files']==[2,9] and value['managed_used']==0 and len(value['pairs'])==1;pair=value['pairs'][0];assert (pair['left'],pair['right'])==(2,9);support=pair['support'];assert support is None or type(support) is bool;v.update(status='supported' if support is True else ('rejected' if support is False else 'inconclusive'),evidence=pair,managed_peak=value['managed_peak'],max_rss_bytes=int(re.search(r'(\d+)\s+maximum resident set size',stderr)[1]))
  assert all(sha(k)==v for k,v in pins.items());save();print(index,v['published_label'],v['status'],flush=True)
 r['status']='complete_bounded_disc21_pilot_execution';save()
if __name__=='__main__':main()
