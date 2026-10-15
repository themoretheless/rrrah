#!/usr/bin/env python3
from pathlib import Path
import json,hashlib,subprocess,os,re
R=Path(__file__).resolve().parents[1];D=R/'docs/research';P=D/'dedup-container-cancel-erasure-v1-prepared-20261015.json';p=json.loads(P.read_text());F=Path(p['source']);M=Path(p['baseline_manifest']);m=json.loads(M.read_text());sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();expected={r['path']:r['sha256']for r in m['source_files']};expected.update({r['path']:r['candidate_sha256']for r in p['changed']})
assert sha(M)==p['baseline_manifest_sha256']
for name,h in expected.items():assert sha(F/name)==h
T=Path('/private/tmp/rrrah-consensus-collection-target-20261014/debug');deps=T/'deps';known={'blake3':'919ad277b78b89ac','errno':'498bb0922c144057','libc':'b019f3744f8ef3b1','libproc':'3a75726e17f60735','rrrah_core':'350525ae69d3249f','rrrah_decode':'3affc33d380d9f4c','rustix':'3cf27bad75aace96','tempfile':'c83029f54bb75712','thiserror':'8b0acd18862397b0'};pins={str(deps/f'lib{k}-{v}.rlib'):sha(deps/f'lib{k}-{v}.rlib')for k,v in known.items()};build=max((T/'build').glob('rrrah-dedup-*/output'),key=lambda p:p.stat().st_mtime);recipe=re.search(r'cargo:rustc-env=RRRAH_BUILT_RECIPE_ID=(\w+)',build.read_text())[1];Q=Path('/private/tmp/rrrah-container-cancel-erasure-metadata-v1-20261015');Q.mkdir(exist_ok=False);O=D/'dedup-container-cancel-erasure-metadata-v1-20261015.json';assert not O.exists();cmd=['rustc','+stable','--edition=2024','--crate-name','rrrah_dedup','--test','--emit=metadata','-C','opt-level=0','--cfg','feature="decode"','--cfg','feature="raster"','--cfg','feature="default"','-L','dependency='+str(deps),'--warn=missing_debug_implementations','crates/rrrah-dedup/src/lib.rs','-o',str(Q/'tests.rmeta')]
for k,v in known.items():cmd+=['--extern',k+'='+str(deps/f'lib{k}-{v}.rlib')]
env=os.environ.copy();env.update(CARGO_MANIFEST_DIR=str(F/'crates/rrrah-dedup'),RRRAH_BUILT_RECIPE_ID=recipe);d=dict(status='running',producer_pid=os.getpid(),producer_sha256=sha(__file__),prepared_manifest=str(P),prepared_manifest_sha256=sha(P),dependency_pins=pins,command=cmd,cached_recipe_output=str(build),cached_recipe_output_sha256=sha(build),recipe=recipe,compiler=subprocess.check_output(['rustc','+stable','-Vv'],text=True),scope='Whole experimental library cfgtest metadata typecheck including original complete library unit-test sources, with private containercallback erasure; actual cached baseline core/decoder dependencies. No linking/test execution/performance/fullCargo dependency resolution/Root adoption. Cached recipe string reused solely to typecheck envmacro, not candidate cache identity qualification.')
def save():
 t=O.with_suffix('.tmp');t.write_text(json.dumps(d,indent=2)+'\n');t.replace(O)
log=Q/'compile.log'
with log.open('x')as out:
 child=subprocess.Popen(cmd,cwd=F,env=env,stdout=out,stderr=subprocess.STDOUT);d['child_pid']=child.pid;save();rc=child.wait()
for name,h in expected.items():assert sha(F/name)==h
assert all(sha(k)==h for k,h in pins.items())
d.update(status='metadata_passed_not_runtime_qualified'if rc==0 else'terminal_metadata_failure',returncode=rc,log=str(log),log_sha256=sha(log));save();print('Whole candidate library cfgtest metadata returncode',rc);raise SystemExit(rc)
