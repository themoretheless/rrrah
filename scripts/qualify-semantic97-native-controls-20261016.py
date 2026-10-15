"""Whole original modules, real NumPy, explicit in-memory SDK and dotenv adapters."""
from pathlib import Path
import sys,types,json,hashlib,warnings
R=Path(__file__).resolve().parents[1];W=R/'.dedup-work/semantic97-20261016';S=W/'source';A=W/'native-controls';A.mkdir(exist_ok=False)
import numpy as np
class Unavailable(Exception):pass
sdk=types.ModuleType('openai');sdk.error=types.SimpleNamespace(ServiceUnavailableError=Unavailable);state={'score':'99','name':'X','fail_embed':False,'fail_chat':0,'chat_calls':0,'embeds':[]}
def chat(**params):
 state['chat_calls']+=1
 if state['fail_chat']>0:state['fail_chat']-=1;raise Unavailable('owned unavailable')
 if 'functions' in params:return types.SimpleNamespace(choices=[{'message':{'function_call':{'arguments':json.dumps({'items':['X','Y']})}}}])
 content=state['score'] if 'Item #1:' in params['messages'][1]['content'] else state['name']
 return types.SimpleNamespace(choices=[types.SimpleNamespace(message={'content':content})])
def embedding(**params):
 state['embeds'].append(params['input'])
 if state['fail_embed']:raise RuntimeError('owned embedding failure')
 return {'data':[{'embedding':[1.,0.]}]}
sdk.ChatCompletion=types.SimpleNamespace(create=chat);sdk.Embedding=types.SimpleNamespace(create=embedding)
dotenv=types.ModuleType('dotenv');dotenv.load_dotenv=lambda:False;sys.modules.update(openai=sdk,dotenv=dotenv);sys.path.insert(0,str(S/'src'))
from semantic_deduplicator import SemanticDeduplicator as SD,DeduplicatedItem as DI
from semantic_deduplicator import utils
h=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
def item(name,vec,origin=None):
 x=DI.__new__(DI);x.name=name;x.item_embedding=list(vec);x.original_input_list=[origin or name];return x
runs=[]
for n in range(2):
 rows=[];state.update(score='99',name='X',fail_embed=False,fail_chat=0,chat_calls=0,embeds=[])
 def check(name,value,want):assert value==want,(name,value,want);rows.append({'case':name,'observed':value})
 sd=SD(background_context='owned fixture',openai_api_key='owned-A');q=item('query',[1,0]);x=item('X',[1,0]);y=item('Y',[0,1])
 check('cosine_identity',float(sd.cosine_similarity(q,x)),1.0)
 check('cosine_orthogonal',float(sd.cosine_similarity(q,y)),0.0)
 with warnings.catch_warnings():warnings.simplefilter('ignore');v=sd.cosine_similarity(q,item('zero',[0,0]))
 check('zero_norm_nan',bool(np.isnan(v)),True)
 failed=False
 try:sd.cosine_similarity(q,item('shape',[1,0,1]))
 except ValueError:failed=True
 check('embedding_shape_mismatch_raises',failed,True)
 sd.deduplicated_items_list=[x,y];check('cosine_prefilter_limits_candidates',[a.name for a,_ in sd.get_similar_items(q)],['X'])
 state['score']='200';check('unbounded_llm_score_accepted',sd.get_similar_items(q)[0][1],2.0)
 state['score']='-1';check('negative_llm_score_rejected',sd.get_similar_items(q),[])
 state['score']='99.5';failed=False
 try:sd.get_similar_items(q)
 except ValueError:failed=True
 check('decimal_llm_score_raises',failed,True)
 state['score']='99';z=item('Z',[1,0]);sd.deduplicated_items_list=[x,z];check('equal_score_tie_keeps_insertion_order',[a.name for a,_ in sd.get_similar_items(q)],['X','Z'])
 sd.cosine_similarity_threshold=float('nan');check('nan_threshold_silently_excludes_all',sd.get_similar_items(q),[]);sd.cosine_similarity_threshold=.75
 wrong=item('X',[0,1],'wrong');matched=item('X',[1,0],'matched');sd.deduplicated_items_list=[wrong,matched];sd.delete_item_from_string('owned query');check('same_name_delete_removes_wrong_object',[a.original_input_list[0] for a in sd.deduplicated_items_list],['matched'])
 state['name']='Changed';state['fail_embed']=True;v=item('Old',[0,1]);failed=False
 try:v.update_item_name('owned context','new')
 except RuntimeError:failed=True
 check('failed_embedding_leaves_new_name_old_vector',[failed,v.name,v.item_embedding],[True,'Changed',[0,1]]);state['fail_embed']=False;state['name']='X'
 sd.deduplicated_items_list=[x];formatted=sd.get_formatted_deduplicated_list('dict_list');formatted[0]['Original Names'].append('external mutation');check('formatted_dict_leaks_mutable_provenance',x.original_input_list,['X','external mutation'])
 failed=False
 try:sd.get_formatted_deduplicated_list('invalid')
 except ValueError:failed=True
 check('unknown_output_type_rejected',failed,True)
 a=SD(background_context='owned fixture',openai_api_key='owned-A');b=SD(background_context='owned fixture',openai_api_key='owned-B');check('constructor_mutates_global_sdk_key',sdk.api_key,'owned-B')
 sleeps=[];original_sleep=utils.time.sleep;utils.time.sleep=lambda t:sleeps.append(t);state['fail_chat']=3;before=state['chat_calls'];failed=False
 try:
  try:utils.call_llm()
  except Unavailable:failed=True
 finally:utils.time.sleep=original_sleep
 check('service_unavailable_three_attempts_backoff',[failed,state['chat_calls']-before,sleeps],[True,3,[1.5,3.0]])
 parsed=sd.parse_items_from_raw_item('owned input');check('function_call_items_parsed',parsed,['X','Y'])
 with warnings.catch_warnings():warnings.simplefilter('ignore');sd.deduplicated_items_list=[item('zero',[0,0])];matches=sd.get_similar_items(q)
 check('zero_embedding_silently_drops_candidate',matches,[])
 assert len(rows)==18;runs.append(rows)
