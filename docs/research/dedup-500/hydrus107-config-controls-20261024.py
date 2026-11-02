import ast,pathlib,types,json,logging,hashlib
b=pathlib.Path.cwd()/'docs/research/dedup-500';e=b/'evidence/hydrusvideodeduplicator--hydrus-video-deduplicator';p=e/'17-config.py';tree=ast.parse(p.read_text());tree.body=[n for n in tree.body if not(isinstance(n,ast.ImportFrom) and n.module=='platformdirs')];g={'__name__':'owned_config','PlatformDirs':lambda *a:types.SimpleNamespace(user_data_dir='/owned/unused')};exec(compile(tree,str(p),'exec'),g);c=g['Config']._load({'SIMILARITY_THRESHOLD':'50','JOB_COUNT':'2','HYDRUS_QUERY':'[1,null]','HVD_GUI':'true'});assert c.similarity_threshold=='50' and c.job_count=='2';assert c.hydrus_query==[1,None];results=[{'control':'config_preserves_numeric_strings','pass':True},{'control':'query_array_allows_nonstring_elements','pass':True}]
try:bool(int(c.hvd_gui))
except ValueError:results.append({'control':'gui_true_string_rejected_by_entrypoint_conversion','pass':True})
else:raise AssertionError()
source=e/'18-entrypoint.py';node=next(n for n in ast.parse(source.read_text()).body if isinstance(n,ast.FunctionDef) and n.name=='main');node.returns=None
for arg in node.args.args:arg.annotation=None
node.args.defaults=[ast.Constant(None) for a in node.args.args]
class Exit(Exception):
 def __init__(self,code=None):super().__init__(code)
class ReachedClient(Exception):pass
def client(*a):raise ReachedClient()
g2={'__name__':'owned_main','logging':logging,'print_and_log':lambda *a:None,'typer':types.SimpleNamespace(Exit=Exit),'DedupeDB':types.SimpleNamespace(set_db_dir=lambda *a:None),'create_client':client,'FailedHVDClientConnection':type('ConnectionFailure',(Exception,),{}),'ClientAPIException':type('ApiFailure',(Exception,),{})};exec(compile(ast.fix_missing_locations(ast.Module(body=[node],type_ignores=[])),str(source),'exec'),g2)
for threshold,expected in [(-1.,Exit),(float('nan'),ReachedClient)]:
 try:g2['main'](api_key='owned',api_url='owned',threshold=threshold,skip_hashing=True,job_count=1)
 except expected:pass
 else:raise AssertionError()
results.append({'control':'nan_threshold_passes_validation_negative_threshold_rejected','pass':True});logging.disable(logging.NOTSET)
report={'scope':'AST-loaded unchanged config excluding platformdirs import; main body unchanged but annotations/defaults removed to avoid Typer dependency, explicit owned arguments and replacement client. No live connection or CLI parser qualification.','controls':results,'source_sha256':{x.name:hashlib.sha256(x.read_bytes()).hexdigest() for x in [p,source]}};(b/'hydrus107-config-controls-20261024.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report,indent=2))
