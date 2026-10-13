#!/usr/bin/env python3
import hashlib,json,shutil,struct,subprocess,time,os
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 a=T/'rrrah-joint-consensus-200301-qualified-20261011';out=D/'dedup-joint-consensus-200301-20261011.json';assert not a.exists() and not out.exists();a.mkdir();audit=D/'dedup-model-points-200301-trace-audit-20261011.json';v=json.loads(audit.read_text());assert all(sha(p)==h for p,h in v['pins'].items());trace=D/'dedup-model-points-200301-trace-20261011.json';r=json.loads(trace.read_text());src=T/'rrrah-opposite-native-snapshot-20261011/target/debug/joint_consensus_diagnostic';binary=a/src.name;shutil.copy2(src,binary);h=T/'rrrah-joint-consensus-diagnostic-harness-20261011';snapshot=D/'dedup-projective-buffer-reuse-snapshot-20261011.json';m=json.loads(snapshot.read_text());assert all(sha(Path(m['snapshot'])/p)==v for p,v in m['file_hashes'].items());pins={str(p):sha(p) for p in [audit,trace,src,binary,h/'Cargo.toml',h/'Cargo.lock',h/'src/bin/joint_consensus_diagnostic.rs',snapshot,Path(__file__)]};plan=[]
 for index,t in enumerate(r['trace']):
  points=[]
  for bits in t['coordinate_bits']:
   vals=[struct.unpack('>d',struct.pack('>Q',b))[0] for b in bits];points.append({'source':vals[:2],'target':vals[2:],'source_bits':bits[:2],'target_bits':bits[2:]})
  p=a/f'window{index}.json';p.write_text(json.dumps({'points':points,'source_dimensions':t['source_dimensions'],'target_dimensions':t['target_dimensions'],'source_tolerance':r['plan']['source_tolerance']})+'\n');pins[str(p)]=sha(p);plan.append(str(p))
 report={'status':'running','producer_pid':os.getpid(),'pins':pins,'plan':plan,'results':[],'scope':'25 exact captured point banks, diagnostic min4 at1m/seed173; original bidirectional residuals retained. No pixel/duplicate acceptance; sampled best is not an exhaustive maximum.'}
 def save():out.write_text(json.dumps(report,indent=2)+'\n')
 save()
 for index,p in enumerate(plan):
  start=time.monotonic();cmd=[str(binary),p,'1000000','173'];child=subprocess.Popen(cmd,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True);row={'window_index':index,'pid':child.pid,'command':cmd,'status':'running'};report['results'].append(row);save();stdout,stderr=child.communicate();row.update(returncode=child.returncode,stdout=stdout,stderr=stderr,elapsed_seconds=time.monotonic()-start);assert child.returncode==0,stderr;value=json.loads(stdout);row['joint_inliers']=len(value['best']['joint_inliers']) if value['best'] else 0;row['status']='complete';save();print(index,row['joint_inliers'],flush=True)
 assert all(sha(p)==h for p,h in pins.items());report['status']='complete_bounded_sampled_consensus_diagnostic';save()
if __name__=='__main__':main()
