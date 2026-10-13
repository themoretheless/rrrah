#!/usr/bin/env python3
"""Freeze current local path sources and run the unchanged full caller probe."""
import hashlib,json,os,re,shutil,subprocess,time
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
    adoption=D/'dedup-loftr-sequential-backbone-adoption-20261012.json';a=json.loads(adoption.read_text());assert all(sha(R/'vendor/loftr/src'/n)==h for n,h in a['after'].items())
    source=T/'rrrah-loftr-current-stack-source-20261012';h=T/'rrrah-loftr-current-stack-harness-20261012';out=D/'dedup-loftr-current-stack-runtime-20261012.json';assert not any(p.exists() for p in [source,h,out]);source.mkdir()
    inputs=[]
    for root in [R/'crates',R/'vendor']:
        for parent,dirs,files in os.walk(root):
            dirs[:]=[d for d in dirs if d not in ['target','.git','node_modules','__pycache__']]
            for name in files:
                p=Path(parent)/name
                if not p.is_symlink():inputs.append(p)
    for name in ['Cargo.toml','Cargo.lock','rust-toolchain.toml','rustfmt.toml']:inputs.append(R/name)
    before={str(p.relative_to(R)):sha(p) for p in inputs}
    for p in inputs:q=source/p.relative_to(R);q.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(p,q)
    assert all(sha(R/n)==sha(source/n)==v for n,v in before.items()),'Current source changed while freezing; preserve and inspect.'
    old=T/'rrrah-disc21-heldout-negative-harness-20261012';old_source=T/'rrrah-disc21-heldout-negative-source-20261012';h.mkdir();(h/'Cargo.toml').write_text((old/'Cargo.toml').read_text().replace(str(old_source),str(source)));shutil.copy2(old/'Cargo.lock',h/'Cargo.lock');probe=h/'src/bin/disc21_heldout_negative_collection.rs';probe.parent.mkdir(parents=True);shutil.copy2(old/'src/bin/disc21_heldout_negative_collection.rs',probe)
    r=dict(status='building',producer_pid=os.getpid(),source=str(source),root_source_hashes=before,pins={str(p):sha(p) for p in [adoption,Path(__file__),h/'Cargo.toml',h/'Cargo.lock',probe]},scope='Full unchanged fixed worst-pair bidirectional caller on frozen current crates/vendor local sources, including adopted sequential backbone. Baseline probe manifest/lock/runtime/features/build configuration retained. Not all root features, current whole app, full458/all500, broad precision, platforms or hard memory cap. Missing/changed dependency compile failures must remain failures.')
    def save():q=out.with_suffix('.tmp');q.write_text(json.dumps(r,indent=2)+'\n');q.replace(out)
    save();runtime=T/'rrrah-loftr-native-runtime-20261010';env=dict(os.environ,LIBTORCH_USE_PYTORCH='1');env['PATH']=str(runtime/'bin')+':'+env['PATH'];target=T/'rrrah-dedup-exact-portfolio-20261011';log=T/'rrrah-loftr-current-stack-build-20261012.log';cmd=['cargo','build','--manifest-path',str(h/'Cargo.toml'),'--bin','disc21_heldout_negative_collection','--locked','--offline','--target-dir',str(target)];r['build_command']=cmd
    with log.open('w') as f:child=subprocess.Popen(cmd,cwd='/tmp',env=env,stdout=f,stderr=subprocess.STDOUT);r['build_pid']=child.pid;save();rc=child.wait()
    r.update(build_returncode=rc,build_log=str(log));r['pins'][str(log)]=sha(log)
    if rc:r['status']='terminal_build_failure';save();return
    exe=T/'rrrah-loftr-current-stack-executable-20261012';assert not exe.exists();shutil.copy2(target/'debug/disc21_heldout_negative_collection',exe);r['pins'][str(exe)]=sha(exe)
    bp=D/'dedup-disc21-heldout-negatives-runtime-20261012.json';b=json.loads(bp.read_text())['results'][7];r['pins'][str(bp)]=sha(bp);cmd=b['command'].copy();cmd[4]=str(exe);stdout=T/'rrrah-loftr-current-stack-20261012.stdout';stderr=T/'rrrah-loftr-current-stack-20261012.stderr';r.update(command=cmd,stdout=str(stdout),stderr=str(stderr),status='running_complete_probe')
    with stdout.open('w') as o,stderr.open('w') as e:start=time.monotonic();child=subprocess.Popen(cmd,cwd='/tmp',stdout=o,stderr=e);r['child_pid']=child.pid;save();rc=child.wait()
    r.update(returncode=rc,elapsed_seconds=time.monotonic()-start);r['pins'].update({str(p):sha(p) for p in [stdout,stderr]})
    assert all(sha(source/n)==v for n,v in before.items());assert all(sha(p)==v for p,v in r['pins'].items())
    if rc:r['status']='terminal_probe_failure';save();return
    evidence=json.loads(stdout.read_text());same=evidence==b['evidence'];r.update(status='complete_identical_evidence_current_sources' if same else 'complete_evidence_changed_current_sources',evidence=evidence,evidence_identical=same,rss_bytes=int(re.search(r'(\d+)\s+maximum resident set size',stderr.read_text())[1]));save();print(r['status'],r['rss_bytes'])
if __name__=='__main__':main()
