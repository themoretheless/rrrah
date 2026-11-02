"""Original AST global semantic merge/none strategy; actual NumPy, synthetic inputs.

No embedding model, cosine-helper adapter, database or image decoding involved.
Logger-only adapter; preformed clusters are explicit synthetic caller inputs.
"""
import ast,json,logging
from pathlib import Path
import numpy as np
b=Path(__file__).resolve().parent;e=b.parent/'evidence/Arkalogy--best-photo-picker';env={'np':np,'log':logging.getLogger('merge-controls'),'os':__import__('os')}
def functions(p,names):
 t=ast.parse(p.read_text());nodes=[ast.ImportFrom(module='__future__',names=[ast.alias(name='annotations')],level=0)]+[x for x in t.body if isinstance(x,ast.FunctionDef) and x.name in names];exec(compile(ast.fix_missing_locations(ast.Module(body=nodes,type_ignores=[])),str(p),'exec'),env)
functions(e/'remaining-pinned/bpp/dedupe/semantic.py',{'_global_semantic_dedup'});functions(e/'remaining-pinned/bpp/dedupe/strategy.py',{'_none_strategy'})
items=[{'filepath':'a','aggregate_score':9,'cluster_size':3,'similar_photos':[{'filepath':'a1'},{'filepath':'a2'}]},{'filepath':'b','aggregate_score':8,'cluster_size':2,'similar_photos':[{'filepath':'b1'}]}];emb={'a':np.array([1.,0.]),'b':np.array([1.,0.])};got=env['_global_semantic_dedup'](items,emb,0.95);assert len(got)==1;row=got[0];assert row['cluster_size']==4 and [x['filepath'] for x in row['similar_photos']]==['a1','a2','b'];out=[{'case':'global_merge_of_preformed_clusters_loses_nested_membership','input_total_members':5,'observed_cluster_size':row['cluster_size'],'observed_siblings':[x['filepath'] for x in row['similar_photos']],'missing':'b1'}]
rows=[{'filepath':'old','cluster_size':2,'similar_photos':[{'filepath':'sibling'}]}];got=env['_none_strategy'](rows,{});assert got[0]['cluster_size']==1 and got[0]['similar_photos']==[{'filepath':'sibling'}];out.append({'case':'none_strategy_resets_count_but_preserves_previous_similar_list','cluster_size':got[0]['cluster_size'],'similar_photos':got[0]['similar_photos'],'input_dict_mutated':got[0] is rows[0]})
rows=[{'filepath':'a','aggregate_score':2},{'filepath':'b','aggregate_score':1}];emb={'a':np.array([2.,0.]),'b':np.array([0.5,np.sqrt(0.75)])};got=env['_global_semantic_dedup'](rows,emb,0.95);assert len(got)==1;out.append({'case':'unnormalized_embedding_boundary','dot_product':float(emb['a']@emb['b']),'independent_cosine':float((emb['a']@emb['b'])/(np.linalg.norm(emb['a'])*np.linalg.norm(emb['b']))),'retained':len(got),'qualification':'Production upstream may guarantee normalized embeddings;not a normal-input defect or model accuracy claim.'})
p={'scope':__doc__,'authored_controls_passed':len(out),'observations':out,'numpy':np.__version__,'limits':'No semantic_deduplicate end-to-end/original model helper,actual embeddings,realphotos/browser/performance. Preformed-cluster helper boundary qualified only.'};(b/'semantic-merge-results.json').write_text(json.dumps(p,indent=2)+'\n');print(json.dumps(p,indent=2))
