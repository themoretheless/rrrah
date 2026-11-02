import ast,pathlib,types,time,logging,json
b=pathlib.Path(__file__).parent;s=b.parent/'evidence/WSH032--image-deduplicate-cluster-webui/remaining-pinned/img_dedup_clust/wd14_fn.py';tree=ast.parse(s.read_text());fn=next(n for n in tree.body if isinstance(n,ast.FunctionDef) and n.name=='use_wd14');fn.decorator_list=[]
created=[];calls=[]
class Tagger:
 def __init__(self,**kw):created.append(kw)
 def inference(self,**kw):calls.append(kw)
ns={'Tagger':Tagger,'tagger':None,'WD14_MODEL_DIR':'/owned/model-placeholder','logging':logging,'time':time,'gr':types.SimpleNamespace(update=lambda **kw:kw)};exec(compile(ast.Module(body=[fn],type_ignores=[]),str(s),'exec'),ns)
args=dict(train_data_dir='/owned/images-placeholder',batch_size=1,max_data_loader_n_workers=0,recursive=False,general_threshold=.35,characters_threshold=.85,caption_extension='.txt',remove_underscore=False,rating=False,undesired_tags='',concurrent_inference=False,if_use_tensorrt=False,tensorrt_batch_size=1)
ns['use_wd14'](model_type=0,keep_updating=False,**args);ns['use_wd14'](model_type=1,keep_updating=True,**args);assert len(created)==1 and created[0]['model_type']==0 and len(calls)==2
(b/'tagger-reuse-results.json').write_text(json.dumps({'cases':1,'scope':'Original undecorated use_wd14 function AST-extracted; fake Tagger records construction/inference, gr.update adapter; no model/download/filesystem or real inference.','observed':'Second call changes model_type and keep_updating but cached Tagger remains original until explicit release_memory.','constructors':created,'inference_calls':len(calls)},indent=2)+'\n');print('Cached model reuse across changed selection reproduced')
