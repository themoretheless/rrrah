import subprocess,re,difflib,pathlib,json,hashlib
root=pathlib.Path(__file__).resolve().parents[4]
streams=[]
for p in ['.dedup-work/media459-20261021/stb_image.h','.dedup-work/libphash454-20261021/vendor/stb_image.h']:
 r=subprocess.run(['cc','-x','c','-fsyntax-only','-Xclang','-dump-raw-tokens',str(root/p)],capture_output=True,text=True);assert r.returncode==0
 rows=[]
 for record in re.split(r"\tLoc=<[^>]+>\n",r.stderr):
  if not record:continue
  m=re.match(r"^(\w+) '(.*)'\t",record,re.S)
  assert m,repr(record[:100])
  rows.append(m.groups())
 tokens=[];comments=[]
 for k,v in rows:
  if k=='unknown':
   assert not v.strip(),repr(v)
  elif k=='comment':comments.append(' '.join(v.split()))
  else:tokens.append((k,v))
 streams.append((tokens,comments))

include=[('hash','#'),('raw_identifier','include'),('less','<'),('raw_identifier','limits'),('period','.'),('raw_identifier','h'),('greater','>')]
normalized=[]
for tokens,comments in streams:
 hits=[i for i in range(len(tokens)) if tokens[i:i+7]==include]
 assert len(hits)==1,hits
 i=hits[0];normalized.append(tokens[:i]+tokens[i+7:])
assert normalized[0]==normalized[1]
def prose(comments):
 return re.sub(r'\s+','', ''.join(c[2:] if c.startswith('//') else c for c in comments))
assert prose(streams[0][1])==prose(streams[1][1])
print(json.dumps({'raw_token_counts':[len(s[0]) for s in streams],'comment_counts':[len(s[1]) for s in streams],'all_branch_tokens_equal_after_single_limits_include_removal':True,'comment_prose_equal_after_line_comment_markers_and_whitespace_removed':True,'normalized_tokens_sha256':hashlib.sha256(json.dumps(normalized[0]).encode()).hexdigest(),'limits':['Raw lexer includes inactive branches but ignores whitespace and directive boundaries; not a preprocessor equivalence proof.','Comment normalization ignores whitespace and comment line boundaries.','No runtime decode, debug diagnostic, external codec or cross-platform qualification.']},indent=2))
