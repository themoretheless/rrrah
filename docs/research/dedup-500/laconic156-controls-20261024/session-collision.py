# Replay the retained original module graph;do not repeat the bounded short collision search.
import sys,types,json
from pathlib import Path
root=Path(__file__).parent/'original'
for name,sub in [('laconic','laconic'),('laconic.compress','laconic/compress'),('laconic.tokenizers','laconic/tokenizers')]:
 p=types.ModuleType(name);p.__path__=[str(root/sub)];sys.modules[name]=p
from laconic.dedup.store import ContentStore,content_hash
from laconic.dedup.session import SessionDedup
from laconic.tokenizers.heuristic import HeuristicCounter
pair=None
for attempt in range(5):
 prefix='Owned '+str(attempt)+' '+('x'*400)+' '
 seen={}
 for i in range(1200000):
  text=prefix+str(i);h=content_hash(text)
  if h in seen:pair=(prefix+str(seen[h]),text,h);break
  seen[h]=i
 if pair is not None:break
assert pair is not None
first,second,digest=pair;store=ContentStore();s=SessionDedup(store=store,mode='store');counter=HeuristicCounter();assert s.process(first,recipient='owned',counter=counter).hits==0
old_reference=s.process(first,recipient='owned',counter=counter);assert old_reference.hits==1
new_reference=s.process(second,recipient='owned',counter=counter);assert new_reference.hits==1 and old_reference.text==new_reference.text
assert s.rehydrate_tool()('lc:'+digest)==second and second!=first
out={'controls_passed':1,'scope':'Original fullSessionDedup/store/segments/HeuristicCounter graph;actual40bit SHA256collision between long owned text blocks;no actualLLM/provider/server/tool installs','findings':['Two distinct sufficiently long blocks produce identical references;secondprocess overwrites store and earlier reference rehydrates secondblock.'],'collision':{'prefix':prefix,'first_suffix':first[len(prefix):],'second_suffix':second[len(prefix):],'digest':digest},'reference':old_reference.text};Path(__file__).with_name('session-results.json').write_text(json.dumps(out,indent=2)+'\n');print(json.dumps(out))
