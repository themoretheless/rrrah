"""Selected unchanged local functions; excludes all imports/token/env/bot/decorators/remote handlers."""
from pathlib import Path
import ast,sys,json,asyncio,io,traceback,os,tempfile,contextlib,types
from functools import partial
from unittest.mock import patch
R=Path(__file__).resolve().parents[3];D=Path(__file__).parent;W=R/'.dedup-work/discord437-20261015';(W/'.gitignore').write_text('*\n');sys.path.insert(0,str(W/'deps'))
from PIL import Image,UnidentifiedImageError
import imagehash
S=D/'evidence/AlgorithMikely--DiscordDuplicateImageDetector/02-discord_duplicate_bot_v1.py';names={'get_default_guild_config','validate_config_data','load_main_config','save_main_config','get_guild_config','save_guild_config','get_hash_file_lock','calculate_hash_sync','calculate_hash','load_hashes_sync','save_hashes_sync','load_guild_hashes','save_guild_hashes','find_duplicates_sync','find_duplicates'};tree=ast.parse(S.read_text());nodes=[x for x in tree.body if isinstance(x,(ast.FunctionDef,ast.AsyncFunctionDef)) and x.name in names];assert len(nodes)==len(names) and not any(x.decorator_list for x in nodes)
ns=dict(HASH_FILENAME_PREFIX='hashes_',VALID_SCOPES=['server','channel'],VALID_CHECK_MODES=['strict','owner_allowed'],Image=Image,UnidentifiedImageError=UnidentifiedImageError,imagehash=imagehash,io=io,os=os,sys=sys,json=json,asyncio=asyncio,partial=partial,traceback=traceback,server_configs={},hash_file_locks={});exec(compile(ast.Module(body=nodes,type_ignores=[]),str(S),'exec'),ns);m=types.SimpleNamespace(**ns);passed=[]
def check(name,fn):
 with tempfile.TemporaryDirectory(dir=W) as p,contextlib.redirect_stdout(io.StringIO()),contextlib.redirect_stderr(io.StringIO()):fn(Path(p))
 passed.append(name);print('PASS',name,flush=True)
def image_bytes(image,fmt='PNG',**kw):
 b=io.BytesIO();image.save(b,format=fmt,**kw);return b.getvalue()
def defaults(p):
 c=m.get_default_guild_config(7);assert c['hash_size']==8 and c['similarity_threshold']==5 and not c['delete_duplicates']
check('safe default delete flag and fixed hash schema',defaults)
def boolean(p):
 c=m.validate_config_data({'delete_duplicates':'false','react_to_duplicates':'false'});assert c['delete_duplicates'] is True and c['react_to_duplicates'] is True
check('string false becomes true in file configuration',boolean)
def limits(p):
 c=m.validate_config_data({'hash_size':-2,'similarity_threshold':10**12});assert c['hash_size']==-2 and c['similarity_threshold']==10**12
check('loaded config hash and threshold numeric ranges unbounded',limits)
def channels(p):
 c=m.validate_config_data({'allowed_channel_ids':['bad','-1']});assert c['allowed_channel_ids'] is None
check('entirely invalid allowed channel list becomes all channels',channels)
def invalidsize(p):
 c=m.validate_config_data({'hash_size':'bad','similarity_threshold':'7','delete_duplicates':'false'});assert c['hash_size']=='bad' and c['similarity_threshold']=='7' and c['delete_duplicates']=='false'
check('first coercion error leaves later values unvalidated',invalidsize)
def flat(p):
 hashes=[m.calculate_hash_sync(image_bytes(Image.new('RGB',(16,16),(i,i,i))),8) for i in range(256)];assert all(str(h)=='0000000000000000' for h in hashes);assert len(m.find_duplicates_sync(hashes[0],{'1-a':{'hash':str(hashes[-1]),'user_id':2}},0,'server','3'))==1
check('256flat grays share dHash and blackwhite is zero-distance candidate',flat)
def alpha(p):
 a=Image.new('RGBA',(16,16),(19,100,240,0));b=Image.new('RGBA',(16,16),(19,100,240,255));assert m.calculate_hash_sync(image_bytes(a),8)==m.calculate_hash_sync(image_bytes(b),8)
