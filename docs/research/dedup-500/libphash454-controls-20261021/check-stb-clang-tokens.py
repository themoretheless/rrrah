"""Compare compiler tokens, including preprocessor semantics, for actual host configurations."""
import pathlib,subprocess,re,hashlib,json
root=pathlib.Path(__file__).resolve().parents[4]
paths=[root/'.dedup-work/media459-20261021/stb_image.h',root/'.dedup-work/libphash454-20261021/vendor/stb_image.h']
rows=[]
for definitions in [[],['STBI_NO_SIMD'],['STBI_NO_STDIO'],['STBI_ONLY_PNG'],['STBI_ONLY_JPEG'],['STBI_ONLY_PNM']]:
 streams=[]
 for path in paths:
  code='#define STB_IMAGE_IMPLEMENTATION\n'+''.join('#define '+name+'\n' for name in definitions)+path.read_text()
  run=subprocess.run(['cc','-DNDEBUG','-x','c','-','-fsyntax-only','-Xclang','-dump-tokens'],input=code,text=True,capture_output=True)
  assert run.returncode==0,run.stderr[-2000:]
  tokens=[]
  for line in run.stderr.splitlines():
   match=re.match(r"^(\S+) '(.*)'(?:\s|$)",line)
   if match:tokens.append(match.groups())
  assert len(tokens)>10000
  streams.append(json.dumps(tokens,separators=(',',':')).encode())

 if streams[0]!=streams[1]:
  left,right=map(json.loads,streams)
  first=next(i for i,(a,b) in enumerate(zip(left,right)) if a!=b)
  raise AssertionError((definitions,first,left[first:first+5],right[first:first+5]))
 rows.append({'definitions':definitions,'compiler_tokens':len(tokens),'sha256':hashlib.sha256(streams[0]).hexdigest(),'equal':True})
print(json.dumps({'rows':rows,'compiler':'AppleClang actual arm64 host','method':'Same stdin translation-unit name, originalheadertexts, STB_IMAGE_IMPLEMENTATION; compiler preprocessor+lexer tokens kind/spelling, whitespace/locationflags ignored; NDEBUG disables assert diagnostic __LINE__ and stringification differences.','source_sha256':[hashlib.sha256(p.read_bytes()).hexdigest() for p in paths],'limits':['Six actualhost release/NDEBUG configurations only; debug assertions and inactiveplatform branches not compiler-qualified','Comment prose changes not fully reviewed by this check','Not runtimecodec or wholevendorcompletion proof']},indent=2))
