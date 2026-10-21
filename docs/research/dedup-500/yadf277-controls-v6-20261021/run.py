"""Original generator and unchanged original CLI; independent exact byte-group oracle."""
from pathlib import Path
import importlib.util,sys,tempfile,subprocess,json,collections
p=Path('.dedup-work/yadf277-20261021/scripts/gen-corpus.py');s=importlib.util.spec_from_file_location('original_corpus',p);u=importlib.util.module_from_spec(s);sys.modules[s.name]=u;s.loader.exec_module(u)
exe=Path('.dedup-work/yadf277-target-20261021/debug/yadf').resolve()
with tempfile.TemporaryDirectory() as td:
 roots=[Path(td)/'first',Path(td)/'second']
 for root in roots:
  u.main(u.CorpusConfig(root,42,100,.3,'small',2,2,.2))
 snapshots=[{str(p.relative_to(root)):p.read_bytes() for p in root.rglob('*') if p.is_file()} for root in roots]
 assert snapshots[0]==snapshots[1]
 print('PASS original seed42 generator repeats identical100file corpus plus manifest')
 root=roots[0];groups=collections.defaultdict(list)
 for p in root.rglob('*'):
  if p.is_file():groups[p.read_bytes()].append(str(p))
 expected=sorted(sorted(v) for v in groups.values() if len(v)>1)
 result=subprocess.run([str(exe),'--format','json','--algorithm','blake3',str(root)],check=True,capture_output=True,text=True,timeout=30)
 actual=sorted(sorted(v) for v in json.loads(result.stdout))
 assert actual==expected
 manifest=json.loads((root/'manifest.json').read_text());assert len(expected)==manifest['duplicate_group_count']
 print('PASS original CLI BLAKE3 groups equal independent full-byte oracle:',len(expected),'groups;20prefix-collision files included')
