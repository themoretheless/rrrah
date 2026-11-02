import pathlib,ast,types,json
b=pathlib.Path.cwd()/'docs/research/dedup-500';p=b/'evidence/hydrusvideodeduplicator--hydrus-video-deduplicator/73-api.py';tree=ast.parse(p.read_text());tree.body=[n for n in tree.body if not(isinstance(n,ast.Import) and any(a.name=='requests' for a in n.names))];calls=[]
response=types.SimpleNamespace(raise_for_status=lambda:None,json=lambda:{})
session=types.SimpleNamespace(request=lambda *a,**kw:(calls.append((a,kw)) or response))
req=types.SimpleNamespace(ConnectTimeout=type('ConnectTimeout',(Exception,),{}),Response=object,Session=object,RequestException=type('RequestException',(Exception,),{}),HTTPError=type('HTTPError',(Exception,),{}),packages=types.SimpleNamespace(urllib3=types.SimpleNamespace(disable_warnings=lambda:None)))
g={'requests':req};exec(compile(tree,str(p),'exec'),g);C=g['Client'];c=C(session=session);results=[]
try:c._api_request('POST','/owned',json={'owned':1})
except KeyError as ex:assert ex.args==('headers',);results.append({'name':'json_without_access_key_or_explicit_headers','observed':'KeyError(headers) before request','pass':True})
else:raise AssertionError()
c=C(access_key='owned',session=session);c._api_request('POST','/owned',json={'owned':1});assert json.loads(calls[-1][1]['data'])=={'owned':1};results.append({'name':'json_with_access_key_serializes','pass':True});c.get_file(hash_='owned');assert calls[-1][1]['stream'] is True and 'timeout' not in calls[-1][1];results.append({'name':'get_file_requests_stream_without_timeout','pass':True});(b/'hydrus107-api-controls-20261024.json').write_text(json.dumps({'scope':'Whole unchanged API module with requests module replaced by recording session/types; no network calls.','controls':results},indent=2)+'\n');print(json.dumps(results))
