#!/usr/bin/env python3
"""Verify every precommitted positive execution, including explicit support outcomes."""
import hashlib,json,re
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
    rp=D/'dedup-loftr-position-positive-runtime-20261012.json';r=json.loads(rp.read_text());assert r['status']=='complete_controls_not_adopted'
    assert all(sha(p)==v for p,v in r['pins'].items())
    pp=D/'dedup-loftr-position-positive-plan-20261012.json';plan=json.loads(pp.read_text());assert all(sha(p)==v for p,v in plan['pins'].items());assert r['plan']==plan['plan'] and len(r['plan'])==2 and len(r['results'])==4
    report_paths={label:D/name for label,name in [('baseline','dedup-loftr-current-stack-runtime-20261012.json'),('candidate','dedup-loftr-position-floor-runtime-20261012.json')]};reports={label:json.loads(p.read_text()) for label,p in report_paths.items()}
    for label,report in reports.items():
        assert report['returncode']==report['build_returncode']==0 and all(sha(p)==v for p,v in report['pins'].items())
        hashes=report['root_source_hashes'] if label=='baseline' else report['source_hashes'];assert all(sha(Path(report['source'])/n)==v for n,v in hashes.items())
    rows=[]
    for i,item in enumerate(plan['plan']):
        evidence=[];rss=[]
        for j,label in enumerate(['baseline','candidate']):
            row=r['results'][2*i+j];assert row['index']==i and row['variant']==label and row['returncode']==0
            cmd=reports[label]['command'].copy();cmd[5:7]=[item['left'],item['right']];cmd[9]=str(item['source_tolerance']);assert cmd==row['command']
            value=json.loads(Path(row['stdout']).read_text());assert value==row['evidence'];assert value['managed_used']==0
            pairs=value['evidence']['pairs'];assert len(pairs)==1 and pairs[0]['supported']==row['supported'] and type(row['supported']) is bool
            measured=int(re.search(r'(\d+)\s+maximum resident set size',Path(row['stderr']).read_text())[1]);assert measured==row['rss_bytes'];evidence.append(value);rss.append(measured)
        rows.append(dict(index=i,baseline_index=item['baseline_index'],baseline_supported=r['results'][2*i]['supported'],candidate_supported=r['results'][2*i+1]['supported'],entire_evidence_identical=evidence[0]==evidence[1],baseline_rss_bytes=rss[0],candidate_rss_bytes=rss[1],rss_delta_bytes=rss[1]-rss[0]))
    supported=all(row['baseline_supported'] and row['candidate_supported'] for row in rows);same=all(row['entire_evidence_identical'] for row in rows)
    assert supported==r['all_supported'] and same==r['entire_evidence_identical']
    out=D/'dedup-loftr-position-positive-complete-audit-20261012.json';assert not out.exists()
    out.write_text(json.dumps(dict(status='verified_two_complete_positive_controls',all_supported=supported,entire_evidence_identical=same,rows=rows,pins={str(p):sha(p) for p in [rp,pp,*report_paths.values(),Path(__file__)]},scope='Two precommitted real positive pairs, original full current caller versus isolated candidate, every complete JSON field and support compared. Not all458, broad precision, universal RSS/latency, gradients, platforms or full500 qualification.'),indent=2)+'\n');print('Positive audit:',supported,same)
if __name__=='__main__':main()