assert runs[0]==runs[1]
report={'status':'verified_complete_original_modules_18_boundary_controls_twice','producer_sha256':h(Path(__file__)),'source_pins':{str(p.relative_to(R)):h(p) for p in (S/'src/semantic_deduplicator').glob('*.py')},'native_numpy_version':np.__version__,'loaded_numpy_file_pins':{str(Path(v.__file__)):h(Path(v.__file__)) for v in list(sys.modules.values()) if getattr(v,'__file__',None) and (v.__name__=='numpy' or v.__name__.startswith('numpy.')) and Path(v.__file__).is_file()},'runs':runs,'scope':'Complete unchanged originalmain/utils/init modules imported, realruntimeNumPy numerical operations. Explicit fakeOpenAI SDKtransport/error API and no-opdotenv adapter inserted before imports; ownliteral APIkeys only. DeterministicLLM/embedding outputs andrecordedsleep replace remote services/waits.18boundarycontrols twice; no upstreamsemantic APItests run.','findings':['Unbounded integerLLMscore200accepted2.0; decimalstringraises. Zero/nan vectors andnanthreshold silently exclude candidates; incompatible embedding shape raises.','Same-name candidates: deletion finds byname andremoves firstdifferentvector object instead of selectedsecond object.','Embedding failure after rename leaves changedname witholdvector; formatteddict returnsmutable provenance alias.','Constructor overwrites globalSDKkey; retryhandlesServiceUnavailable3attempts with1.5/3sec sleeps, othererror policies notqualified.'],'limits':['NumPy runtime version differsfromlocked1.25.2; originalPython3.9/fullPoetryenvironment/OpenAI0.27.10 SDK/dotenv/fullnetwork/upstream7semantic tests notqualified.','Fake remoteoutputs prove mechanics/boundaries only, not semantic accuracy/embedding model quality/cost/performance/platforms.','No realcredentials/user.env loaded, no network/API calls, no productioncodecopy/adoption.']}
p=R/'docs/research/dedup-500/semantic97-native-controls-20261016.json';assert not p.exists();p.write_text(json.dumps(report,indent=2)+'\n');print(json.dumps({'status':'verified','controls':36,'numpy':np.__version__}))
