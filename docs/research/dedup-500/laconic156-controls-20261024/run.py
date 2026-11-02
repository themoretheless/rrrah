import sys,types,importlib.util,inspect,json
from pathlib import Path
b=Path('docs/research/dedup-500/evidence/lindameng-0--laconic');root=Path(__file__).parent/'original'
files={'laconic/dedup/__init__.py':'02-__init__.py','laconic/dedup/session.py':'03-session.py','laconic/dedup/store.py':'04-store.py','tests/test_dedup.py':'05-test_dedup.py','laconic/compress/segments.py':'deep-laconic_compress_segments.py','laconic/tokenizers/base.py':'deep-laconic_tokenizers_base.py','laconic/tokenizers/heuristic.py':'deep-laconic_tokenizers_heuristic.py','laconic/exceptions.py':'deep-laconic_exceptions.py'}
for path,local in files.items():
 p=root/path;p.parent.mkdir(parents=True,exist_ok=True);p.write_bytes((b/local).read_bytes())
for name,sub in [('laconic','laconic'),('laconic.compress','laconic/compress'),('laconic.tokenizers','laconic/tokenizers')]:
 module=types.ModuleType(name);module.__path__=[str(root/sub)];sys.modules[name]=module
from laconic.dedup.store import ContentStore,content_hash
from laconic.tokenizers.heuristic import HeuristicCounter
spec=importlib.util.spec_from_file_location('original_dedup_tests',root/'tests/test_dedup.py');tests=importlib.util.module_from_spec(spec);spec.loader.exec_module(tests);passed=[]
for name,fn in vars(tests).items():
 if name.startswith('test_') and inspect.isfunction(fn):
  fn(**({'counter':HeuristicCounter()} if 'counter' in inspect.signature(fn).parameters else {}));passed.append(name)
assert len(passed)==12
seen={};pair=None
for i in range(2000000):
 text='owned-block-'+str(i);digest=content_hash(text)
 if digest in seen:pair=('owned-block-'+str(seen[digest]),text,digest);break
 seen[digest]=i
assert pair is not None,'no collision in bounded search'
first,second,digest=pair;assert first!=second;store=ContentStore();h=store.put(first);assert store.get(h)==first;assert store.put(second)==h;assert store.get(h)==second and store.get(h)!=first
# Handle prefix isn't enforced by the underlying store.
assert store.get('other:'+digest)==second
result={'original_test_bodies_passed':len(passed),'original_test_names':passed,'authored_controls_passed':2,'collision':{'first':first,'second':second,'digest40bit':digest},'scope':'Unchanged original session/store/segments/counter/exception modules;12 original test function bodies replayed with actual HeuristicCounter;namespaceinit bypass explicit,not fullpytest/CLI suite;owned synthetic strings only','findings':['Actual SHA256 prefix40bit collision overwrites original text under samehandle;rehydration returns other content.','Store accepts arbitrary handleprefix with same digest.']};Path(__file__).with_name('results.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result))
