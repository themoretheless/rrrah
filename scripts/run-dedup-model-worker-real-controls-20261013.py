"""Compare raw proposal bits and complete guarded model evidence across the process boundary."""
import pathlib,json,hashlib,subprocess,shutil,os,time
R=pathlib.Path(__file__).resolve().parents[1];D=R/'docs/research';T=pathlib.Path('/Users/themoretheless/.codex/tmp');W=T/'rrrah-model-worker-real-controls-20261013';H=W/'harness';S=T/'rrrah-model-worker-20261013/source';P=D/'dedup-model-worker-real-controls-20261013.json';assert not W.exists() and not P.exists();(H/'src').mkdir(parents=True)
def sha(p):return hashlib.sha256(pathlib.Path(p).read_bytes()).hexdigest()
old=T/'rrrah-current-native-bank-all-positives-harness-20261012';manifest=(old/'Cargo.toml').read_text().replace(str(T/'rrrah-dedup-current-decode-source-20261012'),str(S)).replace('name = "dedup-loftr-native-probe"','name = "dedup-model-worker-real-controls"').replace('opt-level = 3','opt-level = 1');manifest+='\n[profile.dev.package."*"]\nopt-level=3\n';
for name in ['rrrah-dedup','rrrah-core','rrrah-decode','rrrah-memory']:manifest+='\n[profile.dev.package.'+name+']\nopt-level=1\n'
(H/'Cargo.toml').write_text(manifest);shutil.copy2(old/'Cargo.lock',H/'Cargo.lock')
original=(old/'src/bin/native_bank_current_all_positives.rs').read_text();policy=original[original.index('    let policy = ModelComparisonPolicy {'):original.index('    let mut left=')]
text='''use rrrah_dedup::{anchor_rank::AnchorRankPolicy,animated::AnimationBudget,geometry::{GeometryPolicy,ProjectiveSamplingPolicy},local_rank::LocalRankPolicy,model_compare::{ModelComparisonPolicy,compare_model_files_with,ModelComparisonError,ProposalProviderError},model_input::ModelInputPolicy,proposals::ProposalPolicy,rank_region::RankRegionPolicy,model_worker::{ModelWorkerPolicy,ModelWorkerError,proposals_from_worker,serve_loftr_worker_once},worker::{WorkerLimits,WorkerError,WorkerStop}};
use std::{path::Path,io::Cursor,error::Error,time::Duration};
fn main()->Result<(),Box<dyn Error>>{
 let args:Vec<_>=std::env::args().skip(1).collect();assert_eq!(args.len(),7);tch::set_num_threads(2);tch::manual_seed(173);let budget=rrrah_core::MemoryBudget::new(512*1024*1024);
'''+policy+'''
 let mut left=rrrah_decode::DecodeRequest::new(&args[0]);left.assume_untagged_linear_srgb=true;let mut right=rrrah_decode::DecodeRequest::new(&args[1]);right.assume_untagged_linear_srgb=true;let weights=Path::new(&args[2]);let executable=Path::new(&args[5]);let runtime=&args[6];
 let worker=ModelWorkerPolicy{process:WorkerLimits{deadline:Duration::from_secs(300),poll_interval:Duration::from_millis(2),max_input_bytes:48+2*409600,max_stdout_bytes:16+40*5000,max_stderr_bytes:65536},max_input_pixels:409600,max_proposals:5000,threads:2,seed:173};
 let count;
 {
  let snapshot=rrrah_dedup::exact::ContentSnapshot::read(weights,128*1024*1024,||false)?;
  let l=rrrah_dedup::model_input_file::prepare_gray_model_file(&left,policy.decode,policy.input,&budget,||false)?;let r=rrrah_dedup::model_input_file::prepare_gray_model_file(&right,policy.decode,policy.input,&budget,||false)?;
  let mut request=Vec::new();request.extend_from_slice(b"RRMOD001");request.extend_from_slice(&5000u64.to_le_bytes());for i in [l.input(),r.input()]{for n in i.dimensions.into_iter().chain(i.original){request.extend_from_slice(&n.to_le_bytes());}}for i in [l.input(),r.input()]{request.extend_from_slice(&i.pixels);}
  let mut response=Vec::new();serve_loftr_worker_once(&mut Cursor::new(request),&mut response,weights,409600,5000,&budget,&||false)?;
  let isolated=proposals_from_worker(executable,weights,l.input(),r.input(),worker,&budget,&||false,|c|{c.env("DYLD_LIBRARY_PATH",runtime);})?;
  assert_eq!(&response[..8],b"RRPTS001");count=u64::from_le_bytes(response[8..16].try_into()?);assert_eq!(isolated.len() as u64,count);assert_eq!(response.len(),16+40*isolated.len());
  for (point,row)in isolated.iter().zip(response[16..].chunks_exact(40)){for (value,raw)in point.source.into_iter().chain(point.target).chain([point.confidence]).zip(row.chunks_exact(8)){assert_eq!(value.to_bits(),u64::from_le_bytes(raw.try_into()?));}}
  l.verify_source(||false)?;r.verify_source(||false)?;snapshot.verify(||false)?;
 }
 assert_eq!(budget.used(),0);
 let direct=rrrah_dedup::model_loftr::compare_loftr_files(&left,&right,weights,policy,&budget,||false)?;assert_eq!(budget.used(),0);
 let isolated=compare_model_files_with(&left,&right,weights,policy,&budget,|weights,l,r,b,c|proposals_from_worker(executable,weights,l,r,worker,b,c,|cmd|{cmd.env("DYLD_LIBRARY_PATH",runtime);}).map_err(|e|Box::new(e) as ProposalProviderError),||false)?;
 assert_eq!(format!("{direct:?}"),format!("{isolated:?}"));assert_eq!(budget.used(),0);
 let tiny=ModelWorkerPolicy{process:WorkerLimits{deadline:Duration::from_millis(1),poll_interval:Duration::from_millis(1),..worker.process},..worker};
 let error=compare_model_files_with(&left,&right,weights,policy,&budget,|weights,l,r,b,c|proposals_from_worker(executable,weights,l,r,tiny,b,c,|cmd|{cmd.env("DYLD_LIBRARY_PATH",runtime);}).map_err(|e|Box::new(e) as ProposalProviderError),||false).expect_err("worker deadline is not an empty bank");
 assert!(matches!(error,ModelComparisonError::Provider(ref e) if matches!(e.downcast_ref::<ModelWorkerError>(),Some(ModelWorkerError::Worker(WorkerError::Stopped{reason:WorkerStop::Deadline,..})))));assert_eq!(budget.used(),0);
 println!("{}",serde_json::json!({"raw_proposals":count,"raw_bits_equal":true,"direct_supported":direct.supported(),"isolated_supported":isolated.supported(),"complete_model_evidence_equal":true,"timeout_is_provider_error":true,"managed_used":budget.used(),"managed_peak":budget.peak(),"direct_evidence":format!("{direct:?}")}));Ok(())
}
''';(H/'src/main.rs').write_text(text)
base=json.loads((D/'dedup-real-model-miss-diagnostics-20261013.json').read_text());plan=base['plan'];runtime=T/'rrrah-loftr-native-runtime-20261010';dyld=str(runtime/'lib/python3.12/site-packages/torch/lib');env=dict(os.environ,LIBTORCH_USE_PYTORCH='1');env['PATH']=str(runtime/'bin')+':'+env['PATH'];env['DYLD_LIBRARY_PATH']=dyld
pins={str(p):sha(p) for p in [H/'Cargo.toml',H/'Cargo.lock',H/'src/main.rs',pathlib.Path(__file__)]}
for row in plan:
 for key in ['left','right','weights']:pins[row[key]]=sha(row[key])
