#!/usr/bin/env python3
"""Frozen all-copy collection latency baseline; payload read counts are analytical."""
import hashlib,json,os,re,shutil,subprocess,statistics
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1];D=ROOT/'docs/research'
MANIFEST=D/'dedup-exact-collection-scaling-baseline-snapshot-20261011.json'
OUT=D/'dedup-exact-collection-scaling-baseline-20261011.json'
ARCHIVE=Path('/Users/themoretheless/.codex/tmp/rrrah-exact-collection-scaling-baseline-qualified-20261011')
TEST='exact_collection_scaling_latency_probe'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 assert not OUT.exists() and not ARCHIVE.exists();ARCHIVE.mkdir()
 m=json.loads(MANIFEST.read_text());snapshot=Path(m['snapshot'])
 def verify():assert all(sha(snapshot/p)==h for p,h in m['file_hashes'].items())
 verify();report={'status':'running','producer_pid':os.getpid(),'source_manifest':str(MANIFEST),'source_manifest_sha256':sha(MANIFEST)}
 def save():
  t=OUT.with_suffix('.json.tmp');t.write_text(json.dumps(report,indent=2)+'\n');t.replace(OUT)
 save()
 try:
  command=['cargo','test','--manifest-path',str(snapshot/'Cargo.toml'),'--target-dir',str(ROOT/'target'),'-p','rrrah-dedup','--locked','--offline','--features','decode','--test','projected_portfolio','--no-run','--message-format=json']
  report['build_command']=command;p=subprocess.Popen(command,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True);report['build_pid']=p.pid;save();stdout,stderr=p.communicate();report['build_returncode']=p.returncode
  build=ARCHIVE/'build.jsonl';build.write_text(stdout);build_log=ARCHIVE/'build.log';build_log.write_text(stderr);assert p.returncode==0,stderr
  artifacts=[json.loads(l) for l in stdout.splitlines() if l.startswith('{')]
  artifacts=[x for x in artifacts if x.get('reason')=='compiler-artifact' and x.get('executable') and x['target']['name']=='projected_portfolio'];assert len(artifacts)==1
  item=artifacts[0];assert str(snapshot) in item['package_id'];source=Path(item['executable']);binary=ARCHIVE/'projected_portfolio';digest=sha(source);shutil.copy2(source,binary);assert digest==sha(binary)==sha(source) and str(snapshot).encode() in binary.read_bytes()
  command=[str(binary),TEST,'--exact','--ignored','--nocapture'];report.update(command=command,binary=str(binary),binary_sha256=digest);save()
  p=subprocess.Popen(command,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True);report['process_pid']=p.pid;save();stdout,stderr=p.communicate();log=ARCHIVE/'scaling.log';log.write_text(stdout+stderr);report['returncode']=p.returncode;assert p.returncode==0 and 'test result: ok. 1 passed; 0 failed;' in stdout,(stdout,stderr)
  rows=[json.loads(l.split(' ',1)[1]) for l in stdout.splitlines() if l.startswith('RRRAH_EXACT_SCALING ')]
  assert [r['files'] for r in rows]==[8,32,64]
  for row in rows:
   n=row['files'];pairs=n*(n-1)//2;assert row['pairs']==row['exact_pairs']==pairs and row['families']==1
   assert len(row['elapsed_ns'])==3 and all(v>0 for v in row['elapsed_ns'])
   assert row['analytical_source_bytes']==(4*n+6*pairs)*row['file_bytes']
   row['median_seconds']=statistics.median(row['elapsed_ns'])/1e9
  verify();assert sha(binary)==digest
  pins={str(p):sha(p) for p in [MANIFEST,Path(__file__),build,build_log,log,binary]}
  report.update(status='complete_frozen_all_copy_collection_scaling_baseline',controls=3,samples=9,rows=rows,pins=pins,scope='macOS all-copy exhaustive pair reports on frozen production; latency measured, source payload bytes analytically derived; no device-I/O/RSS, mixed visual collection or universal performance claim')
 except Exception as e:report.update(status='failed_all_copy_collection_scaling_baseline',error=str(e));raise
 finally:save()
 print(report['status'])
 for r in report['rows']:print(r['files'],r['pairs'],round(r['median_seconds'],3),r['analytical_source_bytes'])
if __name__=='__main__':main()
