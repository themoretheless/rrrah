import ast,csv,importlib.util,io,json,tempfile
from pathlib import Path
root=Path('.dedup-work/examples088-20261024');out={}
def load(path):
 spec=importlib.util.spec_from_file_location('owned_example_'+path.replace('/','_'),root/path);mod=importlib.util.module_from_spec(spec);spec.loader.exec_module(mod);return mod
csvmod=load('csv_example/csv_example.py');link=load('record_linkage_example/record_linkage_example.py');gaz=load('gazetteer_example/gazetteer_example.py');pat=load('patent_example/patent_example.py')
with tempfile.TemporaryDirectory(prefix='rrrah-088-data-') as td:
 p=Path(td)/'duplicate-ids.csv'
 with p.open('w') as f:
  w=csv.writer(f);w.writerow(['Id','Site name']);w.writerows([['7','first'],['7','second']])
 parsed=csvmod.readData(str(p));assert len(parsed)==1 and parsed[7]['Site name']=='second';out['csv_duplicate_ids']={'input_rows':2,'retained_rows':len(parsed),'retained_name':parsed[7]['Site name']}
 out['price_cases']=[]
 for mod in [link,gaz]:
  for price in ['$12.50','12.50','-$12.50','€12.50','']:
   with p.open('w') as f:
    w=csv.writer(f);w.writerow(['title','price']);w.writerow(['product',price])
   try:value=next(iter(mod.readData(str(p)).values()))['price'];result={'value':value}
   except Exception as e:result={'error':type(e).__name__}
   out['price_cases'].append({'module':mod.__file__.split('/')[-1],'input':price,**result})
 # Original reader against all four pinned product CSVs, no model training.
 out['original_product_readers']=[]
 for mod,paths in [(link,['record_linkage_example/AbtBuy_Abt.csv','record_linkage_example/AbtBuy_Buy.csv']),(gaz,['gazetteer_example/data/AbtBuy_Abt.csv','gazetteer_example/data/AbtBuy_Buy.csv'])]:
  for path in paths:
   records=mod.readData(str(root/path));out['original_product_readers'].append({'path':path,'records':len(records)})
 records=pat.readData(str(root/'patent_example/patstat_input.csv'));out['original_patent_reader']={'records':len(records)}
 # Execute unchanged top-level result-to-membership statements only, with owned search-result fixtures.
 tree=ast.parse((root/'gazetteer_example/gazetteer_example.py').read_text());main=next(n for n in tree.body if isinstance(n,ast.If));start=next(i for i,n in enumerate(main.body) if isinstance(n,ast.Assign) and any(isinstance(t,ast.Name) and t.id=='cluster_membership' for t in n.targets));nodes=main.body[start:start+3]
 assert isinstance(nodes[2],ast.For)
 out['gazetteer_output']=[]
 for fixture in [[('m0',(('c0',.9),)),('m1',(('c0',.95),))],[('m0',(('c0',.9),('c1',.8))),('m1',(('c0',.95),))]]:
  ns={'results':fixture};exec(compile(ast.Module(body=nodes,type_ignores=[]),'original-gazetteer-output','exec'),ns)
  membership=ns['cluster_membership'];edges={(m,c) for m,matches in fixture for c,_ in matches};rendered={(m,c) for m in ['m0','m1'] for c in ['c0','c1'] if m in membership and c in membership and membership[m]['Cluster ID']==membership[c]['Cluster ID']}
  out['gazetteer_output'].append({'input_edges':[list(e) for e in sorted(edges)],'serialized_membership':membership,'reconstructed_edges':[list(e) for e in sorted(rendered)],'lost_edges':[list(e) for e in sorted(edges-rendered)],'invented_edges':[list(e) for e in sorted(rendered-edges)]})
 assert out['gazetteer_output'][0]['lost_edges']==[['m0','c0']]
 assert out['gazetteer_output'][1]['invented_edges']==[['m1','c1']]
out['scope']='Original guarded modules imported unchanged for readers; original membership AST statements with synthetic results. No learned matching quality, database, or full example execution.'
print(json.dumps(out,ensure_ascii=False,indent=2))
