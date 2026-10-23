import json
from hypoarena.dedup import DedupConfig,DuplicateFinder,NoveltyGuard
results=[]
for minimum in (1,2,3):
 cfg=DedupConfig(method='tfidf',threshold=.9,min_document_frequency=minimum)
 guard=NoveltyGuard(cfg);guard.observe('a','alpha')
 results.append({'min_document_frequency':minimum,'finder_clusters':[c.as_dict() for c in DuplicateFinder(cfg).find({'a':'alpha','b':'beta'})],'guard_beta':guard.check('beta').as_dict()})
assert not results[0]['finder_clusters']
assert all(r['finder_clusters'] and r['guard_beta']['is_novel'] for r in results[1:])
print(json.dumps({'scope':'Actual unchanged pinned public APIs; disjoint single-word texts at three configured minimum frequencies. No universal equivalence contract assumed.','results':results},indent=2))
