#!/usr/bin/env python3
import hashlib,json,os,re,shutil,subprocess
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp');A=T/'rrrah-opposite-native-current-qualified-20261011';OUT=D/'dedup-opposite-native-current-audit-20261011.json'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 assert not A.exists() and not OUT.exists();A.mkdir();paths=[R/'Cargo.toml',R/'Cargo.lock',Path(__file__)]
 for name in ['rrrah-dedup','rrrah-core','rrrah-decode','rrrah-memory']:paths.extend(p for p in (R/'crates'/name).rglob('*') if p.is_file() and p.suffix in ['.rs','.toml'])
 paths.extend(p for p in (R/'crates/rrrah-dedup/tests/fixtures/rotation').rglob('*') if p.is_file());hashes={str(p):sha(p) for p in paths};pins={}
 for p in paths:
  q=A/'source'/p.relative_to(R);q.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(p,q);assert sha(q)==hashes[str(p)];pins[str(q)]=sha(q)
 command=['cargo','test','--manifest-path',str(R/'Cargo.toml'),'-p','rrrah-dedup','--features','decode','--test','opposite_native','--locked','--offline','--no-run','--message-format=json','--config','profile.dev.package.rrrah-dedup.opt-level=3','--target-dir',str(T/'rrrah-opposite-native-snapshot-20261011/target')];report={'status':'running','producer_pid':os.getpid(),'build_command':command,'source_before':hashes,'archive_pins':pins,'scope':'Current four-crate Rust/TOML sources, root workspace manifest/current lock and rotation fixtures, copied program, current macOS. Cwd/tmp avoids loading root cwd cargo config. Not full workspace/platform/corpus/RSS qualification; source capture is checked before/after, not atomic.'}
 def save():
  tmp=OUT.with_suffix('.tmp');tmp.write_text(json.dumps(report,indent=2)+'\n');tmp.replace(OUT)
 save()
 try:
  proc=subprocess.Popen(command,cwd='/tmp',stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True);report['build_pid']=proc.pid;save();stdout,stderr=proc.communicate();artifacts=A/'build.jsonl';artifacts.write_text(stdout);log=A/'build.log';log.write_text(stderr);report.update(build_returncode=proc.returncode,build_artifacts=str(artifacts),build_log=str(log));save();assert proc.returncode==0,stderr
  entries=[json.loads(s) for s in stdout.splitlines()];bins=[v for v in entries if v.get('reason')=='compiler-artifact' and v.get('executable') and v['target']['name']=='opposite_native'];assert len(bins)==1;binary=A/'opposite_native';shutil.copy2(bins[0]['executable'],binary);before=sha(binary);result=subprocess.run([str(binary),'--test-threads=1'],capture_output=True,text=True);testlog=A/'tests.log';testlog.write_text(result.stdout+result.stderr);report.update(test_returncode=result.returncode,test_log=str(testlog));save();assert result.returncode==0 and re.findall(r'test result: ok\. (\d+) passed; 0 failed;',result.stdout)==['16'];assert all(sha(p)==h for p,h in hashes.items());assert sha(binary)==before;report.update(status='complete_bounded_current_source_opposite_native_tests',tests=16,binary=str(binary),binary_sha256=before,pins={str(p):sha(p) for p in [artifacts,log,testlog]});save();print(report['status'],16,flush=True)
 except Exception as e:report.update(status='failed',error=str(e));save();raise
if __name__=='__main__':main()
