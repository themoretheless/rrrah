"""Full installed clustering with original fixture and complete independent assertions."""
import importlib.util,json,pathlib
import numpy as np
import dedupe.clustering as c
root=pathlib.Path(__file__).resolve().parents[3]
p=root/'.dedup-work/dedupe083-20261024/tests/test_dedupe.py'
s=importlib.util.spec_from_file_location('owned_upstream_cluster_tests',p);m=importlib.util.module_from_spec(s);s.loader.exec_module(m)
t=m.ClusteringTest();t.setUp()
assert t.clusterEquals([((1,2),(.9,.9)),((3,4),(.8,.8))],[((1,2),(.9,.9)),((5,6),(.1,.1))]) is True
expected={1:[],0.5:[((1,2,3),(.778,.860,.778)),((4,5),(.720,.720)),((10,11),(.9,.9))],0:[((1,2,3,4,5),(.526,.564,.542,.320,.623)),((10,11),(.9,.9))]}
for threshold,want in expected.items():
 got=list(c.cluster(t.dupes.copy(),threshold))
 assert len(got)==len(want)
 for (ids,scores),(wanted_ids,wanted_scores) in zip(got,want):
  assert tuple(ids)==wanted_ids
  np.testing.assert_allclose(scores,wanted_scores,atol=.001,rtol=0)
empty=np.array([],dtype=[('pairs','i4',2),('score','f4')])
try:list(c.cluster(empty))
except RuntimeError as ex:
 assert 'generator raised StopIteration' in str(ex);empty_error=str(ex)
else:raise AssertionError('expected observed empty input failure')
print(json.dumps({'full_cluster_threshold_cases_passed':3,'all_cluster_members_and_scores_checked':True,'original_helper_accepts_corrupted_second_cluster':True,'empty_cluster_input_error':empty_error,'scope':'Installed pinned source, original finite fixture, full NumPy/SciPy implementation; not universal clustering or performance proof.'},indent=2))
