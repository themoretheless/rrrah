"""Run complete unfiltered package correctness gate on one durable exact legacy variant."""
from pathlib import Path
import hashlib,json,os,re,shutil,subprocess,sys,traceback
R=Path(__file__).resolve().parents[1];D=R/'docs/research';h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();assert len(sys.argv)==2 and sys.argv[1] in ['baseline','candidate'];variant=sys.argv[1];manifest=D/'dedup-durable-source-pair-v5-prepared-20261015.json';pair=json.loads(manifest.read_text());snapshot=next(s for s in pair['variants'] if s['variant']==variant);source=Path(snapshot['source']);W=Path(pair['artifact_root']).parent;work=Path(pair['artifact_root'])/variant;out=D/f'dedup-durable-{variant}-gate-v5-20261015.json';assert not out.exists() and not work.exists();assert (D/'dedup-durable-pair-v5-20261015/cache-seed.json').is_file();assert len(snapshot['source_files'])==3378
if variant=='candidate':
 baseline=D/'dedup-durable-baseline-audit-v5-20261015.json';a=json.loads(baseline.read_text());assert a['gate_passed'] and a['failed']==0 and len(a['targets'])==162 and a['manifest_sha256']==h(manifest)
for row in snapshot['source_files']:assert h(source/row['path'])==row['sha256']
work.mkdir(parents=True);tmp=Path(pair['tmp_root'])/variant;tmp.mkdir(parents=True);env=dict(os.environ,TMPDIR=str(tmp),CARGO_INCREMENTAL='0',GIT_CEILING_DIRECTORIES=pair['git_ceiling'],PYTHONDONTWRITEBYTECODE='1');command=['cargo','+stable','test','--locked','--offline','-p','rrrah-decode','-p','rrrah-dedup','--features','rrrah-dedup/decode','--lib','--tests','--no-fail-fast','--jobs','2','--config','profile.test.lto="off"','--config','profile.test.package.rrrah-dedup.debug=0','--config','profile.test.package.rrrah-decode.debug=0','--target-dir',pair['target']];data=dict(status='starting',variant=variant,producer_pid=os.getpid(),producer_sha256=h(__file__),manifest=str(manifest),manifest_sha256=h(manifest),source=str(source),command=command,environment_overrides={k:env[k] for k in ['TMPDIR','CARGO_INCREMENTAL','GIT_CEILING_DIRECTORIES','PYTHONDONTWRITEBYTECODE']},compiler=subprocess.run(['rustc','+stable','-vV'],cwd=source,env=env,capture_output=True,text=True,check=True).stdout,retained_binaries=[],scope='Freshcomplete162lib/integrationtarget gate onexactlegacy3378sourcebytes, originalprofile/assertions/default+decode, nofilters/pruning. Workspace durable source/TMP/cache/artifacts, Gitceiling isolatesouterworkingGitmetadata. Controller/compiler recorded; notlatestRoot/allformats/platform/nativecorpus/benchmark/performance proof.')
def save():
 temp=out.with_suffix('.tmp');temp.write_text(json.dumps(data,indent=2)+'\n');temp.replace(out)
save();log=work/'cargo.log'
try:
 with log.open('x') as stream:
  child=subprocess.Popen(command,cwd=source,env=env,stdout=stream,stderr=subprocess.STDOUT);data.update(status='running',child_pid=child.pid);save();code=child.wait()
 data.update(status='retaining_terminal_inventory',returncode=code,cargo_log=str(log),cargo_log_sha256=h(log));save()
 for row in snapshot['source_files']:assert h(source/row['path'])==row['sha256']
 paths=re.findall(r'^\s*Running .+ \((/[^\n]+)\)$',log.read_text(),re.M)
 for index,path in enumerate(dict.fromkeys(paths)):
  original=Path(path);copy=work/f'binary-{index}-{original.name}';shutil.copyfile(original,copy);copy.chmod(0o755);assert h(original)==h(copy);listed=subprocess.run([str(copy),'--list'],cwd=source,env=env,capture_output=True,text=True);inventory=work/f'inventory-{index}.log';inventory.write_text(listed.stdout+listed.stderr);data['retained_binaries'].append(dict(original=str(original),copy=str(copy),sha256=h(copy),inventory=str(inventory),inventory_sha256=h(inventory),inventory_returncode=listed.returncode))
 data['status']='terminal_failure' if code else 'terminal_pending_inventory_audit';save();print(variant,code,'retained',len(data['retained_binaries']));sys.exit(code)
except BaseException as error:
 if isinstance(error,SystemExit):raise
 data.update(status='controller_failure',controller_error=type(error).__name__,controller_traceback=traceback.format_exc());save();raise
