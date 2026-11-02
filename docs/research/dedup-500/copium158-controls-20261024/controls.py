import ast,sys,types,importlib.util,inspect,json,random
from pathlib import Path
b=Path('docs/research/dedup-500/evidence/iKislay--copium')
name='owned_mcp_session';spec=importlib.util.spec_from_file_location(name,b/'04-session_dedup.py');mcp=importlib.util.module_from_spec(spec);sys.modules[name]=mcp;spec.loader.exec_module(mcp)
t=ast.parse((b/'deep-test_mcp_proxy_dedup_cache.py').read_text());cls=next(n for n in t.body if isinstance(n,ast.ClassDef) and n.name=='TestSessionDedup');ns={'SessionDedup':mcp.SessionDedup};exec(compile(ast.Module(body=[cls],type_ignores=[]),'original-MCPtests','exec'),ns);passed=[]
for n,fn in vars(ns['TestSessionDedup']).items():
 if n.startswith('test_'):
  instance=ns['TestSessionDedup']();instance.setup_method();getattr(instance,n)();passed.append(n)
assert len(passed)==9
s=mcp.SessionDedup(max_entries=1)
for i in range(3):s.check_definitions([{'name':str(i)}])
assert s.stats['definitions_cached']==3
for i in range(3):s.cache_schema(str(i),{'type':'object'})
assert s.stats['schemas_cached']==3
s.check_call('owned',{},'OK');marker=s.check_call('owned',{},'OK');assert s._estimate_tokens(marker)>s._estimate_tokens('OK')
# Original full transform body;replace only external base/result/tokenizer transport types.
t=ast.parse((b/'05-session_dedup.py').read_text());t.body=[n for n in t.body if not(isinstance(n,ast.ImportFrom) and n.level>0)]
module=types.ModuleType('owned_transform');module.Transform=object;module.TransformResult=types.SimpleNamespace;module.Tokenizer=object;sys.modules[module.__name__]=module;exec(compile(t,'original-transform','exec'),module.__dict__)
class Counter:
 def count_messages(self,messages):return sum(len(str(m.get('content',''))) for m in messages)
text='Owned synthetic output content with unchanged details. '*20
tr=module.SessionDedup(module.SessionDedupConfig(minhash_enabled=False));message={'role':'tool','name':'Read','content':text};first=tr.apply([message],Counter());second=tr.apply([message],Counter());assert first.messages[0]['content']==text and second.messages[0]['content']!=text and 'turn 0' in second.messages[0]['content']
assert module._content_hash('    '+text)==module._content_hash(text) and '    '+text!=text
random.seed(765);before=random.getstate();module._minhash_signature(text,8);assert random.getstate()!=before
out={'original_test_bodies_passed':9,'authored_controls_passed':6,'scope':'Unchanged original MCPmodule and9 selected original test bodies.Original fulltransform with explicit external base/result/tokenizer type adapters and character-count tokenizer fixture;no actualMCP or provider/context calls,no token savings claim.','findings':['max_entries1 still retains3 tool-definition sets.','max_entries1 still retains3 schemas.','Short OK result replaced by marker with larger original estimatedtoken count.','Default repeatedapply of one-message history replaces sole original fulltext with marker pointing toturn0;actual host prefix/frozen lifecycle unqualified.','Exact hash strips leading whitespace as well as trailing;raw-distinct content equated.','MinHash helper seeds/changes process-global random state.']};Path(__file__).with_name('results.json').write_text(json.dumps(out,indent=2)+'\n');print(json.dumps(out))
