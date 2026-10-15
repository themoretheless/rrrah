#!/usr/bin/env python3
from pathlib import Path
import hashlib,json,runpy,math,itertools,warnings,sys
import numpy as np
R=Path(__file__).resolve().parents[1];D=R/'docs/research/dedup-500';E=D/'evidence/idealo--imagededup/03-information_retrieval.py';sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();review=json.loads((D/'reviews/002.json').read_text());expected=next(v['sha256']for v in review['available_source_evidence']if v['local']==E.name);assert sha(E)==expected;m=runpy.run_path(str(E));ap=m['avg_prec'];ndcg=m['ndcg'];jac=m['jaccard_similarity'];checks=[]
for name in ['avg_prec','ndcg','jaccard_similarity']:
 for correct,retrieved,expect in [([],[],1.),([],['x'],0.),(['a'],[],0.),(['a'],['x'],0.),(['a'],['a'],1.)]:
  actual=float(m[name](correct,retrieved));assert math.isclose(actual,expect);checks.append(dict(metric=name,truth=correct,retrieved=retrieved,actual=actual,expected=expect))
for retrieved in itertools.permutations(['a','b','x']):
 truth=['a','b'];relevance=[int(x in truth)for x in retrieved];hits=0;precision=0.;gain=0.
 for rank,rel in enumerate(relevance,1):
  hits+=rel;precision+=rel*hits/rank;gain+=rel/math.log2(rank+1)
 expected_ap=precision/2;expected_ndcg=gain/(1+1/math.log2(3));assert math.isclose(float(ap(truth,retrieved)),expected_ap)and math.isclose(float(ndcg(truth,retrieved)),expected_ndcg)and math.isclose(float(jac(truth,retrieved)),2/3);checks.append(dict(metric='three_distinct_results',retrieved=retrieved,average_precision=expected_ap,ndcg=expected_ndcg,jaccard=2/3))
missing_truth=['a','b'];missing_result=['a','x'];actual_ndcg=float(ndcg(missing_truth,missing_result));standard_ndcg=1/(1+1/math.log2(3));assert actual_ndcg==1.0 and standard_ndcg<actual_ndcg
repeated_ap=float(ap(['a'],['a','a']));assert repeated_ap==2.0
assert jac(['a'],['a','a'])==1.0
exceptions={}
for label,args in [('default_none',({'a':['b']},{'a':['b']})),('missing_query',({'a':['b']},{},'map')),('unknown_metric',({'a':['b']},{'a':['b']},'bogus'))]:
 try:m['mean_metric'](*args)
 except (AttributeError,KeyError)as e:exceptions[label]=type(e).__name__
 else:raise AssertionError(label)
with warnings.catch_warnings(record=True)as caught:
 warnings.simplefilter('always');empty=float(m['mean_metric']({}, {},'map'))
assert math.isnan(empty);assert sha(E)==expected
O=D/'imagededup2-metrics-selected-runtime-v1-20261015.json';assert not O.exists();O.write_text(json.dumps(dict(status='verified_selected_unchanged_metrics_with_real_numpy',repository=review['repository'],commit=review['commit'],source=str(E.relative_to(R)),source_sha256=expected,producer_sha256=sha(__file__),python=sys.version,numpy_version=np.__version__,ordinary_oracle_checks=checks,negative_controls=dict(missing_relevant_in_retrieved_depth=dict(truth=missing_truth,retrieved=missing_result,observed_ndcg=actual_ndcg,standard_binary_ndcg_at_2=standard_ndcg,reason='Upstream ideal denominator sorts only retrieved relevance, so omitted relevant item does not contribute to idealgain'),repeated_identifier=dict(truth=['a'],retrieved=['a','a'],observed_average_precision=repeated_ap,observed_jaccard=1.0),exceptions=exceptions,empty_groundtruth_mean='NaN',empty_warnings=[str(w.message)for w in caught]),scope='Entire143line metricsmodule read and executed byte-for-byte using actual numpy, no adapters or dependency imports removed. Authored synthetic filename lists only. Not full repository review, upstream metrics test suite, visual quality/data corpus qualification, currentRoot pipeline metrics or production reuse.',implications=['Validate distinct result identifiers and complete query domains before external ranking evaluation','Use independent idealranking at intendeddepth when comparing missed-positive quality','Do not report empty-query NaN as success or use ranking metrics to assert pixel identity']),indent=2)+'\n');print(len(checks),'ordinary metricoracles; incomplete retrieval NDCG inflation and repeated-ID AP>1 reproduced')
