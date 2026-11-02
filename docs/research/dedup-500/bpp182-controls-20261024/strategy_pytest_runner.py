"""Complete unchanged original strategy test module under real pytest.

Package namespaces/logger are adapters. Original full constants/common/hash/
cluster/semantic/strategy modules loaded; cosine embedding helper is a sentinel
that raises if called. No CLIP extraction/cosine path is qualified by these tests.
"""
import importlib.util,json,logging,sys,types
from pathlib import Path
import pytest
b=Path(__file__).resolve().parent;e=b.parent/'evidence/Arkalogy--best-photo-picker'
for name in ['bpp','bpp.dedupe','bpp.utils','bpp.scoring']:
 m=types.ModuleType(name);m.__path__=[];sys.modules[name]=m
log=types.ModuleType('bpp.utils.logging');log.get_logger=logging.getLogger;sys.modules[log.__name__]=log
embed=types.ModuleType('bpp.scoring.clip_embed')
def no_cosine(*a,**kw):raise AssertionError('Unqualified embedding helper called')
embed.cosine_similarity=no_cosine;sys.modules[embed.__name__]=embed
sources={
'bpp.constants':e/'remaining-pinned/bpp/constants.py',
'bpp.dedupe.common':e/'remaining-pinned/bpp/dedupe/common.py',
'bpp.dedupe.phash':e/'05-phash.py',
'bpp.dedupe.cluster':e/'remaining-pinned/bpp/dedupe/cluster.py',
'bpp.dedupe.semantic':e/'remaining-pinned/bpp/dedupe/semantic.py',
'bpp.dedupe.strategy':e/'remaining-pinned/bpp/dedupe/strategy.py'}
for name,path in sources.items():
 spec=importlib.util.spec_from_file_location(name,path);m=importlib.util.module_from_spec(spec);sys.modules[name]=m;spec.loader.exec_module(m)
results=[]
class Capture:
 def pytest_runtest_logreport(self,report):
  if report.when=='call' or report.failed:results.append({'node':report.nodeid,'when':report.when,'outcome':report.outcome})
code=pytest.main(['-q','--noconftest','-p','no:cacheprovider',str(e/'remaining-pinned/tests/test_dedupe_strategy_registry.py')],plugins=[Capture()]);p={'scope':__doc__,'pytest_exit_code':int(code),'passed':sum(x['when']=='call' and x['outcome']=='passed' for x in results),'failed':sum(x['outcome']=='failed' for x in results),'reports':results,'pytest_version':pytest.__version__};(b/'strategy-pytest-results.json').write_text(json.dumps(p,indent=2)+'\n');print(json.dumps(p,indent=2));assert code==0
