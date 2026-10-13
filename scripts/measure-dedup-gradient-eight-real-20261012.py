#!/usr/bin/env python3
import hashlib,json,os,re,shutil,statistics,subprocess,time
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 mp=D/'dedup-native-bank-outcome-native-snapshot-20261011.json';m=json.loads(mp.read_text());parent=Path(m['snapshot']);s=T/'rrrah-gradient-eight-real-source-20261012';out=D/'dedup-gradient-eight-real-20261012.json';assert not s.exists() and not out.exists()
 for k,v in m['file_hashes'].items():
  assert sha(parent/k)==v;q=s/k;q.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(parent/k,q)
 candidate=T/'rrrah-gradient-eight-candidate-20261012'
 for name in ['gradient.rs','gradient_batch_tests.rs']:shutil.copy2(candidate/'src'/name,s/'crates/rrrah-dedup/src'/name)
 p=s/'crates/rrrah-dedup/src/gradient_batch_tests.rs';text=p.read_text();a=text.index('fn real_photo_matcher_timing()');b=text.index('\n#[test]',a);text=text[:a]+text[a:b].replace('scalar(', 'current_four_lane(')+text[b:];p.write_text(text)
 pins={str(s/k):sha(s/k) for k in m['file_hashes']};pins.update({str(p):sha(p) for p in [mp,Path(__file__),D/'dedup-gradient-eight-candidate-source-20261012.json']})
 paths=[T/'rrrah-dedup-copydays'/f for f in ['original-resolution-negative-200201/200300.jpg','original-resolution-strong-all/200302.jpg','original-resolution-negative-200201/201400.jpg','original-resolution-negative-200201/212400.jpg','original-resolution-strong-all/212402.jpg']];pins.update({str(p):sha(p) for p in paths})
 r={'status':'building','producer_pid':os.getpid(),'pins':pins,'scope':'Isolated candidate: actual current-four versus candidate-eight complete matcher, alternating on decoded original JPEG descriptors. No root adoption or whole-operation speedup claim.'}
 def save():p=out.with_suffix('.tmp');p.write_text(json.dumps(r,indent=2)+'\n');p.replace(out)
 save()
 try:
  log=T/'rrrah-gradient-eight-real-build-20261012.log';cmd=['cargo','test','--manifest-path',str(s/'Cargo.toml'),'-p','rrrah-dedup','--features','decode','--lib','--offline','--no-run','--target-dir',str(T/'rrrah-native-no-torch-target-20261012')]
  with log.open('w')as f:
   p=subprocess.Popen(cmd,cwd='/tmp',stdout=f,stderr=subprocess.STDOUT);r['build_pid']=p.pid;save();code=p.wait()
  r.update(build_returncode=code,build_log=str(log),build_log_sha256=sha(log));save();assert code==0
  matches=re.findall(r'Executable unittests .*\(([^)]+)\)',log.read_text());assert len(matches)==1;binary=T/'rrrah-gradient-eight-real-executable-20261012';assert not binary.exists();shutil.copy2(matches[0],binary);pins[str(binary)]=sha(binary)
  env=os.environ.copy();env['RRRAH_GRADIENT_BENCH_FILES']=':'.join(map(str,paths));cmd=[str(binary),'gradient::batch_tests::real_photo_matcher_timing','--ignored','--nocapture'];r.update(status='running',command=cmd);start=time.monotonic();p=subprocess.Popen(cmd,env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True);r['child_pid']=p.pid;save();stdout,stderr=p.communicate();r.update(returncode=p.returncode,stdout=stdout,stderr=stderr,elapsed_seconds=time.monotonic()-start);assert p.returncode==0 and '1 passed; 0 failed' in stdout
  rows=[]
  for pair,n,k,radius,eight,count,seconds in re.findall(r'real_matcher pair=(\S+) features=(\d+):(\d+) radius=(\S+) batched=(true|false) matches=(\d+) seconds=([0-9.]+)',stderr):rows.append(dict(pair=pair,left_features=int(n),right_features=int(k),radius=radius,eight=eight=='true',matches=int(count),seconds=float(seconds)))
  assert len(rows)==40;summary=[]
  for i in range(5):
   for radius in ['None','Some(64.0)']:
    case=[x for x in rows if x['pair']==f'0:{i}'and x['radius']==radius];assert len(case)==4 and [x['eight'] for x in case]==[False,True,True,False] and len({x['matches']for x in case})==1
    four=statistics.median(x['seconds']for x in case if not x['eight']);eight=statistics.median(x['seconds']for x in case if x['eight']);summary.append(dict(pair=f'0:{i}',radius=radius,current_four_seconds=four,candidate_eight_seconds=eight,four_over_eight=four/eight))
  assert all(sha(p)==v for p,v in pins.items());r.update(status='complete_ten_real_descriptor_controls_not_adopted',samples=rows,summary=summary);save();print(r['status'],flush=True)
 except Exception as e:r.update(status='failed',error=str(e));save();raise
if __name__=='__main__':main()
