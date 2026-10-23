"""Complete-file structural parsing of bundled documentation assets; no DOM execution."""
import hashlib,json,pathlib,subprocess
import tinycss2
root=pathlib.Path(__file__).resolve().parents[3]
w=root/'.dedup-work/dedupe083-20261024'
tree=json.loads((root/'docs/research/dedup-500/evidence/dedupeio--dedupe/tree.json').read_text())['tree']
results=[]
for rel in ['docs/_static/js/bootstrap.min.js','docs/_static/js/jquery.min.js','docs/_static/css/bootstrap.css']:
 p=w/rel;b=p.read_bytes();entry=next(x for x in tree if x['path']==rel)
 assert hashlib.sha1(b'blob '+str(len(b)).encode()+b'\0'+b).hexdigest()==entry['sha']
 result={'path':rel,'bytes':len(b),'sha256':hashlib.sha256(b).hexdigest(),'manual_full_text_read':False}
 if p.suffix=='.js':
  run=subprocess.run(['node','--check',str(p)],capture_output=True,text=True)
  assert run.returncode==0,(rel,run.stderr)
  result.update(method='Entire original JS parsed with node --check; no execution',exit_code=run.returncode)
 else:
  rules=tinycss2.parse_stylesheet(b.decode('utf8'),skip_comments=True,skip_whitespace=True)
  errors=[];tokens=0
  def walk(items):
   global tokens
   for item in items:
    tokens+=1
    if item.type=='error':errors.append(str(item))
    for attr in ['prelude','content','arguments']:
     value=getattr(item,attr,None)
     if value is not None:walk(value)
  walk(rules);assert not errors,errors
  result.update(method='Entire CSS tokenized and stylesheet parsed with tinycss2, recursive syntax token walk',top_level_rules=len(rules),tokens_walked=tokens,syntax_errors=errors)
 results.append(result)
print(json.dumps({'assets':results,'limits':['Parsing is not manual whole-source review, runtime behavior, CSS semantic validation or browser rendering.','No asset copied into production or run against a live DOM.']},indent=2))
