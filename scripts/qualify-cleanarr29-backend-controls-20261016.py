"""Unchanged original Python backend with exact pinned dependencies, fake Plex transport and owned DB/files."""
from pathlib import Path
import hashlib,json,os,sys,types,importlib.metadata
R=Path(__file__).resolve().parents[1];D=R/'docs/research/dedup-500';W=R/'.dedup-work/cleanarr29-20261016';S=W/'source/backend';B=W/'backend-controls-v2';B.mkdir(exist_ok=False);h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();os.environ.update(DEBUG='0',PLEXAPI_CONFIG_PATH=str(B/'absent-plex-config.ini'));sys.path[:0]=[str(W/'python-deps'),str(S)]
import requests
# Guard before any source import: accidental HTTP is a test failure, never a real request.
def no_network(*a,**kw):raise AssertionError('Network forbidden in owned backend controls')
requests.sessions.Session.request=no_network
import plexwrapper,database,main as appmod
assert Path(plexwrapper.__file__).resolve()==S/'plexwrapper.py';assert Path(database.__file__).resolve()==S/'database.py';assert Path(appmod.__file__).resolve()==S/'main.py'
class Media:
 def __init__(self,id,calls,fail=False):
  self.id=id;self.parts=[types.SimpleNamespace(size=20),types.SimpleNamespace(size=30)];self.calls=calls;self.fail=fail
 def delete(self):
  if self.fail:raise RuntimeError('owned deletion refusal')
  self.calls.append(self.id)
class Section:
 def __init__(self,type,items):self.type=type;self.title=type;self.items=items;self.searches=[]
 def all(self):return self.items
 def search(self,**kw):self.searches.append(kw);return self.items
results=[]
for repeat in [1,2]:
 case=B/f'run{repeat}';case.mkdir();os.environ['CONFIG_DIR']=str(case);calls=[];db=database.Database();wrapper=plexwrapper.PlexWrapper.__new__(plexwrapper.PlexWrapper);wrapper.db=db;wrapper.page_size=2;wrapper.libraries=['Movies','TV'];wrapper.baseurl='https://owned.invalid';wrapper.traces={};wrapper.plex=types.SimpleNamespace(friendlyName='Owned fixture',machineIdentifier='owned',fetchItem=lambda key:content)
 content=types.SimpleNamespace(TYPE='movie',key='/owned',title='Owned',type='movie',librarySectionID=1,thumbUrl='https://owned.invalid/thumb',duration=1000,guid='owned',year=2020,media=[Media(7,calls)],seasonNumber=1,seasonEpisode='s01e01',grandparentTitle='Owned series')
 rows=[]
 def record(name,value,expected):assert value==expected,(name,value,expected);rows.append({'case':name,'observed':value})
 movie_section=Section('movie',[content]);episode=types.SimpleNamespace(**vars(content));episode.TYPE='episode';tv_section=Section('show',[episode]);sections=[movie_section,tv_section];wrapper._get_sections=lambda:sections
 record('short_movie_and_episode_samples_all_skipped',wrapper.get_content_sample_files(),[])
 # Dispatcher on original method, serializing a simple object through unchanged original movie_to_dict.
 content.media=[Media(7,calls),Media(8,calls)];episode.media=content.media
 # Fake media adapters for nested serialization: preserve dispatch result independently of Plex field-fetch details.
 original_media=wrapper.media_to_dict;wrapper.media_to_dict=lambda media:{'id':media.id}
 tv=wrapper.get_dupe_content_for_section(3,tv_section);record('show_section_dispatched_as_movie',[x['contentType'] for x in tv],['movie']);record('page3_search_arguments',tv_section.searches[-1],{'duplicate':True,'libtype':'episode','container_start':4,'limit':6})
 record('unsupported_section_uses_dict_empty',wrapper.get_dupe_content_for_section(1,Section('music',[])),{})
 wrapper.media_to_dict=original_media
 db.set_deleted_size('Movies',10);db.set_deleted_size('TV',20);record('native_tinydb_two_library_fields_retained',wrapper.get_deleted_sizes(),{'Movies':10,'TV':20})
 content.media=[Media(7,calls)];wrapper.delete_media('Movies','/owned',7);record('existing_fake_media_deleted_and_bytes_counted',{'calls':list(calls),'size':db.get_deleted_size('Movies')},{'calls':[7],'size':60})
 wrapper.delete_media('Movies','/owned',999);record('missing_media_no_effect',{'calls':list(calls),'size':db.get_deleted_size('Movies')},{'calls':[7],'size':60})
 appmod.PlexWrapper=lambda:wrapper;appmod.app.config.update(TESTING=True);static=case/'static';static.mkdir();(static/'index.html').write_text('OWNED_INDEX');outside=case/'outside-owned.txt';outside.write_text('OWNED_OUTSIDE_STATIC_SENTINEL');appmod.app.static_folder=str(static);client=appmod.app.test_client();proxied=[]
 def fake_get(url):proxied.append(url);return types.SimpleNamespace(content=b'OWNED_PROXY_BYTES')
 appmod.requests=types.SimpleNamespace(get=fake_get)
 r=client.post('/delete/media',json={'library_name':'Movies','content_key':'/owned','media_id':999});record('missing_media_http_still_success',{'status':r.status_code,'json':r.json,'calls':list(calls)},{'status':200,'json':{'success':True},'calls':[7]})
 content.media=[Media(9,calls,fail=True)];r=client.post('/delete/media',json={'library_name':'Movies','content_key':'/owned','media_id':9});record('delete_error_http500_counter_retained',{'status':r.status_code,'size':db.get_deleted_size('Movies'),'calls':list(calls)},{'status':500,'size':60,'calls':[7]})
 r=client.post('/delete/media',json={'media_id':7});record('missing_request_fields_become500',r.status_code,500)
 r=client.get('/content/dupes?page=not-a-number');record('invalid_page_becomes500',r.status_code,500)
 r=client.get('/content/samples');record('sample_endpoint_returns_empty',{'status':r.status_code,'json':r.json},{'status':200,'json':[]})
 r=client.get('/server/proxy',query_string={'url':'https://owned.invalid/arbitrary-path'});record('proxy_passes_arbitrary_supplied_url_to_fake_transport',{'status':r.status_code,'body':r.data.decode(),'url':proxied[-1]},{'status':200,'body':'OWNED_PROXY_BYTES','url':'https://owned.invalid/arbitrary-path'})
 r=client.get('/../outside-owned.txt');record('static_parent_path_reads_owned_sibling',{'status':r.status_code,'body':r.data.decode()},{'status':200,'body':'OWNED_OUTSIDE_STATIC_SENTINEL'})
 r=client.get('/unmapped-spa-route');record('spa_fallback_index',{'status':r.status_code,'body':r.data.decode()},{'status':200,'body':'OWNED_INDEX'})
 for _ in range(2):assert client.post('/content/ignore',json={'content_key':'/owned'}).status_code==200
 record('native_ignore_duplicates_stored',len(db.get_db().table('ignored')),2);assert client.post('/content/unignore',json={'content_key':'/owned'}).status_code==200;record('unignore_removes_all_same_key',db.get_ignored_item('/owned'),None)
 record('server_info_original_route',client.get('/server/info').json,{'name':'Owned fixture','url':'https://owned.invalid/web/index.html'})
 # Source Class method handles attribute fetch failures by returning None.
 record('fetch_attribute_exception_swallowed',plexwrapper.PlexWrapper.fetch_attribute(lambda:(_ for _ in ()).throw(RuntimeError('owned attribute failure'))),None)
 db.get_db().close();results.append(rows)
