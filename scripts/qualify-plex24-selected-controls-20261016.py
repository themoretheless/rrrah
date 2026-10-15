"""Execute unchanged selected AST bodies and main block with explicit local Plex/HTTP/UI adapters."""
from pathlib import Path
import ast,collections,contextlib,copy,hashlib,io,itertools,json,logging,os,sys,types
from fnmatch import fnmatch
from urllib.parse import urljoin
R=Path(__file__).resolve().parents[1];W=R/'.dedup-work/plex24-20261016';S=W/'source';D=R/'docs/research/dedup-500';D.mkdir(exist_ok=True)
h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
source=ast.parse((S/'plex_dupefinder.py').read_text());config=ast.parse((S/'config.py').read_text());cfg={}
# Select exact function bodies, retaining locations and byte-pinned original source.
ns=dict(collections=collections,itertools=itertools,logging=logging,os=os,sys=sys,fnmatch=fnmatch,urljoin=urljoin,log=logging.getLogger('plex24-control'),time=types.SimpleNamespace(sleep=lambda _:None))
exec(compile(ast.Module(body=[n for n in source.body if isinstance(n,ast.FunctionDef)],type_ignores=[]),str(S/'plex_dupefinder.py'),'exec'),ns)
cn=dict(json=json,os=os,sys=sys)
exec(compile(ast.Module(body=[n for n in config.body if isinstance(n,ast.FunctionDef) or isinstance(n,ast.Assign) and any(isinstance(t,ast.Name) and t.id=='base_config' for t in n.targets)],type_ignores=[]),str(S/'config.py'),'exec'),cn)
base=copy.deepcopy(cn['base_config']);ns['cfg']=copy.deepcopy(base);out=[]
def record(name,value):out.append(dict(control=name,result=value))
def capture(fn):
 try:return dict(value=fn())
 except Exception as e:return dict(error=type(e).__name__,message=str(e))
# Native metadata/scoring/formatting functions, no real service access.
ns['cfg']['SKIP_LIST']=['/Movies4K/'];assert ns['should_skip'](['/Movies4K/a']) and not ns['should_skip'](['/movies4k/a']);record('skip_list_case_sensitive_substring',True)
ns['cfg']['SKIP_LIST']=[''];assert ns['should_skip'](['/a']);record('empty_skip_pattern_matches_every_path',True)
record('duration_wraps_at24hours',ns['millis_to_string'](25*3600*1000));assert out[-1]['result']=='01:00:00'
a=types.SimpleNamespace(channels=6,title='English');p=types.SimpleNamespace(file='/a',size=100,audioStreams=lambda:[a,a]);item=types.SimpleNamespace(id=1,parts=[p,p],audioChannels=6)
info=ns['get_media_info'](item);assert info['audio_channels']==24 and info['file_size']==200 and info['multipart'];record('audio_channels_sum_tracks_and_parts',info)
record('missing_parts_not_guarded',capture(lambda:ns['get_media_info'](types.SimpleNamespace())));assert out[-1]['result']['error']=='AttributeError'
ns['cfg']=copy.deepcopy(base);info=dict(info,audio_channels=0,file=['/a','/b'],video_duration=0);ns['cfg']['FILENAME_SCORES']={'*':7};assert ns['get_score'](info)==14;record('filename_score_accumulates_per_part',14)
ns['cfg']['VIDEO_CODEC_SCORES']={'x':'not-an-int'};bad=dict(info,video_codec='x');record('invalid_score_value',capture(lambda:ns['get_score'](bad)));assert out[-1]['result']['error']=='ValueError'
# Config uses shallow copy and first already-valid auto-delete answer is never assigned.
x=cn['prefilled_default_config'](dict(url='local',token='synthetic',auto_delete=False));x['AUDIO_CODEC_SCORES']['synthetic']=99;assert cn['base_config']['AUDIO_CODEC_SCORES']['synthetic']==99;record('prefilled_config_shares_nested_dict',True)
cn['base_config']=copy.deepcopy(base)
for responses,expected in [(['local','fake-user','y'],False),(['local','fake-user','bad','y'],True)]:
 path=W/('config-'+str(len(out))+'.json');path.unlink(missing_ok=True);answers=iter(responses);cn.update(config_path=str(path),input=lambda _:next(answers),getpass=lambda _:'synthetic',MyPlexAccount=lambda *_:types.SimpleNamespace(authenticationToken='synthetic'))
 with contextlib.redirect_stdout(io.StringIO()):assert cn['build_config']()
 actual=json.loads(path.read_text())['AUTO_DELETE'];assert actual==expected;record('config_auto_delete_'+('-'.join(responses[2:])),actual)
