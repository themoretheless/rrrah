#!/usr/bin/env python3
import hashlib,json,os,re,shutil,subprocess
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp');A=T/'rrrah-model-bank-joint-recovery-qualified-20261011';OUT=D/'dedup-model-bank-joint-recovery-audit-20261011.json'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 assert not A.exists() and not OUT.exists();A.mkdir();mp=D/'dedup-model-bank-joint-recovery-snapshot-20261011.json';m=json.loads(mp.read_text());S=Path(m['snapshot']);assert all(sha(S/f)==h for f,h in m['file_hashes'].items());assert sha(m['parent_manifest'])==m['parent_sha256'];expected={'model_compare':26,'model_tiles':21,'search_portfolio':31,'portfolio_collection':26,'joint_witnesses':10,'geometry':9,'projective_domains':4,'joint_projective':4,'projective_pruning':3};cmd=['cargo','test','--manifest-path',str(S/'Cargo.toml'),'-p','rrrah-dedup','--no-default-features','--features','decode']
 for suite in expected:cmd+=['--test',suite]
 cmd+=['--locked','--offline','--no-run','--message-format=json','--config','profile.dev.package.rrrah-dedup.opt-level=3','--target-dir',str(T/'rrrah-opposite-native-snapshot-20261011/target')];report={'status':'running','producer_pid':os.getpid(),'source_manifest':str(mp),'pins':{str(mp):sha(mp),str(Path(__file__)):sha(Path(__file__))},'build_command':cmd,'results':[],'scope':'Frozen compatible four-crate source and lock, new joint primitive plus legacy/pair/collection tests. Source coherent before/after; root concurrent HDR edits preserved outside this gate. Not full workspace/platform/corpus/RSS qualification.'}
 def save():
  temp=OUT.with_suffix('.tmp');temp.write_text(json.dumps(report,indent=2)+'\n');temp.replace(OUT)
 save()
 try:
  proc=subprocess.Popen(cmd,cwd='/tmp',stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True);report['build_pid']=proc.pid;save();stdout,stderr=proc.communicate();artifacts=A/'build.jsonl';artifacts.write_text(stdout);log=A/'build.log';log.write_text(stderr);report['pins'].update({str(p):sha(p) for p in [artifacts,log]});save();assert proc.returncode==0,stderr;entries=[json.loads(s) for s in stdout.splitlines()];bins={e['target']['name']:e['executable'] for e in entries if e.get('reason')=='compiler-artifact' and e.get('executable') and e['target']['name'] in expected};assert set(bins)==set(expected)
  for suite,count in expected.items():
   binary=A/suite;shutil.copy2(bins[suite],binary);digest=sha(binary);result=subprocess.run([str(binary),'--test-threads=1'],capture_output=True,text=True);log=A/(suite+'.log');log.write_text(result.stdout+result.stderr);assert result.returncode==0 and re.findall(r'test result: ok\. (\d+) passed; 0 failed;',result.stdout)==[str(count)],str(log);assert sha(binary)==digest;report['results'].append({'suite':suite,'tests':count,'binary':str(binary),'binary_sha256':digest,'log':str(log),'log_sha256':sha(log)});save()
  assert all(sha(S/f)==h for f,h in m['file_hashes'].items());owned=['crates/rrrah-dedup/src/local_scan.rs','crates/rrrah-dedup/src/geometry.rs','crates/rrrah-dedup/src/anchor_rank_file.rs','crates/rrrah-dedup/src/search_portfolio.rs','crates/rrrah-dedup/src/portfolio_collection.rs','crates/rrrah-dedup/src/model_loftr.rs'];report.update(status='complete_bounded_frozen_joint_model_tests',tests=sum(expected.values()),owned_root_matches={f:sha(R/f)==m['file_hashes'][f] for f in owned});save();print(report['status'],report['tests'],flush=True)
 except Exception as e:report.update(status='failed',error=str(e));save();raise
if __name__=='__main__':main()