data={'status':'waiting_for_native_worker_build','producer_pid':os.getpid(),'harness':str(H),'source':str(S),'plan':plan,'results':[],'pins':pins,'scope':'Two fixed real input pairs; raw native proposal bit parity and complete guarded comparison evidence parity for one full-image inference each. One child per provider invocation. Not25bank performance, full458, OS RSS or persistent reuse qualification.'}
def save():P.write_text(json.dumps(data,indent=2)+'\n')
save();Q=D/'dedup-model-worker-20261013.json'
while True:
 q=json.loads(Q.read_text())
 if q['status'] in ['passed','failed']:break
 time.sleep(2)
assert q['status']=='passed','worker qualification/build failed';assert sha(q['binary'])==q['binary_sha256'];pins[q['binary']]=q['binary_sha256']
cmd=['cargo','build','--offline','--target-dir',str(T/'rrrah-cache-interpretation-fix-20261012/target')];data.update(status='building',build_command=cmd);save()
with (W/'build.log').open('x') as log:
 child=subprocess.Popen(cmd,cwd=H,env=env,stdout=log,stderr=subprocess.STDOUT);data['child_pid']=child.pid;save();code=child.wait()
data['build_returncode']=code;pins[str(W/'build.log')]=sha(W/'build.log');pins[str(H/'Cargo.lock')]=sha(H/'Cargo.lock');save()
if code:data['status']='terminal_build_failure';save();raise SystemExit(code)
binary=W/'probe';shutil.copy2(T/'rrrah-cache-interpretation-fix-20261012/target/debug/dedup-model-worker-real-controls',binary);pins[str(binary)]=sha(binary)
for row in plan:
 cmd=[str(binary),row['left'],row['right'],row['weights'],'640',str(row['tolerance']),q['binary'],dyld];data.update(status='running',active_kind=row['kind'],command=cmd);save();child=subprocess.Popen(cmd,env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True);data['child_pid']=child.pid;save();out,err=child.communicate();path=W/(row['kind']+'.json');path.write_text(json.dumps(dict(kind=row['kind'],command=cmd,returncode=child.returncode,stdout=out,stderr=err),indent=2)+'\n');data['results'].append(dict(kind=row['kind'],report=str(path),sha256=sha(path)));save()
 if child.returncode:break
data['status']='complete_pending_audit' if len(data['results'])==2 and all(json.loads(pathlib.Path(r['report']).read_text())['returncode']==0 for r in data['results']) else 'terminal_execution_failure';save();print(data['status'])