assert results[0]==results[1] and len(results[0])==19
versions={}
for line in (S/'requirements.txt').read_text().splitlines():
 name,version=line.split('==');actual=importlib.metadata.version(name);assert actual==version,(name,actual,version);versions[name]=actual
loaded={str(Path(m.__file__).resolve().relative_to(R)):h(Path(m.__file__).resolve()) for m in list(sys.modules.values()) if getattr(m,'__file__',None) and Path(m.__file__).resolve().is_relative_to(W/'python-deps') and Path(m.__file__).is_file()}
out=D/'cleanarr29-backend-controls-20261016.json';assert not out.exists();out.write_text(json.dumps({'status':'verified_original_backend_19controls_twice_owned_plex_adapters','whole_repo_complete':False,'producer_sha256':h(__file__),'runs':results,'source_pins':{str(x.relative_to(R)):h(x) for x in S.glob('*.py')},'requirement_versions':versions,'loaded_dependency_file_pins':loaded,'scope':'Whole unchangedoriginalplexwrapper/database/main/utils/logger modules imported withall19exactrequirementversions. NativeFlask3.0/TinyDB4.7/testclient andoriginalclass methods/routes executed. FakeexternalPlextransport/media.delete only,in-memoryfakeproxyHTTP; originalsconstructor bypassed, originalmedia_to_dict temporarily replaced onlyforTVdispatchcontrol. Session.request forbidden before sourceimports. OwnedCONFIG_DIR/static/siblingfixtures andownPlexconfigpath; no usercredentials/config read orlivePlex/HTTP/message/delete operation.','findings':['SampleTYPE predicate alwaysskips movies/episodes;showsectiondispatchesmovie serializer','Missingmedia deleteendpoint returnssuccess despite no deletion; exceptionerror500counterpreserved','Invalidpage/missingfields500;proxyarbitraryURLpassed tofakeget, nooutboundrequest','Flasktestclient parentpath readsowned siblingstaticfile; notdeployedwebserver/externaldata proof','TinyDB preserves2libraryfields; duplicateignore inserts2rows,unignore removesboth; fetchattribute errorsbecomeNone'],'limits':['FullReactUI/build/upstreamtests/actualPlexAPI network/productionpermissions/concurrency/performance notqualified','Onlypositiveerror/boundarycontrols onfakeexternalservice; notcontent/visualduplicateaccuracy oradoption','Originalbenchmarks notexecuted/no dotenvloaded, realmedia neveraccessed/deleted']},indent=2)+'\n');print('19originalbackend controls twice passed; exactFlask/TinyDB, no network orrealPlexdeletion')
