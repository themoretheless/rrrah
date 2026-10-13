"""Execute fixed real-LoFTR negative/positive controls on qualified diagnostics source."""
import pathlib,json,hashlib,subprocess,shutil,os,time
R=pathlib.Path(__file__).resolve().parents[1];D=R/'docs/research';T=pathlib.Path('/Users/themoretheless/.codex/tmp');W=T/'rrrah-real-model-miss-diagnostics-20261013';H=W/'harness';P=D/'dedup-real-model-miss-diagnostics-20261013.json'
assert not W.exists() and not P.exists();W.mkdir();H.mkdir()
def sha(p):return hashlib.sha256(pathlib.Path(p).read_bytes()).hexdigest()
audit=D/'dedup-model-miss-diagnostics-audit-20261012.json';a=json.loads(audit.read_text());assert a['status']=='verified_compact_model_miss_diagnostics';assert all(sha(p)==h for p,h in a['pins'].items())
qualified=json.loads((D/'dedup-model-miss-diagnostics-20261012.json').read_text());S=pathlib.Path(qualified['source']);assert all(sha(S/'crates/rrrah-dedup'/f)==h for f,h in qualified['inputs'].items())
O=T/'rrrah-current-native-bank-all-positives-harness-20261012';old_source=T/'rrrah-dedup-current-decode-source-20261012';manifest=(O/'Cargo.toml').read_text();assert manifest.count(str(old_source))==7;(H/'Cargo.toml').write_text(manifest.replace(str(old_source),str(S)));shutil.copy2(O/'Cargo.lock',H/'Cargo.lock');(H/'src/bin').mkdir(parents=True);shutil.copy2(O/'src/native_policy.rs',H/'src/native_policy.rs')
name='native_bank_miss_diagnostics_20261013';caller=(O/'src/bin/native_bank_current_all_positives.rs').read_text();needle='"model_attempted":p.evidence.model_attempted(),';assert caller.count(needle)==1
caller=caller.replace(needle,needle+'''"model_search":p.evidence.model_search.as_ref().map(|e|serde_json::json!({"attempts":e.attempts,"raw_proposals":e.comparison.raw_proposals,"selected_points":e.comparison.selected_points,"pixel_attempts":e.pixel_attempts,"geometry_candidates":e.diagnostics.geometry_candidates,"pixel_geometry_refusals":e.diagnostics.pixel_geometry_refusals,"pixel_rejections":e.diagnostics.pixel_rejections,"confirmed_transform":e.comparison.transform.is_some()})),''');(H/'src/bin'/f'{name}.rs').write_text(caller)
negative=json.loads((D/'dedup-current-heldout-negatives-20261012.json').read_text());positive=json.loads((D/'dedup-current-native-bank-all-positives-20261012.json').read_text());pos=next(v for v in positive['plan'] if v['baseline_index']==0);neg=negative['plan'][0]
plan=[dict(kind='fixed_negative_index0',left=neg['left'],right=neg['right'],weights=neg['weights'],tolerance=2.0,baseline=negative['results'][0]['report']),dict(kind='publisher_positive_baseline_index0',left=pos['left'],right=pos['right'],weights=neg['weights'],tolerance=pos['source_tolerance'],baseline=next(v['report'] for v in positive['results'] if v['baseline_index']==0))]
runtime=T/'rrrah-loftr-native-runtime-20261010';pins={str(p):sha(p) for p in [audit,H/'Cargo.toml',H/'Cargo.lock',H/'src/native_policy.rs',H/'src/bin'/f'{name}.rs',pathlib.Path(__file__)]}
for row in plan:
 for key in ['left','right','weights','baseline']:pins[row[key]]=sha(row[key])
data={'status':'building','producer_pid':os.getpid(),'source':str(S),'harness':str(H),'pins':pins,'plan':plan,'results':[],'scope':'Two fixed real CPU LoFTR controls with original640/max1m/radii2and6 policy. New compact diagnostics only; no heldout precision/general RSS bound/full458 qualification.'}
def save():P.write_text(json.dumps(data,indent=2)+'\n')
save();env=dict(os.environ,LIBTORCH_USE_PYTORCH='1');env['PATH']=str(runtime/'bin')+':'+env['PATH'];target=T/'rrrah-cache-interpretation-fix-20261012/target';cmd=['cargo','build','--locked','--offline','--bin',name,'--target-dir',str(target)];data['build_command']=cmd;save()
with (W/'build.log').open('x') as log:
 child=subprocess.Popen(cmd,cwd=H,env=env,stdout=log,stderr=subprocess.STDOUT);data['child_pid']=child.pid;save();code=child.wait()
data['build_returncode']=code;pins[str(W/'build.log')]=sha(W/'build.log');save()
if code: data['status']='terminal_build_failure';save();raise SystemExit(code)
binary=W/name;shutil.copy2(target/'debug'/name,binary);pins[str(binary)]=sha(binary);data['binary']=str(binary)
for row in plan:
 cmd=['/usr/bin/time','-l','/usr/bin/env','DYLD_LIBRARY_PATH='+str(runtime/'lib/python3.12/site-packages/torch/lib'),str(binary),row['left'],row['right'],row['weights'],'640',str(row['tolerance']),'1000000'];data.update(status='running',active_kind=row['kind'],command=cmd);save();started=time.monotonic();child=subprocess.Popen(cmd,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True);data['child_pid']=child.pid;save();out,err=child.communicate();case=dict(kind=row['kind'],command=cmd,returncode=child.returncode,stdout=out,stderr=err,elapsed_seconds=time.monotonic()-started);path=W/(row['kind']+'.json');path.write_text(json.dumps(case,indent=2)+'\n');data['results'].append(dict(kind=row['kind'],report=str(path),sha256=sha(path)));save()
 if child.returncode:break
data['status']='complete_pending_audit' if len(data['results'])==2 and all(json.loads(pathlib.Path(v['report']).read_text())['returncode']==0 for v in data['results']) else 'terminal_execution_failure';save();print(data['status'])
