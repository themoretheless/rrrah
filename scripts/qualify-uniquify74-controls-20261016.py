"""Execute unchanged upstream async handler bodies against owned in-memory Telegram adapters."""
from pathlib import Path
import ast,asyncio,hashlib,json,types
R=Path(__file__).resolve().parents[1];D=R/'docs/research/dedup-500';W=R/'.dedup-work/uniquify74-20261016';S=W/'source';h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
class FloodWait(Exception):
 def __init__(self):self.x=3
class Reply:
 def __init__(self,text=''):self.text=text;self.edits=[];self.deleted=False
 async def edit_text(self,text,*a,**kw):self.edits.append(text)
 edit=edit_text
 async def delete(self):self.deleted=True
class Message(Reply):
 def __init__(self,id=1):super().__init__();self.from_user=types.SimpleNamespace(id=id,mention='synthetic');self.message_id=100;self.replies=[]
 async def reply_text(self,text,*a,**kw):r=Reply(text);self.replies.append(r);return r
async def sleep(_):pass
presets={};exec(compile((S/'presets.py').read_text(),str(S/'presets.py'),'exec'),presets)
def namespace():
 tree=ast.parse((S/'plugins/commands.py').read_text());nodes=[n for n in tree.body if isinstance(n,ast.AsyncFunctionDef)]
 for n in nodes:n.decorator_list=[]
 ns={'Bot':object,'Message':Message,'Presets':presets['Presets'],'asyncio':types.SimpleNamespace(sleep=sleep),'FloodWait':FloodWait,'Config':types.SimpleNamespace(AUTH_USERS={1}),'purge_status':{},'chat':{},'main_delay':{},'reply_markup_start':None,'reply_markup_cancel':None,'reply_markup_close':None}
 exec(compile(ast.fix_missing_locations(ast.Module(body=nodes,type_ignores=[])),str(S/'plugins/commands.py'),'exec'),ns);return ns
class User:
 def __init__(self,entries,ns,error=None,cancel=False):self.entries=entries;self.ns=ns;self.error=error;self.cancel=cancel;self.attempts=[];self.deleted=[];self.visits=0
 async def iter_history(self,chat):
  for i,e in enumerate(self.entries):
   if self.cancel and i==1:self.ns['purge_status'].pop(1,None)
   self.visits+=1;yield types.SimpleNamespace(empty=False,message_id=i+1)
 async def get_messages(self,chat,id,**kw):
  e=self.entries[id-1];return types.SimpleNamespace(message_id=id,**{k:types.SimpleNamespace(file_unique_id=v) for k,v in e.items()})
 async def delete_messages(self,chat,id):
  self.attempts.append(id)
  if self.error:raise self.error()
  self.deleted.append(id)
async def run(label,entries,expected,error=None,cancel=False,configure=True,delay=False):
 ns=namespace();m=Message();u=User(entries,ns,error,cancel);c=types.SimpleNamespace(USER=u)
 if configure:ns['chat'][1]=-100111
 if delay:ns['main_delay'][1]=0
 await ns['delete_duplicates'](c,m)
 edits=[x for r in m.replies for x in r.edits];actual={'attempts':u.attempts,'deleted':u.deleted,'visits':u.visits,'reports_deleted_one':any('Messages deleted   : 1' in x for x in edits),'final_success':presets['Presets'].PROCESS_FINISHED_TEXT in edits,'state':{k:dict(ns[k]) for k in ['chat','purge_status','main_delay']}}
 for key,value in expected.items():assert actual[key]==value,(label,key,actual)
 return {'case':label,**actual}
async def controls():
 rows=[]
 rows.append(await run('same_document_id_deletes_second', [{'document':'a'},{'document':'a'}],{'deleted':[2],'final_success':True}))
 rows.append(await run('cross_media_type_shared_id', [{'document':'a'},{'video':'a'}],{'deleted':[2]}))
 rows.append(await run('different_ids_no_byte_comparison', [{'document':'a'},{'document':'b'}],{'deleted':[],'final_success':False}))
 rows.append(await run('photos_are_unsupported', [{'photo':'a'},{'photo':'a'}],{'deleted':[],'final_success':False}))
 rows.append(await run('generic_delete_failure_still_claims_success', [{'document':'a'},{'document':'a'}],{'attempts':[2],'deleted':[],'reports_deleted_one':True,'final_success':True},error=RuntimeError))
 rows.append(await run('floodwait_not_retried_still_claims_success', [{'document':'a'},{'document':'a'}],{'attempts':[2],'deleted':[],'reports_deleted_one':True,'final_success':True},error=FloodWait))
 rows.append(await run('cancel_before_second_message', [{'document':'a'},{'document':'a'}],{'deleted':[],'final_success':False,'state':{'chat':{},'purge_status':{},'main_delay':{}}},cancel=True,delay=True))
 rows.append(await run('unconfigured_purge_leaves_status', [],{'deleted':[],'state':{'chat':{},'purge_status':{1:1},'main_delay':{}}},configure=False))
 # There is an explicit authorization check in configuration; execute its refusal branch.
 ns=namespace();m=Message(id=2);m.text='/chat -100111';c=types.SimpleNamespace(USER=None);await ns['config_chat'](c,m);assert m.replies[0].edits==[presets['Presets'].NOT_AUTH_TXT] and not ns['chat'];rows.append({'case':'unauthorized_configuration_refused','chat':dict(ns['chat'])})
 # Cross-type IDs are synthetic, not a claim Telegram generates such collisions.
 return rows
if __name__=='__main__':
 results=[asyncio.run(controls()) for _ in range(2)];assert results[0]==results[1];out=D/'uniquify74-controls-20261016.json';assert not out.exists();out.write_text(json.dumps({'status':'verified_scoped_original_handler_controls','whole_repo_complete':False,'runs':2,'controls_per_run':len(results[0]),'controls':results,'producer_sha256':h(__file__),'source_pins':{str(p.relative_to(R)):h(p) for p in S.rglob('*') if p.is_file()},'adapters':'All4 original async command handler AST bodies compiled unchanged; decorators/imports omitted, original Presets executed. Fake in-memory message/user/exception/config/async sleep; no Pyrogram import, network, user session, real message or deletion. Python current interpreter, not upstream Python3.9/Pyrogram1.4.8 runtime.','limits':['No upstream tests exist in pinned tree','Shared cross-type ID synthetic only; distinct IDs no comparison demonstrated without asserting actual byte equality','Deletion failure report is reproduced only against fake API exceptions, not live Telegram','No portable byte/image duplicate engine, no production code copied or adopted']},indent=2)+'\n');print('9 original-handler controls twice passed; no Telegram operations')
