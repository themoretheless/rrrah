#!/usr/bin/env python3
import json,hashlib,math
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
P=D/'dedup-sscd-oracle-v2-plan-20261013.json';O=D/'dedup-sscd-oracle-v2-20261013.json';plan=json.loads(P.read_text());data=json.loads(O.read_text());assert data['status']=='complete_fixed_oracle' and data['completed_images']==data['required_images']==414 and data['inputs_unchanged'] and data['model_unchanged'] and data['source_unchanged'] and sha(P)==data['plan_sha256'];assert sha(R/'scripts/run-dedup-sscd-oracle-v2-20261013.py')==plan['producer_sha256']
A=Path(plan['acquisition']);a=json.loads(A.read_text());assert sha(A)==plan['acquisition_sha256'] and sha(a['weights'])==a['weights_sha256'];S=Path(a['source']);assert all(sha(S/p)==h for p,h in a['source_sha256'].items()) and all(sha(p)==h for p,h in plan['pins'].items())
vectors={};norms={};orientation_nonidentity=[]
for expected,r in zip(plan['images'],data['embeddings']):
 assert expected['path']==r['path'] and sha(r['report'])==r['sha256'];v=json.loads(Path(r['report']).read_text());assert v['input_sha256']==expected['sha256'];values=v['embedding'];assert len(values)==512 and all(math.isfinite(x) for x in values);norm=math.sqrt(math.fsum(x*x for x in values));assert abs(norm-1)<1e-5 and math.isclose(norm,v['norm'],rel_tol=1e-12,abs_tol=1e-12);vectors[r['path']]=values;norms[r['path']]=norm
 if v['exif_orientation']!=1:orientation_nonidentity.append(dict(path=r['path'],orientation=v['exif_orientation']))
def dot(a,b):return math.fsum(x*y for x,y in zip(vectors[a],vectors[b]))
summary={}
for c,r in zip(plan['pairs'],data['rows']):
 assert all(c[k]==r[k] for k in c);value=dot(c['left'],c['right']);distance=math.fsum((x-y)**2 for x,y in zip(vectors[c['left']],vectors[c['right']]));assert math.isclose(value,r['cosine'],rel_tol=1e-12,abs_tol=1e-12) and math.isclose(distance,r['squared_l2'],rel_tol=1e-12,abs_tol=1e-12);assert r['above_published_example_0_75']==(value>0.75);assert -1.000001<=value<=1.000001
 summary.setdefault(c['label'],[]).append(value)
assert len(data['rows'])==484
cross=[];nearest_wrong=[];ranks=[]
for c,reported in zip(plan['unique_positive_pairs'],data['rankings']):
 query,original=c['query'],c['original'];scores=sorted([(dot(query,p),p) for p in plan['originals']],key=lambda v:(-v[0],v[1]));rank=next(i+1 for i,(_,p) in enumerate(scores) if p==original);assert rank==reported['rank'] and scores[0][1]==reported['top1'];ranks.append(rank)
 for score,p in scores:cross.append(dict(query=query,reference=p,origin=original,publisher_origin_match=p==original,cosine=score))
 score,p=next((score,p) for score,p in scores if p!=original);nearest_wrong.append(dict(query=query,reference=p,origin=original,positive_cosine=dot(query,original),wrong_cosine=score,positive_rank=rank,positive_margin=dot(query,original)-score))
assert len(ranks)==229 and len(cross)==229*157 and sum(r['publisher_origin_match'] for r in cross)==229
original_score_summary={label:dict(directions=len(values),minimum=min(values),maximum=max(values),above_published_example_0_75=sum(x>0.75 for x in values)) for label,values in summary.items()}
wrong=[r['cosine'] for r in cross if not r['publisher_origin_match']];assert len(wrong)==35724
matrix=D/'dedup-sscd-copydays-cross-scores-20261013.json';assert not matrix.exists();matrix.write_text(json.dumps(dict(status='derived_from_fixed_pinned_embeddings',pairs=len(cross),positive_pairs=229,negative_pairs=len(wrong),rows=cross,scope='Publisher retrieval groundtruth: only same first-four-digit origin prefix is positive. Storedpixel square320 oracle,157originals, no external distractors or threshold calibration; not native cascade decisions.'),indent=2)+'\n')
nearest=D/'dedup-sscd-copydays-nearest-wrong-20261013.json';assert not nearest.exists();nearest.write_text(json.dumps(dict(status='fixed_diagnostic_hard_negative_inventory_not_native_results',pairs=len(nearest_wrong),rows=nearest_wrong),indent=2)+'\n')
out=D/'dedup-sscd-oracle-v2-audit-20261013.json';assert not out.exists();result=dict(status='verified414_image_sscd_oracle_484_pairs_and35953_copydays_cross_scores',producer_sha256=sha(O),plan_sha256=sha(P),acquisition_sha256=sha(A),weights_sha256=a['weights_sha256'],embeddings=414,dimensions=512,norm_minimum=min(norms.values()),norm_maximum=max(norms.values()),nonidentity_exif=orientation_nonidentity,summary=original_score_summary,retrieval_originals=157,query_strong=229,top1=sum(r==1 for r in ranks),mean_reciprocal_rank=math.fsum(1/r for r in ranks)/229,published_example_0_75_cross_negative_count=sum(x>0.75 for x in wrong),cross_negative_maximum=max(wrong),cross_scores=str(matrix),cross_scores_sha256=sha(matrix),nearest_wrong=str(nearest),nearest_wrong_sha256=sha(nearest),elapsed_seconds=data['elapsed_seconds'],scope=plan['scope']+' All414 vector norms and484pair scores checked with independent scalar math.fsum; all229 rankings replayed and complete35953cross-score table saved. Quality is not separated by a demonstrated global threshold. The model and Python/Pillow decoder have not been integrated into native library.');out.write_text(json.dumps(result,indent=2)+'\n');print(json.dumps({k:v for k,v in result.items() if k in ['status','top1','mean_reciprocal_rank','summary','cross_negative_maximum','published_example_0_75_cross_negative_count']},indent=2))
