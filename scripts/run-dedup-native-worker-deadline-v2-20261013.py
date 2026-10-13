"""Use the new supervisor to stop and reap the actual native comparison executable."""
import pathlib,json,hashlib,subprocess,os
R=pathlib.Path(__file__).resolve().parents[1];D=R/'docs/research';T=pathlib.Path('/Users/themoretheless/.codex/tmp');W=T/'rrrah-native-worker-deadline-v2-20261013';S=T/'rrrah-worker-supervisor-20261013/source';P=D/'dedup-native-worker-deadline-v2-20261013.json';assert not W.exists() and not P.exists();(W/'src').mkdir(parents=True)
def sha(p):return hashlib.sha256(pathlib.Path(p).read_bytes()).hexdigest()
qualified=json.loads((D/'dedup-worker-supervisor-20261013.json').read_text());assert qualified['status']=='passed';assert all(sha(S/'crates/rrrah-dedup'/f)==h for f,h in qualified['inputs'].items())
(W/'Cargo.toml').write_text('[package]\nname="dedup-native-worker-deadline"\nversion="0.0.0"\nedition="2024"\n[workspace]\n[dependencies]\nrrrah-dedup={path="'+str(S/'crates/rrrah-dedup')+'",default-features=false}\n')
(W/'src/main.rs').write_text('''use rrrah_dedup::worker::{run_worker, WorkerLimits, WorkerError, WorkerStop};
use std::{process::Command,time::Duration};
fn main() {
 let mut args=std::env::args_os().skip(1);let mut command=Command::new(args.next().expect("native worker executable"));command.args(args);command.env("DYLD_LIBRARY_PATH",std::env::var_os("RRRAH_NATIVE_WORKER_DYLD").expect("explicit native worker runtime"));
 let limits=WorkerLimits{deadline:Duration::from_millis(200),poll_interval:Duration::from_millis(2),max_input_bytes:0,max_stdout_bytes:1048576,max_stderr_bytes:65536};
 let error=run_worker(command,&[],limits,||false).expect_err("long native comparison must hit worker deadline");
 assert!(matches!(error,WorkerError::Stopped{reason:WorkerStop::Deadline,status:Some(status)} if !status.success()));
 println!("native_worker_deadline=terminated_and_reaped");
}
''')
# Use the source workspace's exact lock resolution; Cargo may only add this harness package.
import shutil
shutil.copy2(T/'rrrah-native-worker-deadline-20261013/Cargo.lock',W/'Cargo.lock')
native=json.loads((D/'dedup-real-model-miss-diagnostics-20261013.json').read_text());item=native['plan'][0];binary=native['binary'];env=dict(os.environ);env['RRRAH_NATIVE_WORKER_DYLD']=str(T/'rrrah-loftr-native-runtime-20261010/lib/python3.12/site-packages/torch/lib')
cmd=['cargo','run','--locked','--offline','--target-dir',str(T/'rrrah-cache-interpretation-fix-20261012/target'),'--',binary,item['left'],item['right'],item['weights'],'640','2.0','1000000'];log=W/'test.log';pins={str(p):sha(p) for p in [W/'Cargo.toml',W/'src/main.rs',D/'dedup-worker-supervisor-20261013.json',pathlib.Path(__file__),pathlib.Path(binary),pathlib.Path(item['left']),pathlib.Path(item['right']),pathlib.Path(item['weights'])]};data={'status':'running','producer_pid':os.getpid(),'source':str(S),'command':cmd,'log':str(log),'pins':pins,'scope':'Actual native comparison worker stopped at observed200ms deadline and waited on. No assertion that a Torch kernel had started; no RSS or hard real-time bound.'}
def save():P.write_text(json.dumps(data,indent=2)+'\n')
save()
with log.open('x') as out:
 child=subprocess.Popen(cmd,cwd=W,env=env,stdout=out,stderr=subprocess.STDOUT);data['child_pid']=child.pid;save();code=child.wait()
pins[str(log)]=sha(log);pins[str(W/'Cargo.lock')]=sha(W/'Cargo.lock');data.update(status='passed' if code==0 and 'native_worker_deadline=terminated_and_reaped' in log.read_text() else 'failed',returncode=code);save();print(data['status'])
