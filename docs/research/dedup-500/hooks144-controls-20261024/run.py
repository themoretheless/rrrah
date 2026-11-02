import types,sys,importlib.util,unittest,ast,difflib,logging,json
from pathlib import Path
b=Path('docs/research/dedup-500/evidence/mann1x--claude-hooks')
# Namespace packages bypass unrelated original package/provider initialization.
for name in ['claude_hooks','claude_hooks.providers']:
 p=types.ModuleType(name);p.__path__=[];sys.modules[name]=p
name='claude_hooks.providers._content_hash';spec=importlib.util.spec_from_file_location(name,b/'05-_content_hash.py');module=importlib.util.module_from_spec(spec);sys.modules[name]=module;spec.loader.exec_module(module)
spec=importlib.util.spec_from_file_location('original_content_hash_tests',b/'06-test_content_hash.py');tests=importlib.util.module_from_spec(spec);spec.loader.exec_module(tests);names=unittest.defaultTestLoader.getTestCaseNames(tests.TestContentHash);names=[n for n in names if n!='test_pgvector_alias_still_works'];assert len(names)==8;suite=unittest.TestSuite(tests.TestContentHash(n) for n in names);r=unittest.TextTestRunner(verbosity=2).run(suite);assert r.wasSuccessful()
tree=ast.parse((b/'03-dedup.py').read_text());funcs=[n for n in tree.body if isinstance(n,ast.FunctionDef)];ns={'difflib':difflib,'log':logging.getLogger('owned'),'Provider':object,'Optional':__import__('typing').Optional};exec(compile(ast.Module(body=funcs,type_ignores=[]),'original-dedup-functions','exec'),ns)
class P:
 name='owned'
 def recall(self,text,k):return[types.SimpleNamespace(text='a'*500+'different-old-tail')]
 def recall_vec(self,vec,k):return self.recall('',k)
p=P();assert ns['should_store']('a'*500+'different-new-tail',p) is False;assert ns['should_store']('a'*500+'different-new-tail',p,vec=[1.,0.]) is False
class Failure(P):
 def recall(self,*args,**kw):raise RuntimeError('owned provider failure')
assert ns['should_store']('new',Failure()) is True;assert ns['should_store']('   ',p) is False
result={'original_selected_tests_passed':8,'original_alias_test_not_run':'Requires original pgvector provider graph;not substituted','authored_controls_passed':4,'scope':'Original content_hash module and8 unchanged selected original tests;namespace package initialization bypass explicit.Original dedup functions AST;synthetic provider;no hooks installed,no realmemory/stores accessed.','findings':['Same first500 characters suppress different tails.','Precomputed full-content vector still uses500char final comparison.','Provider error returnsTrue failopen.','Whitespace-only content rejected.']};Path(__file__).with_name('results.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result))