# The unchanged full main block runs with synthetic Plex objects and an HTTP recorder.
main=next(n for n in source.body if isinstance(n,ast.If) and isinstance(n.test,ast.Compare));main_code=compile(ast.Module(body=[main],type_ignores=[]),str(S/'plex_dupefinder.py'),'exec')
def media(mid,path,bitrate=1):return types.SimpleNamespace(id=mid,parts=[types.SimpleNamespace(file=path,size=1,audioStreams=lambda:[])],bitrate=bitrate,audioChannels=0)
def movie(title,mid1,mid2,key='/library/metadata/1',bitrate=1):return types.SimpleNamespace(type='movie',title=title,key=key,locations=['/a','/b'],media=[media(mid1,'/keep',bitrate),media(mid2,'/protected/victim',bitrate)])
def run_main(items,auto=True,answer='b',path_only=False,skip=None):
 n=ns;c=copy.deepcopy(base);c.update(PLEX_LIBRARIES=['Movies'],PLEX_SERVER='http://synthetic.invalid',PLEX_TOKEN='synthetic',AUTO_DELETE=auto,FIND_DUPLICATE_FILEPATHS_ONLY=path_only,SKIP_LIST=skip or [])
 section=types.SimpleNamespace(type='movie',search=lambda **_:items);n['cfg']=c;n['plex']=types.SimpleNamespace(library=types.SimpleNamespace(section=lambda _:section));calls=[]
 n.update(__name__='__main__',input=lambda _:answer,tabulate=lambda *_args,**_kwargs:'display-only adapter',decision_filename=str(W/'decisions-owned.log'),requests=types.SimpleNamespace(delete=lambda url,**kwargs:(calls.append(dict(url=url,kwargs=kwargs)) or types.SimpleNamespace(status_code=200))))
 buf=io.StringIO()
 with contextlib.redirect_stdout(buf):res=capture(lambda:exec(main_code,n))
 return dict(result=res,http_delete_calls=calls,stdout=buf.getvalue())
a=run_main([movie('same',1,2),movie('same',3,4,key='/library/metadata/2')]);assert len(a['http_delete_calls'])==1 and '/2/media/4' in a['http_delete_calls'][0]['url'];record('same_title_overwrites_earlier_item',a)
a=run_main([movie('manual',1,2)],auto=False,skip=['/protected/']);assert len(a['http_delete_calls'])==1;record('manual_mode_does_not_apply_skip_list',a)
a=run_main([movie('automatic',1,2)],skip=['/protected/']);assert not a['http_delete_calls'];record('automatic_mode_applies_skip_list',a)
a=run_main([movie('manual',1,2)],auto=False,answer='invalid');assert a['result']['error']=='ValueError' and not a['http_delete_calls'];record('manual_non_numeric_response_aborts',a)
a=run_main([movie('manual',1,2)],auto=False,answer='0');assert not a['http_delete_calls'];record('manual_zero_skips',a)
a=run_main([movie('manual',1,2)],auto=False,answer='s');assert not a['http_delete_calls'];record('manual_s_skips',a)
a=run_main([movie('zero-id',0,2)]);assert not a['http_delete_calls'];record('zero_keeper_id_is_falsy_no_delete',a)
a=run_main([movie('negative',1,2,bitrate=-10)]);assert not a['http_delete_calls'];record('all_negative_scores_no_keeper',a)
# Path-only filtering admits duplicate names only; does not inspect file bytes.
n=dict(ns);n['cfg']=dict(base,FIND_DUPLICATE_FILEPATHS_ONLY=True);objs=[types.SimpleNamespace(locations=['/same','/same']),types.SimpleNamespace(locations=['/a','/b'])];sec=types.SimpleNamespace(type='show',search=lambda **kw:objs);n['plex']=types.SimpleNamespace(library=types.SimpleNamespace(section=lambda _:sec))
# Functions share their original globals; temporarily use the selected namespace for this control.
ns['cfg']=n['cfg'];ns['plex']=n['plex'];assert ns['get_dupes']('TV')==objs[:1];record('path_only_identical_strings_admitted',1)
ns['plex']=types.SimpleNamespace(library=types.SimpleNamespace(section=lambda _:types.SimpleNamespace(type='movie',search=lambda **_: [types.SimpleNamespace(locations=[])])));record('path_only_empty_locations',capture(lambda:ns['get_dupes']('Movies')));assert out[-1]['result']['value'] # any(empty) is false, item retained
# HTTP adapter records exact URL, method args and lack of timeout; no request emitted.
calls=[];ns['cfg']=dict(base,PLEX_SERVER='https://synthetic.invalid/base/',PLEX_TOKEN='synthetic');ns['requests']=types.SimpleNamespace(delete=lambda *args,**kw:(calls.append(dict(args=args,kwargs=kw)) or types.SimpleNamespace(status_code=500)))
with contextlib.redirect_stdout(io.StringIO()):ns['delete_item']('/library/metadata/5',8)
assert calls[0]['args'][0]=='https://synthetic.invalid/library/metadata/5/media/8' and 'timeout' not in calls[0]['kwargs'];record('delete_absolute_url_and_no_timeout_status500_returns',calls)
# Convert synthetic object results to stable JSON descriptions.
for row in out:
 if row['control']=='path_only_empty_locations':row['result']=dict(retained_items=len(row['result']['value']))
report=dict(scope='Unchanged selected AST function bodies and original main block; synthetic Plex metadata/HTTP response recorder, display-only tabulate and no-op sleep, owned config/log files. No original package import, real Plex/HTTP, shell deletion, upstream suite, quality/performance/platform claim.',source_pins={p:h(S/p) for p in ['plex_dupefinder.py','config.py','deletefiles.sh','requirements.txt']},controls=out,producer_sha256=h(__file__))
run=sys.argv[1];p=D/f'plex24-selected-run{run}-20261016.json';assert not p.exists();p.write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(dict(controls=len(out),run=run)))
