#!/usr/bin/env python3
import hashlib,json,os,re,shutil,subprocess
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp');A=T/'rrrah-joint-projective-portfolio-current-qualified-20261011';OUT=D/'dedup-joint-projective-portfolio-current-audit-20261011.json'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 assert not A.exists() and not OUT.exists();A.mkdir();paths=[R/'Cargo.toml',R/'Cargo.lock',Path(__file__)]
 for name in ['rrrah-dedup','rrrah-core','rrrah-decode','rrrah-memory']:paths.extend(p for p in (R/'crates'/name).rglob('*') if p.is_file() and p.suffix in ['.rs','.toml'])
 paths.extend(p for p in (R/'crates/rrrah-dedup/tests/fixtures/rotation').rglob('*') if p.is_file());hashes={str(p):sha(p) for p in paths};pins={}
 for p in paths:
  q=A/'source'/p.relative_to(R);q.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(p,q);assert sha(q)==hashes[str(p)];pins[str(q)]=sha(q)
 cmd=['cargo','test','--manifest-path',str(R/'Cargo.toml'),'-p','rrrah-dedup','--features','decode','--test','anchor_reflection_fallback','--test','opposite_native','--test','search_portfolio','--test','portfolio_collection','--locked','--offline','--no-run','--message-format=json','--config','profile.dev.package.rrrah-dedup.opt-level=3','--target-dir',str(T/'rrrah-opposite-native-snapshot-20261011/target')];report={'status':'running','producer_pid':os.getpid(),'build_command':cmd,'source_before':hashes,'archive_pins':pins,'results':[],'scope':'Current-source/current-lock macOS gate: legacy reflection/opposite tests and primary/recovery pair and collection portfolio tests. Copied four-crate Rust/TOML and fixture inputs, pre/post hashes, not atomic capture/full workspace/platform/corpus/RSS qualification.'}
 def save():
  temp=OUT.with_suffix('.tmp');temp.write_text(json.dumps(report,indent=2)+'\n');temp.replace(OUT)
 save()
 try:
  proc=subprocess.Popen(cmd,cwd='/tmp',stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True);report['build_pid']=proc.pid;save();stdout,stderr=proc.communicate();artifacts=A/'build.jsonl';artifacts.write_text(stdout);buildlog=A/'build.log';buildlog.write_text(stderr);assert proc.returncode==0,stderr;entries=[json.loads(s) for s in stdout.splitlines()];expected={'anchor_reflection_fallback':12,'opposite_native':4,'search_portfolio':17,'portfolio_collection':19};bins={v['target']['name']:v['executable'] for v in entries if v.get('reason')=='compiler-artifact' and v.get('executable') and v['target']['name'] in expected};assert set(bins)==set(expected)
  for name,count in expected.items():
   binary=A/name;shutil.copy2(bins[name],binary);digest=sha(binary);result=subprocess.run([str(binary),'--test-threads=1'],capture_output=True,text=True);log=A/(name+'.log');log.write_text(result.stdout+result.stderr);assert result.returncode==0 and re.findall(r'test result: ok\. (\d+) passed; 0 failed;',result.stdout)==[str(count)];assert sha(binary)==digest;report['results'].append({'suite':name,'tests':count,'binary':str(binary),'binary_sha256':digest,'log':str(log),'log_sha256':sha(log)});save()
  assert all(sha(p)==h for p,h in hashes.items());report.update(status='complete_bounded_current_source_joint_projective_portfolio_tests',tests=52,pins={str(p):sha(p) for p in [artifacts,buildlog]},adopted_current={f:sha(R/f) for f in ['crates/rrrah-dedup/src/anchor_rank_file.rs','crates/rrrah-dedup/src/search_portfolio.rs','crates/rrrah-dedup/tests/search_portfolio.rs','crates/rrrah-dedup/src/portfolio_collection.rs','crates/rrrah-dedup/tests/portfolio_collection.rs','crates/rrrah-dedup/src/model_loftr.rs','crates/rrrah-dedup/src/geometry.rs']});save();print(report['status'],52,flush=True)
 except Exception as e:report.update(status='failed',error=str(e));save();raise
if __name__=='__main__':main()
