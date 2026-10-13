#!/usr/bin/env python3
"""Run precommitted positive controls serially on frozen baseline and candidate."""
import hashlib,json,re,subprocess,time
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
    pp=D/'dedup-loftr-position-positive-plan-20261012.json';plan=json.loads(pp.read_text());assert all(sha(p)==v for p,v in plan['pins'].items())
    cp=D/'dedup-loftr-position-floor-runtime-20261012.json';c=json.loads(cp.read_text());assert c['status']=='complete_candidate_not_adopted' and c['returncode']==0
    ca=D/'dedup-loftr-position-floor-complete-audit-20261012.json';assert json.loads(ca.read_text())['status']=='verified_complete_candidate_not_adopted'
    bp=D/'dedup-loftr-current-stack-runtime-20261012.json';b=json.loads(bp.read_text());assert all(sha(p)==v for p,v in b['pins'].items()) and all(sha(p)==v for p,v in c['pins'].items())
    out=D/'dedup-loftr-position-positive-runtime-20261012.json';assert not out.exists()
    r=dict(status='running',plan=plan['plan'],results=[],pins={str(p):sha(p) for p in [pp,cp,ca,bp,Path(__file__)]},scope='Two precommitted model-positive controls, unchanged complete current full caller and one-file position-floor candidate. Separate from historical all458 executable. One-shot RSS, no universal limits/platform/all500 completion.')
    def save():q=out.with_suffix('.tmp');q.write_text(json.dumps(r,indent=2)+'\n');q.replace(out)
    save()
    for i,item in enumerate(plan['plan']):
        for label,report in [('baseline',b),('candidate',c)]:
            cmd=report['command'].copy();cmd[5:7]=[item['left'],item['right']];cmd[9]=str(item['source_tolerance'])
            stdout=T/f'rrrah-loftr-position-positive-{i}-{label}-20261012.stdout';stderr=T/f'rrrah-loftr-position-positive-{i}-{label}-20261012.stderr';assert not stdout.exists() and not stderr.exists()
            with stdout.open('w') as o,stderr.open('w') as e:start=time.monotonic();child=subprocess.Popen(cmd,cwd='/tmp',stdout=o,stderr=e);r.update(active_pair=i,active_variant=label,child_pid=child.pid);save();rc=child.wait()
            row=dict(index=i,variant=label,command=cmd,returncode=rc,elapsed_seconds=time.monotonic()-start,stdout=str(stdout),stderr=str(stderr));r['pins'].update({str(p):sha(p) for p in [stdout,stderr]})
            if rc==0:
                evidence=json.loads(stdout.read_text());pairs=evidence['evidence']['pairs'];assert len(pairs)==1
                row.update(evidence=evidence,supported=pairs[0]['supported'],rss_bytes=int(re.search(r'(\d+)\s+maximum resident set size',stderr.read_text())[1]))
            r['results'].append(row);save()
            if rc:r['status']='terminal_execution_failure';save();return
    assert all(sha(p)==v for p,v in r['pins'].items())
    r['all_supported']=all(row['supported'] is True for row in r['results'])
    r['entire_evidence_identical']=all(r['results'][2*i]['evidence']==r['results'][2*i+1]['evidence'] for i in range(2))
    r['status']='complete_controls_not_adopted';save();print(r['status'],r['all_supported'],r['entire_evidence_identical'])
if __name__=='__main__':main()
