"""Compare all logical preprocessor directives and function-macro adjacency."""
import pathlib,re,json,hashlib,difflib
root=pathlib.Path(__file__).resolve().parents[4]
paths=['.dedup-work/media459-20261021/stb_image.h','.dedup-work/libphash454-20261021/vendor/stb_image.h']
lex=re.compile(r'"(?:\\.|[^"\\])*"|\'(?:\\.|[^\'\\])*\'|/\*.*?\*/|//[^\n]*',re.S)
token=re.compile(r'"(?:\\.|[^"\\])*"|\'(?:\\.|[^\'\\])*\'|[A-Za-z_][A-Za-z_0-9]*|[0-9]+|##|\.\.\.|&&|\|\||<<|>>|<=|>=|==|!=|[^\s]')
streams=[];counts=[]
for path in paths:
 s=(root/path).read_text();s=re.sub(r'\\\n','',s)
 def strip(m):
  v=m.group();return (' '+ '\n'*v.count('\n')) if v.startswith(('/*','//')) else v
 s=lex.sub(strip,s)
 rows=[]
 for line in s.splitlines():
  if not re.match(r'^\s*#',line):continue
  ts=token.findall(line)
  if ts==['#','include','<','limits','.','h','>']:continue
  m=re.match(r'^\s*#\s*define\s+([A-Za-z_]\w*)(.*)$',line)
  adjacency=bool(m and m[2].startswith('('))
  rows.append([ts,adjacency])
 counts.append(len(rows));streams.append(rows)
if streams[0]!=streams[1]:
 for op,i,j,k,l in difflib.SequenceMatcher(None,list(map(str,streams[0])),list(map(str,streams[1])),autojunk=False).get_opcodes():
  if op!='equal': print(op,streams[0][i:j],streams[1][k:l])
 raise AssertionError('directives differ')
print(json.dumps({'paths':paths,'sha256':[hashlib.sha256((root/p).read_bytes()).hexdigest() for p in paths],'directives_excluding_one_limits_include':counts,'equal':True,'function_macro_adjacency_compared':True,'method':'Remove escaped-newline continuations, replace comments while preserving line boundaries and quoted strings, tokenize each directive; preserve define-name opening-parenthesis adjacency. All conditional branches included.','limits':['Whitespace within stringified macro arguments may alter diagnostic text; prior release-only compiler comparison remains scoped.','Not a compiler implementation or runtime cross-platform proof.']},indent=2))
