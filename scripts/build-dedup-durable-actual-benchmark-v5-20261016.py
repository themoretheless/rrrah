"""Build optimized actual-engine binary only after both complete audited durable gates."""
from pathlib import Path
import argparse,hashlib,json,os,runpy,shutil,subprocess
R=Path(__file__).resolve().parents[1];D=R/'docs/research';h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 parser=argparse.ArgumentParser();parser.add_argument('--check-only',action='store_true');args=parser.parse_args();P=D/'dedup-durable-actual-benchmark-v5-protocol-20261016.json';p=json.loads(P.read_text());manifest=Path(p['manifest']);assert h(manifest)==p['manifest_sha256'];checker=R/p['gate_admission'];assert h(checker)==p['gate_admission_sha256'];missing=[v for v in ['baseline','candidate'] if not (D/f'dedup-durable-{v}-audit-v5-20261015.json').is_file()]
 if missing:
  assert args.check_only,'Complete durable correctness audits still missing: '+','.join(missing)
  print(json.dumps({'status':'waiting_for_complete_durable_correctness','missing_variant_audits':missing,'compiled':False,'timed':False}));return
 runpy.run_path(str(checker));pair=json.loads(manifest.read_text());source=Path(p['source']);candidate=next(v for v in pair['variants'] if v['variant']=='candidate');assert candidate['source']==p['source'];assert h(source/'crates/rrrah-dedup/src/collection.rs')==p['candidate_collection_sha256']
 if args.check_only:print(json.dumps({'status':'full_paired_correctness_admitted_build_not_started','compiled':False,'timed':False}));return
 W=Path(p['work']);W.mkdir(exist_ok=False);tmp=W/'tmp';tmp.mkdir();env=dict(os.environ,TMPDIR=str(tmp),CARGO_INCREMENTAL='0',GIT_CEILING_DIRECTORIES=pair['git_ceiling'],PYTHONDONTWRITEBYTECODE='1');log=W/'cargo-json.log';err=W/'cargo-stderr.log';cmd=p['build_command']
 with log.open('x') as stdout,err.open('x') as stderr:proc=subprocess.run(cmd,cwd=source,env=env,stdout=stdout,stderr=stderr)
 assert proc.returncode==0,'Build failed; rawlogs retained; no timing'
 events=[json.loads(line) for line in log.read_text().splitlines() if line.startswith('{')];finished=[x for x in events if x.get('reason')=='build-finished'];assert len(finished)==1 and finished[0]['success'];artifacts=[x for x in events if x.get('reason')=='compiler-artifact' and x.get('target',{}).get('name')=='rrrah_dedup' and x.get('profile',{}).get('test') and x.get('executable')];assert len(artifacts)==1;artifact=artifacts[0];profile=artifact['profile']
 for key,value in p['compiler_profile_required'].items():assert profile[key]==value,(key,profile)
 original=Path(artifact['executable']);binary=W/'engine-opt3';shutil.copyfile(original,binary);binary.chmod(0o755);assert h(original)==h(binary);inventory=subprocess.check_output([str(binary),'--list'],cwd=source,env=env,text=True);(W/'inventory.log').write_text(inventory);assert inventory.count(p['test_name']+': test')==1
 for row in candidate['source_files']:assert h(source/row['path'])==row['sha256']
 out=D/'dedup-durable-actual-benchmark-v5-build-20261016.json';assert not out.exists();out.write_text(json.dumps({'status':'compiled_opt3_not_timed','protocol':str(P),'protocol_sha256':h(P),'producer_sha256':h(__file__),'manifest_sha256':h(manifest),'command':cmd,'profile':profile,'compiler':subprocess.check_output(['rustc','+stable','-vV'],text=True),'binary':str(binary),'binary_sha256':h(binary),'inventory':str(W/'inventory.log'),'inventory_sha256':h(W/'inventory.log'),'logs':{str(x):h(x) for x in [log,err]},'full_gate_pins':{v:h(D/f'dedup-durable-{v}-audit-v5-20261015.json') for v in ['baseline','candidate']},'scope':'Both full162target correctnessaudits admitted, samefrozen3378sourcecandidate opt3libtestbinary withactualenginebenchmark retained. Completeopt3librarytests not rerun here; no timing/speedup/productionadoption.'},indent=2)+'\n');print('Retained compiler-confirmed opt3samebinary, nottimed')
if __name__=='__main__':main()
