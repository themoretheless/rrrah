"""Run unchanged pinned es-dedupe with real local dependencies; synthetic ES boundary only."""
from pathlib import Path
import sys, json, hashlib, contextlib, io, datetime, importlib, importlib.metadata
R=Path(__file__).resolve().parents[1]; W=R/'.dedup-work/es113-20261016'
sys.path[:0]=[str(W/'source'),str(W/'python-deps')]
import requests
def no_network(*a,**k): raise AssertionError('Network prohibited in source controls')
requests.sessions.Session.request=no_network
import esdedupe.esdedupe as m
from esdedupe.cli import ArgumentParser
from esdedupe.utils import time_to_sec,to_es_date
from types import SimpleNamespace
def args(**k):
 with contextlib.redirect_stdout(io.StringIO()): a=ArgumentParser().parse_args(['--no-progress'])
 for n,v in k.items():setattr(a,n,v)
 return a
def groups(values,fields):
 d={};e=m.Esdedupe()
 for i,v in enumerate(values):e.build_index(d,fields,{'_id':str(i),'_source':v})
 return d
def check(name,fn,expected,rows):
 try: value=fn()
 except Exception as e:value={'exception':type(e).__name__}
 assert value==expected,(name,value,expected)
 rows.append({'name':name,'observed':value,'expected':expected})
def windows(minutes):
 # SDK scan provider records queries and yields no hits. All original methods remain unchanged.
 queries=[];original=m.helpers.scan
 def scan(*a,**k):queries.append(k['query']);return iter(())
 m.helpers.scan=scan
 try:
  start=datetime.datetime(2021,1,1)
  m.Esdedupe().process_index(None,{},['name'],0,'owned',args(window='5m',timestamp='t',since=start,until=start+datetime.timedelta(minutes=minutes)))
 finally:m.helpers.scan=original
 return len(queries)
def deletes(parallel=False):
 e=m.Esdedupe();key='parallel_bulk' if parallel else 'streaming_bulk';original=getattr(m,key);actions=[]
 def bulk(es,iterator,**kwargs):
  for action in iterator:actions.append(action);yield True,{'delete':{'_shards':{'successful':2}}}
 setattr(m,key,bulk)
 try:
  count=(e.parallel_delete if parallel else e.sequential_delete)({'same':['first','second','third']},'owned',None,args(),2)
 finally:setattr(m,key,original)
 return [count,[a['_id'] for a in actions]]
def swallowed():
 def gen():yield 'first';raise RuntimeError('owned synthetic interruption')
 return list(m.Esdedupe().wrapper(gen()))
def parsed_secret():
 out=io.StringIO()
 with contextlib.redirect_stdout(out):ArgumentParser().parse_args(['--password','OWNED_LITERAL_TEST_SECRET'])
 return 'OWNED_LITERAL_TEST_SECRET' in out.getvalue()
def second_parse():
 p=ArgumentParser()
 with contextlib.redirect_stdout(io.StringIO()):p.parse_args([]);p.parse_args([])
def run():
 rows=[]
 check('ambiguous_field_concatenation',lambda:len(groups([{'a':'ab','b':'c'},{'a':'a','b':'bc'}],['a','b'])),1,rows)
 check('integer_string_coercion',lambda:len(groups([{'a':1},{'a':'1'}],['a'])),1,rows)
 check('none_string_coercion',lambda:len(groups([{'a':None},{'a':'None'}],['a'])),1,rows)
 check('single_field_distinct_positive',lambda:len(groups([{'a':'ab'},{'a':'abc'}],['a'])),2,rows)
 check('dotted_key_real_benedict',lambda:len(groups([{'a':{'b':'x'}},{'a':{'b':'x'}}],['a.b'])),1,rows)
 check('missing_field',lambda:groups([{'other':'x'}],['a']),{'exception':'KeyError'},rows)
 check('one_exact_window_omitted',lambda:windows(5),0,rows)
 check('two_exact_windows_last_omitted',lambda:windows(10),1,rows)
 check('partial_tail_positive',lambda:windows(11),3,rows)
 check('short_window_positive',lambda:windows(2),1,rows)
 check('empty_interval_positive',lambda:windows(0),1,rows)
 check('zero_window_accepted_no_loop_run',lambda:time_to_sec('0m'),0,rows)
 check('negative_window_accepted_no_loop_run',lambda:time_to_sec('-1m'),-60,rows)
 check('offset_formatted_as_utc_without_conversion',lambda:to_es_date(datetime.datetime(2021,1,1,12,30,0,123456,tzinfo=datetime.timezone(datetime.timedelta(hours=5)))),'2021-01-01T12:30:00.000Z',rows)
 check('non_timestamp_query_is_none',lambda:m.Esdedupe().es_query(args()),None,rows)
 check('delete_preserves_first_scan_order',lambda:list(m.Esdedupe().delete_iterator({'same':['newest','oldest']},'owned',args())),[{'_op_type':'delete','_index':'owned','_id':'oldest'}],rows)
 check('sequential_counts_replicas',lambda:deletes(),[4,['second','third']],rows)
 check('parallel_counts_replicas',lambda:deletes(True),[4,['second','third']],rows)
 check('wrapper_swallows_generator_error',swallowed,['first'],rows)
 check('host_already_port_double_appended',lambda:m.Esdedupe().elastic_uri(args(host='http://localhost:9200')),'http://localhost:9200:9200',rows)
 check('cli_echoes_owned_literal_password',parsed_secret,True,rows)
 check('parser_reuse_conflict',second_parse,{'exception':'ArgumentError'},rows)
 return rows
result=[run(),run()];assert result[0]==result[1]
h=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
deps=[]
for name in ['ujson','benedict','psutil','requests','tqdm','elasticsearch','urllib3']:
 mod=importlib.import_module(name);p=Path(mod.__file__)
 deps.append({'module':name,'version':str(getattr(mod,'__version__',getattr(mod,'VERSION','not exposed'))),'path':str(p.relative_to(R)),'sha256':h(p)})
p=R/'docs/research/dedup-500/es113-boundary-controls-20261016.json';assert not p.exists()
p.write_text(json.dumps({'status':'verified_selected_native_boundaries','runs':2,'cases_per_run':len(result[0]),'executions':sum(map(len,result)),'rows':result,'producer_sha256':h(Path(__file__)),'python':sys.version,'dependencies':deps,'scope':'Whole unchanged esdedupe.py/cli.py/utils.py imported with real benedict/ujson/requests/Elasticsearch8 SDK dependencies. Only helpers.scan and module bulk entrypoints temporarily replaced by owned empty-query recorder and success-response generators, restored finally. HTTP session request blocked before source imports. No function extraction or algorithm changes.','limits':['Original8integrationtests require actualElasticsearch and were read not executed','No real cluster/delete/network/SDK bulk integration, package install, platform/scale/performance/imagequality/currentroot proof','Dependency versions resolved now, no historical environment parity','Zero/negative window conversion tested without executing potentially unbounded loop']},indent=2)+'\n')
print(json.dumps({'status':'passed','cases':len(result[0]),'runs':2}))