check('alpha-only difference omitted by hash conversion',alpha)
def animation(p):
 first=Image.new('RGB',(16,16),'white');a=image_bytes(first,'GIF',save_all=True,append_images=[Image.new('RGB',(16,16),'black')],duration=50);b=image_bytes(first,'GIF',save_all=True,append_images=[Image.new('RGB',(16,16),'red')],duration=50);assert a!=b and m.calculate_hash_sync(a,8)==m.calculate_hash_sync(b,8)
check('different later GIF frames share first-frame hash',animation)
def malformed(p):assert m.calculate_hash_sync(b'not an image',8) is None
check('malformed image returns None',malformed)
def different_hash_schema(p):
 h=m.calculate_hash_sync(image_bytes(Image.new('RGB',(16,16),'white')),8);old=m.calculate_hash_sync(image_bytes(Image.new('RGB',(16,16),'white')),16);assert m.find_duplicates_sync(h,{'1-a':{'hash':str(old),'user_id':2}},5,'server','3')==[]
check('hash-size change silently ignores old incompatible hashes',different_hash_schema)
def owner_metadata(p):
 h=imagehash.hex_to_hash('0'*16);r=m.find_duplicates_sync(h,{'42-file-name':'0'*16},0,'server','3');assert len(r)==1 and r[0]['original_user_id'] is None and r[0]['original_message_id']==42
check('old format match owner remains unknown',owner_metadata)
def scope(p):
 h=imagehash.hex_to_hash('0'*16);db={'42-a':{'hash':'0'*16,'user_id':2}};assert len(m.find_duplicates_sync(h,db,0,'server','3'))==1 and m.find_duplicates_sync(h,db,0,'channel','3')==[]
check('server to channel scope switch does not migrate old entries',scope)
def truncated(p):
 f=p/'db';f.write_text('{"old":');assert m.load_hashes_sync(str(f))=={}
check('damaged JSON treated as empty hash database',truncated)
def partialsave(p):
 f=p/'db';f.write_text('{"old":"preserve"}');real=json.dump
 def fail(obj,handle,**kw):handle.write('{"partial":');raise OSError('injected disk failure')
 with patch.object(json,'dump',side_effect=fail):assert m.save_hashes_sync({'new':'value'},str(f)) is False
 assert f.read_text()=='{"partial":'
check('save failure truncates old database without rollback',partialsave)
def badguild(p):
 f=p/'configs';f.write_text(json.dumps({'1':{},'2':None}));ns.update(CONFIG_FILE_PATH=str(f),config_lock=asyncio.Lock(),server_configs={99:{}});asyncio.run(m.load_main_config());assert ns['server_configs']=={}
check('one malformed guild resets entire loaded configuration',badguild)
def race(p):
 async def once(path):
  cfg=m.get_default_guild_config(7);cfg['hash_db_file']=str(path);ns.update(server_configs={7:cfg},hash_file_locks={});path.write_text('{}');loop=asyncio.get_running_loop();barrier=asyncio.Event();reads=[]
  async def update(key):
   data=await m.load_guild_hashes(7,loop);reads.append(len(data))
   if len(reads)==2:barrier.set()
   await barrier.wait();data[key]={'hash':'0'*16,'user_id':1};assert await m.save_guild_hashes(7,data,loop)
  await asyncio.gather(update('1-a'),update('2-b'));final=m.load_hashes_sync(str(path));assert reads==[0,0] and len(final)==1
 async def allruns():
  for i in range(3):await once(p/f'db-{i}')
 asyncio.run(allruns())
check('three actual async executor schedules lose one locked read-modify-write update',race)
print(json.dumps(dict(passed=len(passed),failed=0,scope='15unchanged selected local function ASTs, realPillow/ImageHash/numpy and actual async thread executor/file locks. No Discord import/decorators/token/env/network/API, no messages sent. Filewrite fault and explicit two-reader barrier identified; no wholebot deployment/runtime/performance/corpus proof.')))
