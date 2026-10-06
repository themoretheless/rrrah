#!/usr/bin/env python3
"""Compare paired recorded outcomes without treating correspondence labels as duplicates."""
import argparse, collections, json, pathlib
p=argparse.ArgumentParser()
p.add_argument('baseline',type=pathlib.Path);p.add_argument('alternative',type=pathlib.Path);p.add_argument('output',type=pathlib.Path)
a=p.parse_args();base=json.loads(a.baseline.read_text());other=json.loads(a.alternative.read_text())
assert base['manifest_sha256']==other['manifest_sha256']
assert base['required_pairs']==other['required_pairs']==580

def records(report):
    rows={}
    for row in report['results']:
        key=(row['sequence'],row['target_index'])
        assert key not in rows, 'duplicate pair'
        rows[key]=row
    assert len(rows)==report['completed_pairs']
    return rows

left,right=records(base),records(other);shared=sorted(left.keys() & right.keys())
counts=collections.Counter();changes=[]
def execution_outcome(row):
    if row['status']=='ok':return 'ok'
    if row['status'] in ('timeout','process_error'):return row['status']
    diagnostic=row.get('stderr','')
    if 'Pixels(Budget)' in diagnostic:return 'pixel_budget_refusal'
    if 'Pixels(Invalid)' in diagnostic:return 'invalid_pixel_geometry_refusal'
    return 'execution_error'
execution_transitions=collections.Counter()
for key in shared:
    x,y=left[key],right[key]
    execution_transitions[execution_outcome(x)+' -> '+execution_outcome(y)]+=1
    if x['status']!='ok' or y['status']!='ok':
        counts['execution_failure_pairs']+=1;changes.append({'sequence':key[0],'target_index':key[1],'baseline_status':x['status'],'alternative_status':y['status'],'baseline_execution_outcome':execution_outcome(x),'alternative_execution_outcome':execution_outcome(y)});continue
    x,y=x['evidence'],y['evidence']
    # Compare native evidence as well as decisions; resource-only changes should
    # not silently alter registration, lane verification or memory accounting.
    differing_fields=sorted(k for k in x.keys() | y.keys() if x.get(k)!=y.get(k))
    counts['native_evidence_equal_pairs']+=not differing_fields
    counts['native_evidence_changed_pairs']+=bool(differing_fields)
    if differing_fields:
        changes.append({'sequence':key[0],'target_index':key[1],'native_evidence_changed_fields':differing_fields})
    counts['successful_pairs']+=1
    for label,e in [('baseline',x),('alternative',y)]:
        counts[label+'_geometry']+=e['geometry'] is not None
        counts[label+'_candidates']+=e['candidate']
    if x['candidate']!=y['candidate'] or (x['geometry'] is None)!=(y['geometry'] is None):
        changes.append({'sequence':key[0],'target_index':key[1],'baseline_candidate':x['candidate'],'alternative_candidate':y['candidate'],'baseline_correspondences':x['correspondences'],'alternative_correspondences':y['correspondences'],'baseline_inliers':x['inliers'],'alternative_inliers':y['inliers']})
    counts['candidate_gains']+=not x['candidate'] and y['candidate']
    counts['candidate_losses']+=x['candidate'] and not y['candidate']
result={'manifest_sha256':base['manifest_sha256'],'baseline_probe_sha256':base['probe_sha256'],'alternative_probe_sha256':other['probe_sha256'],'required_pairs':580,'paired_records':len(shared),'complete':len(shared)==580,'counts':dict(counts),'changes':changes,'scope':'Paired geometric correspondence diagnostics; candidate gains are not duplicate recall, losses are not false-negative labels, and this corpus has no unrelated-pair precision labels'}
result['execution_transitions']=dict(execution_transitions)
a.output.write_text(json.dumps(result,indent=2)+'\n');print({k:v for k,v in result.items() if k not in ('changes','scope')})
