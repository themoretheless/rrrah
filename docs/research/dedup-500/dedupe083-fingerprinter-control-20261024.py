"""Whole original stdlib-only blocking module with explicit synthetic predicates."""
import importlib.util,json,pathlib
root=pathlib.Path(__file__).resolve().parents[3]
p=root/'.dedup-work/dedupe083-20261024/dedupe/blocking.py'
spec=importlib.util.spec_from_file_location('owned_dedupe_blocking',p)
m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
class Predicate:
 def __init__(self,field):self.field=field
 def __iter__(self):return iter([self])
 def __call__(self,record,target=False):return frozenset([record[self.field]])
f=m.Fingerprinter([Predicate('first'),Predicate('second')])
records={1:{'first':'same','second':'other'},2:{'first':'different','second':'same'}}
f.index_all(records)
actual=list(f(records.items()))
assert actual==[('same:0',1),('other:1',1),('different:0',2),('same:1',2)]
assert not ({k for k,i in actual if i==1}&{k for k,i in actual if i==2})
f.reset_indices()
assert list(f(records.items(),target=True))==actual
print(json.dumps({'whole_original_module_imported':True,'predicate_namespaces_separate_same_text':True,'target_flag_forwarding_baseline':True,'scope':'Two synthetic non-index predicates, two records; original index predicates and learned model not qualified.'},indent=2))
