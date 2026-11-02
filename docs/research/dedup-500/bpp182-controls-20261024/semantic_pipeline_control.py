"""Complete original semantic pipeline functions + original cosine helper.

Actual NumPy, normalized synthetic embeddings;logger-only adapter. No model,
image decoding, real corpus, database or CLI. cosine source function only read.
"""
import ast,datetime,json,logging,os
from pathlib import Path
import numpy as np
b=Path(__file__).resolve().parent;e=b.parent/'evidence/Arkalogy--best-photo-picker';env={'np':np,'datetime':datetime,'os':os,'log':logging.getLogger('semantic-pipeline')}
for source,names in [(e/'remaining-pinned/bpp/scoring/clip_embed.py',{'cosine_similarity'}),(e/'remaining-pinned/bpp/dedupe/semantic.py',{'cluster_time_windowed','semantic_deduplicate','_global_semantic_dedup'})]:
 t=ast.parse(source.read_text());nodes=[ast.ImportFrom(module='__future__',names=[ast.alias(name='annotations')],level=0)]+[x for x in t.body if isinstance(x,ast.FunctionDef) and x.name in names];exec(compile(ast.fix_missing_locations(ast.Module(body=nodes,type_ignores=[])),str(source),'exec'),env)
rows=[{'id':i,'filepath':fp,'aggregate_score':score,'date':date} for i,(fp,score,date) in enumerate([('a',9,'2024-01-01T00:00:00'),('a1',3,'2024-01-01T00:00:10'),('a2',2,'2024-01-01T00:00:20'),('b',8,'2024-01-02T00:00:00'),('b1',1,'2024-01-02T00:00:10')])];emb={i:np.array([1.,0.]) for i in range(5)};out=env['semantic_deduplicate'](rows,emb,threshold=.92,config={'time_window_seconds':30});assert len(out)==1 and out[0]['cluster_size']==4 and [x['filepath'] for x in out[0]['similar_photos']]==['a1','a2','b'];p={'scope':__doc__,'authored_controls_passed':1,'input_count':5,'input_embedding_norms':[float(np.linalg.norm(x)) for x in emb.values()],'output_count':len(out),'cluster_size':out[0]['cluster_size'],'recorded_siblings':[x['filepath'] for x in out[0]['similar_photos']],'missing_sibling':'b1','qualification':'Original full two-pass semantic pipeline on valid unit synthetic vectors reproduces loss of nested membership/count;not just preformed helper boundary. No actualCLIP/model/semanticaccuracy.'};(b/'semantic-pipeline-results.json').write_text(json.dumps(p,indent=2)+'\n');print(json.dumps(p,indent=2))
