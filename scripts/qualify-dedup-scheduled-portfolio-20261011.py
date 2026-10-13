#!/usr/bin/env python3
import hashlib,json,os,shutil,subprocess,re
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';A=Path('/Users/themoretheless/.codex/tmp/rrrah-scheduled-portfolio-qualified-20261011');OUT=D/'dedup-scheduled-portfolio-audit-20261011.json'
EXPECTED={'rrrah_dedup':2,'portfolio_collection':16,'pair_schedule':5,'projected_portfolio':24,'projected_collection':8}
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 assert not A.exists() and not OUT.exists();A.mkdir();paths=[R/'Cargo.toml',R/'Cargo.lock',Path(__file__)]
 for name in ['rrrah-dedup','rrrah-core','rrrah-decode','rrrah-memory']:
  paths += [p for p in (R/'crates'/name).rglob('*') if p.is_file() and 'target' not in p.parts and p.suffix in ['.rs','.toml']]
 live={str(p):sha(p) for p in paths};pins={}
 for p in paths:
  target=A/'source'/p.relative_to(R);target.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(p,target);assert sha(target)==live[str(p)];pins[str(target)]=sha(target)
 command=['cargo','test','-p','rrrah-dedup','--features','decode','--locked','--offline','--config','profile.dev.package.rrrah-dedup.opt-level=3','--no-run','--message-format=json','--lib']
 for name in EXPECTED:
  if name!='rrrah_dedup':command+=['--test',name]
 report={'status':'running','producer_pid':os.getpid(),'build_command':command,'results':[]}
 def save():
  p=OUT.with_suffix('.tmp');p.write_text(json.dumps(report,indent=2)+'\n');p.replace(OUT)
 save()
 try:
  process=subprocess.Popen(command,cwd=R,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True);report['build_pid']=process.pid;save();stdout,stderr=process.communicate();(A/'build.jsonl').write_text(stdout);(A/'build.log').write_text(stderr);assert process.returncode==0,stderr
  artifacts={}
  for line in stdout.splitlines():
   try:item=json.loads(line)
   except json.JSONDecodeError:continue
   if item.get('reason')=='compiler-artifact' and item.get('executable') and item['target']['name'] in EXPECTED:artifacts[item['target']['name']]=item
  assert set(artifacts)==set(EXPECTED)
  copied={}
  for name,item in artifacts.items():
   source=Path(item['executable']);target=A/name;digest=sha(source);shutil.copy2(source,target);assert sha(target)==digest;pins[str(target)]=digest;copied[name]=target
  for name,count in EXPECTED.items():
   binary=copied[name];args=[str(binary),*(['collection::scheduled_guard_tests::'] if name=='rrrah_dedup' else [])];row={'suite':name,'command':args};report['results'].append(row);save();proc=subprocess.Popen(args,cwd=R,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True);row['pid']=proc.pid;save();stdout,stderr=proc.communicate();log=A/(name+'.log');log.write_text(stdout+stderr);assert proc.returncode==0,(name,stdout,stderr)
   assert re.findall(r'test result: ok\. (\d+) passed; 0 failed;',stdout)==[str(count)],(name,stdout[-500:]);row.update(status='passed',tests=count,binary_sha256=sha(binary),log=str(log),log_sha256=sha(log));pins[str(log)]=sha(log);save();print(name,count,flush=True)
  assert all(sha(p)==h for p,h in live.items());assert all(sha(p)==h for p,h in pins.items())
  for p in [A/'build.jsonl',A/'build.log']:pins[str(p)]=sha(p)
  report.update(status='complete_bounded_scheduled_portfolio_generic_gate',tests=sum(EXPECTED.values()),programs=len(EXPECTED),pins=pins,scope='macOS copied programs, scheduled source/cancellation guards, portfolio confirmations/unknown/fatal outcomes, full pair schedule, legacy exact-first and projected collection regressions. Archived current source files; not a complete frozen-workspace build, native Torch execution, full corpus confirmation or Linux qualification of this integration.');save()
 except Exception as e:report.update(status='failed',error=str(e));save();raise
if __name__=='__main__':main()
