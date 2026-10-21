import ast,contextlib,io,json,pathlib,hashlib,re,sys,types
p=pathlib.Path('.dedup-work/spotify351-20261021/main.py');b=p.read_bytes();tree=ast.parse(b)
ns={'sys':types.SimpleNamespace(argv=['main.py','synthetic-user','p']),'re':re,'sa':None}
exec(compile(ast.Module(body=[n for n in tree.body if isinstance(n,(ast.FunctionDef,ast.ClassDef,ast.Assign))],type_ignores=[]),str(p),'exec'),ns)
def track(id,name,duration):return {'track':{'id':id,'name':name,'duration_ms':duration,'artists':[{'id':'artist','name':'Synthetic artist'}]}}
tracks=[track('a','Alpha',100),track('b','Beta',100),track('c','Beta',200)]
class Auth:
 def __init__(self,*a):pass
 def wait_for_auth(self):pass
 def stop_server(self):pass
 def get_spotify(self):return None
class Tool:
 calls=[]
 def __init__(self,*a):pass
 def get_user_playlists(self):return [{'id':'p','name':'Synthetic playlist'}]
 def set_playlist_by_name(self,*a):pass
 def get_playlist_tracks(self):return tracks
 def remove_specific_track(self,*a):self.calls.append(a);raise AssertionError('No mutation expected')
ns['sa']=types.SimpleNamespace(SpotifyAuth=Auth,SpotifyTool=Tool,FailedAuth=RuntimeError,NotFound=LookupError)
ns['input']=lambda _: 'n';out=io.StringIO()
with contextlib.redirect_stdout(out):ns['main']()
text=out.getvalue();assert '1.' in text and '2.' in text
# Both reports nominate original position2 (b): A~B by duration and C~B by name.
assert text.count('(#2) by Synthetic artist')==2,text
assert not Tool.calls
candidates=ns['calculate_positions'](tracks);answers=iter(['1,1','continue']);ns['input']=lambda _:next(answers)
with contextlib.redirect_stdout(io.StringIO()):ns['validate_songs_to_remove'](candidates)
assert [x['track']['id'] for x in candidates]==['c']
r={'source_sha256':hashlib.sha256(b).hexdigest(),'method':'Original classes/functions from main.py compiled unchanged. Fake auth/tool, synthetic metadata and scripted input; no imports of spotifyauth, HTTP, Spotify or playlist writes.','duplicate_nomination_of_same_position':{'original_position_1_based':2,'times':2,'stdout':text},'repeated_keep_selection_1_1':{'expected_unique_keep_ids':['a'],'actual_removed_from_candidate_list':['a','b'],'remaining_candidates':['c']},'limits':'Control-flow and list-selection counterexamples only; no Spotify API/version, deletion or audio-content qualification.'}
q=pathlib.Path('docs/research/dedup-500/spotify351-controls-20261021/result.json');q.write_text(json.dumps(r,indent=2)+'\n');print(q.read_text())
