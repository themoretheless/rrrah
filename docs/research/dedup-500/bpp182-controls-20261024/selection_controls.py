"""Original complete selection functions with precomputed synthetic hashes.

Logger and CLUSTER_UNASSIGNED=-1 are adapters; original date/hash functions.
Image-decoder hook raises if called; no decoding path qualified.
"""
import ast,datetime,json,logging
from pathlib import Path
b=Path(__file__).resolve().parent;e=b.parent/'evidence/Arkalogy--best-photo-picker';env={'datetime':datetime,'log':logging.getLogger('selection-controls'),'CLUSTER_UNASSIGNED':-1}
def funcs(p,names):
 t=ast.parse(p.read_text());nodes=[ast.ImportFrom(module='__future__',names=[ast.alias(name='annotations')],level=0)]+[x for x in t.body if isinstance(x,ast.FunctionDef) and x.name in names];exec(compile(ast.fix_missing_locations(ast.Module(body=nodes,type_ignores=[])),str(p),'exec'),env)
funcs(e/'05-phash.py',{'hamming_distance','dual_hash_distance'});funcs(e/'remaining-pinned/bpp/dedupe/common.py',{'parse_date','within_time_window'});funcs(e/'remaining-pinned/bpp/dedupe/cluster.py',{'_hash_distance','_global_dedup','deduplicate'})
def no_decode(*a):raise AssertionError('unexpected decoder invocation')
env['compute_hashes_from_file']=no_decode
items=[{'filepath':n,'phash':h,'ahash':None,'date':'2024-01-01T00:00:00','aggregate_score':score} for n,h,score in [('a',0,1),('b',1,9),('c',2,2)]]
got=env['deduplicate'](items,{'hash_distance_threshold':1,'global_hash_distance_threshold':0});assert len(got)==1 and got[0]['filepath']=='b' and got[0]['cluster_size']==3;dist=env['_hash_distance'](got[0],items[2]);assert dist==2
out=[{'case':'best_score_representative_changes_after_cluster_assignment','threshold':1,'kept':'b','reported_cluster_size':3,'distance_from_selected_to_member_c':dist}]
items=[{'filepath':'before','phash':0,'ahash':None,'date':'2024-01-31T23:59:55','date_month':'2024-01','aggregate_score':1},{'filepath':'after','phash':0,'ahash':None,'date':'2024-02-01T00:00:05','date_month':'2024-02','aggregate_score':2}];got=env['deduplicate'](items);assert [x['filepath'] for x in got]==['after'];out.append({'case':'first_pass_can_remove_an_entire_month_before_global_protection','kept_months':[x['date_month'] for x in got],'qualification':'Global helper protects months among its input only;not a demonstrated breach of helper contract.'})
p={'scope':__doc__,'authored_controls_passed':len(out),'observations':out};(b/'selection-results.json').write_text(json.dumps(p,indent=2)+'\n');print(json.dumps(p,indent=2))
