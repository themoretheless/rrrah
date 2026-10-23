import ast, hashlib, json, sys, zipfile
from pathlib import Path
base=Path.cwd()/'.dedup-work'
src=base/'intra095-20261024'
sys.path.insert(0,str(src))
from scanner.hashFile import getHashDict
fixture=src/'Tests/baseArchiveTestSetup.py'
tree=ast.parse(fixture.read_text())
rows=ast.literal_eval(next(n.value for n in tree.body if isinstance(n,ast.Assign) and any(isinstance(t,ast.Name) and t.id=='CONTENTS' for t in n.targets)))
checks=[]
for row in rows:
 ident, archive, member, md5, phash, kind, width, height=row
 p=src/'Tests/test_ptree_base'/Path(archive).name
 if member:
  with zipfile.ZipFile(p) as z: data=z.read(member)
  info=getHashDict(member,data)
  actual=[info['hexHash'],info['pHash'],None,info['imX'],info['imY']]
 else:
  actual=[hashlib.md5(p.read_bytes()).hexdigest(),None,None,None,None]
 expected=[md5,phash,kind,width,height]
 checks.append(dict(id=ident,archive=p.name,member=member,expected=expected,actual=actual,matches=actual==expected))
result=dict(fixture_sha256=hashlib.sha256(fixture.read_bytes()).hexdigest(),rows=len(rows),unique_ids=len({r[0] for r in rows}),archives=len({r[1] for r in rows}),envelopes=sum(not r[2] for r in rows),members=sum(bool(r[2]) for r in rows),matched=sum(c['matches'] for c in checks),checks=checks,scope='AST literal extraction without fixture import; original getHashDict plus zipfile and envelope MD5; kind None is a placeholder, not MIME verification; no DB or original test invocation; not whole source review')
out=base.parent/'docs/research/dedup-500/intra095-row-fixture-control-20261024.json'
out.write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps({k:v for k,v in result.items() if k!='checks'}))
for c in checks:
 if not c['matches']: print(json.dumps(c))
assert result['matched']==result['rows']
