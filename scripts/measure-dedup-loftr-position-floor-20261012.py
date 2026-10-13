#!/usr/bin/env python3
"""Measure isolated position-floor candidate after current-stack baseline finishes."""
import hashlib,json,os,re,shutil,subprocess,time
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
    bp=D/'dedup-loftr-current-stack-runtime-20261012.json';b=json.loads(bp.read_text())
    assert b['status'] in ['complete_identical_evidence_current_sources','complete_evidence_changed_current_sources'] and b['returncode']==0
    audit=D/'dedup-loftr-current-stack-complete-audit-20261012.json'
    assert json.loads(audit.read_text())['status']=='verified_complete_current_source_fixed_pair'
    units=T/'rrrah-loftr-position-floor-units-20261012.log'
    assert 'test result: ok. 57 passed; 0 failed; 0 ignored;' in units.read_text()
    source=T/'rrrah-loftr-position-floor-source-20261012';h=T/'rrrah-loftr-position-floor-harness-20261012';out=D/'dedup-loftr-position-floor-runtime-20261012.json'
    assert not any(p.exists() for p in [source,h,out])
    assert all(sha(Path(b['source'])/n)==v for n,v in b['root_source_hashes'].items())
    shutil.copytree(b['source'],source)
    p=source/'vendor/loftr/src/loftr_model.rs';code=p.read_text();assert code.count('(256, 256),')==1;p.write_text(code.replace('(256, 256),','(1, 1),'))
    qualified=T/'rrrah-loftr-position-floor-units-20261012/src/loftr_model.rs';assert sha(p)==sha(qualified)
    hashes={str(p.relative_to(source)):sha(p) for p in source.rglob('*') if p.is_file()}
    assert [n for n,v in hashes.items() if v!=b['root_source_hashes'][n]]==['vendor/loftr/src/loftr_model.rs']
    old=T/'rrrah-loftr-current-stack-harness-20261012';h.mkdir();(h/'Cargo.toml').write_text((old/'Cargo.toml').read_text().replace(b['source'],str(source)));shutil.copy2(old/'Cargo.lock',h/'Cargo.lock');shutil.copytree(old/'src',h/'src')
    r=dict(status='building',producer_pid=os.getpid(),source=str(source),source_hashes=hashes,pins={str(p):sha(p) for p in [bp,audit,units,qualified,Path(__file__),*h.rglob('*')] if p.is_file()},scope='Isolated one-file candidate against frozen current-stack fixed full caller. Not adopted; exact whole evidence and whole process RSS required. No universal memory, corpus or platform qualification.')
    def save():q=out.with_suffix('.tmp');q.write_text(json.dumps(r,indent=2)+'\n');q.replace(out)
    save();runtime=T/'rrrah-loftr-native-runtime-20261010';env=dict(os.environ,LIBTORCH_USE_PYTORCH='1');env['PATH']=str(runtime/'bin')+':'+env['PATH'];target=T/'rrrah-dedup-exact-portfolio-20261011';log=T/'rrrah-loftr-position-floor-probe-build-20261012.log'
    cmd=['cargo','build','--manifest-path',str(h/'Cargo.toml'),'--bin','disc21_heldout_negative_collection','--locked','--offline','--target-dir',str(target)]
    with log.open('w') as f:child=subprocess.Popen(cmd,cwd='/tmp',env=env,stdout=f,stderr=subprocess.STDOUT);r['build_pid']=child.pid;save();rc=child.wait()
    r.update(build_returncode=rc);r['pins'][str(log)]=sha(log)
    if rc:r['status']='terminal_build_failure';save();return
    exe=T/'rrrah-loftr-position-floor-executable-20261012';assert not exe.exists();shutil.copy2(target/'debug/disc21_heldout_negative_collection',exe);r['pins'][str(exe)]=sha(exe)
    cmd=b['command'].copy();cmd[4]=str(exe);stdout=T/'rrrah-loftr-position-floor-20261012.stdout';stderr=T/'rrrah-loftr-position-floor-20261012.stderr';r.update(command=cmd,status='running_complete_probe',stdout=str(stdout),stderr=str(stderr))
    with stdout.open('w') as o,stderr.open('w') as e:start=time.monotonic();child=subprocess.Popen(cmd,cwd='/tmp',stdout=o,stderr=e);r['child_pid']=child.pid;save();rc=child.wait()
    r.update(returncode=rc,elapsed_seconds=time.monotonic()-start);r['pins'].update({str(p):sha(p) for p in [stdout,stderr]})
    assert all(sha(source/n)==v for n,v in hashes.items()) and all(sha(p)==v for p,v in r['pins'].items())
    if rc:r['status']='terminal_probe_failure';save();return
    evidence=json.loads(stdout.read_text());rss=int(re.search(r'(\d+)\s+maximum resident set size',stderr.read_text())[1]);same=evidence==b['evidence'];r.update(status='complete_candidate_not_adopted',evidence=evidence,evidence_identical=same,rss_bytes=rss,rss_delta_vs_current_stack=rss-b['rss_bytes']);save();print(r['status'],same,rss,r['rss_delta_vs_current_stack'])
if __name__=='__main__':main()
