import ast,contextlib,io,json,logging,mimetypes,types
from pathlib import Path
from PIL import Image
base=Path(__file__).resolve().parent/'intra095-20261024'
def funcs(path,names,env):
 tree=ast.parse((base/path).read_text());nodes=[n for n in tree.body if isinstance(n,ast.FunctionDef) and n.name in names]
 exec(compile(ast.Module(body=nodes,type_ignores=[]),str(base/path),'exec'),env)
data=(base/'Tests/testimages/lolcat-crocs.jpg').read_bytes()
env=dict(mimetypes=mimetypes,Image=Image,io=io,HASH_SIZE=32,make_response=lambda b:types.SimpleNamespace(headers={},body=b),get_image_handle_from_id=lambda row:('owned.jpg',data))
funcs('inspector/image_views.py',{'guessItemMimeType','to_resource','get_resource_color_conversion','get_resource_rescaled'},env)
unknown=env['guessItemMimeType']('owned.unknown_rrrah_extension');response=env['get_resource_color_conversion'](1)
resize_error=None
try:env['get_resource_rescaled'](1)
except AttributeError as e:resize_error=str(e)
assert unknown is None and response.body.startswith(b'\x89PNG\r\n\x1a\n') and response.headers['Content-Type']=='image/jpeg' and 'ANTIALIAS' in resize_error
class Pool:
 def getconn(self,key):raise OSError('owned acquisition failure')
 def putconn(self,*args):raise AssertionError('unexpected release')
poolenv=dict(contextlib=contextlib,__cpool=Pool());funcs('inspector/db_pool.py',{'db_cursor'},poolenv)
error=None
try:
 with poolenv['db_cursor']():pass
except Exception as e:error=dict(type=type(e).__name__,message=str(e),context_type=type(e.__context__).__name__)
assert error['type']=='UnboundLocalError' and error['context_type']=='OSError'
print(json.dumps(dict(unknown_extension_mime=unknown,converted_payload_is_png=True,converted_content_type=response.headers['Content-Type'],rescale_error=resize_error,cursor_acquisition_error=error,scope='Exact AST-selected original inspector functions;real original JPEG and Pillow;response/query adapters;no Flask application,HTTPserver,PostgreSQL or session pool started'),sort_keys=True))
