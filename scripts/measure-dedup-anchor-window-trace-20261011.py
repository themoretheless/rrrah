#!/usr/bin/env python3
import hashlib,json,os,re,shutil,subprocess,time
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 out=D/'dedup-anchor-window-trace-20261011.json';A=T/'rrrah-anchor-window-trace-qualified-20261011';assert not out.exists() and not A.exists();A.mkdir()
 mp=D/'dedup-anchor-window-trace-snapshot-20261011.json';m=json.loads(mp.read_text());assert all(sha(Path(m['snapshot'])/f)==h for f,h in m['file_hashes'].items());assert all(sha(Path(m['harness'])/f)==h for f,h in m['harness_hashes'].items())
 build=T/'dedup-anchor-window-trace-build-20261011.jsonl';entries=[json.loads(x) for x in build.read_text().splitlines()];assert entries[-1]['reason']=='build-finished' and entries[-1]['success'];bins=[e['executable'] for e in entries if e.get('reason')=='compiler-artifact' and e.get('executable')];assert len(bins)==1;binary=A/'sampling_trace';shutil.copy2(bins[0],binary)
 rp=D/'dedup-model-joint-refit-20261011.json';base=json.loads(rp.read_text());assert all(sha(p)==h for p,h in base['pins'].items());pins=dict(base['pins']);pins.update({str(p):sha(p) for p in [mp,build,T/'dedup-anchor-window-trace-build-20261011.log',binary,rp,Path(__file__)]})
 report={'status':'running','producer_pid':os.getpid(),'pins':pins,'results':[],'scope':'Frozen diagnostic counts per unchanged geometrically selected anchor window. Exact aggregate output parity with previous201300 refit required. No per-window filtering, threshold relaxation, production change, or proof of unique mismatch cause.'}
 def save():out.write_text(json.dumps(report,indent=2)+'\n')
 save()
 for old in base['results'][:2]:
  assert all(sha(p)==h for p,h in pins.items());row={'trials':old['trials'],'status':'running'};report['results'].append(row);save();start=time.monotonic();cmd=[str(binary),old['command'][1],str(old['trials']),'173'];p=subprocess.Popen(cmd,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True);row['pid']=p.pid;save();stdout,stderr=p.communicate();log=A/f"{old['trials']}.log";log.write_text(stdout+stderr);row.update(returncode=p.returncode,elapsed_seconds=time.monotonic()-start,stdout=stdout,stderr=stderr,log=str(log),log_sha256=sha(log));assert p.returncode==0,stderr;v=json.loads(stdout);assert v==json.loads(old['stdout']);assert v['managed_used']==0
  windows=[]
  for line in stderr.splitlines():
   assert line.startswith('RRRAH_ANCHOR_WINDOW\t');_,a,b,rect,e=line.split('\t');counts=dict((k,int(n)) for k,n in re.findall(r'(sites|valid_sites|informative_pairs|agreeing_pairs|pixel_reads): (\d+)',e));assert len(counts)==5;windows.append({'source_dimensions':json.loads(a.replace('(', '[').replace(')', ']')),'target_dimensions':json.loads(b.replace('(', '[').replace(')', ']')),'rectangle':json.loads(rect),'counts':counts})
  n=v['anchor_result']['anchors'][0];assert len(windows)==sum(v['anchor_result']['anchors'])
  for ws,aggregate in zip([windows[:n],windows[n:]],v['anchor_result']['directions']):
   for key in aggregate:assert sum(w['counts'][key] for w in ws)==aggregate[key]
  row.update(status='verified_exact_aggregate_parity',windows=windows);save();print(old['trials'],len(windows),'windows',flush=True)
 assert all(sha(p)==h for p,h in pins.items());report['status']='complete_verified_anchor_window_trace';save()
if __name__=='__main__':main()
