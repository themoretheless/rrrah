"""Unchanged selected Python AST; no imports/main/FFmpeg/ImageMagick/Pascalbinary execution."""
from pathlib import Path
import ast,types,time,os,sys,tempfile,pickle,collections,json
R=Path(__file__).resolve().parents[3];D=Path(__file__).parent;W=R/'.dedup-work/video211-20261015';E=D/'evidence/PierreCrette--VideoDedup';passed=[]
def select(file,names,ns):
 tree=ast.parse(file.read_text());nodes=[x for x in tree.body if isinstance(x,ast.FunctionDef) and x.name in names];assert len(nodes)==len(names);exec(compile(ast.Module(body=nodes,type_ignores=[]),str(file),'exec'),ns);return types.SimpleNamespace(**ns),tree
base=dict(os=os,sys=sys,time=time,log=lambda *a,**k:None,perf=time.perf_counter(),debug=0,txtgreen='',txtnocolor='',txterr='',foldervideo='/source/',folderimg='/images/',gmpy2=types.SimpleNamespace(hamdist=lambda a,b:(int(a)^int(b)).bit_count()),threshold=1,ctrlref=True,imglst=[],srclst=[])
n=dict(base);m,tree=select(E/'03-3n_analyse.py',{'ShortName','MidName','TempName','PathName','source','newimage','sortoccurence','sort1','sort2','mp2_SourceControl','mp3_HD_ReadCache','mp5_HD_DistanceControl'},n)
p=dict(base,srclst=[],renamed=False,cpttodo=0,cptdone=0,mpdata=[],jobcounter=0,parallel=False,clean=False);q,parser=select(E/'02-1k_parse.py',{'sortoccurence','duration','SameSrcNames','BoucleFichiers','OneFile'},p)
def check(name,fn):
 with tempfile.TemporaryDirectory(dir=W) as temp:fn(Path(temp))
 passed.append(name);print('PASS',name,flush=True)
def empty_short(t):
 try:m.ShortName('');assert False
 except IndexError:pass
check('empty ShortName raises IndexError',empty_short)
def root_short(t):assert m.ShortName('/a')=='/a'
check('single-component absolute ShortName retains slash',root_short)
def basename_path(t):
 try:m.PathName('plain');assert False
 except IndexError:pass
check('PathName without separator raises IndexError',basename_path)
def temp_collision(t):assert m.TempName('/foo/image.jpg')==m.TempName('/fox/image.jpg')=='fo_image.jpg'
check('TempName truncates parent and collides distinct directories',temp_collision)
def empty_map(t):
 n['imglst']=[]
 try:m.newimage('video.mp4/frame.jpg');assert False
 except IndexError:pass
check('empty image lookup map raises IndexError',empty_map)
def stale(t):
 f=t/'frame.jpg';f.write_bytes(b'old');name='video.mp4/frame.jpg';n['imglst']=[['video.mp4',str(t)]];assert m.mp3_HD_ReadCache(([name],[name]))==[];f.write_bytes(b'new');assert m.mp3_HD_ReadCache(([name],[name]))==[]
check('existing name-only HD cache skips changed source bytes',stale)
def rec(a,b):return [1,[a,b],[a+'/f1.jpg',b+'/f1.jpg'],[],[],0]
def greedy(t):
 n.update(threshold=1,ctrlref=True,srclst=[['A',''],['B',''],['C','']]);r=m.mp2_SourceControl(([rec('A','B'),rec('A','C')],[]));assert len(r[0])==1 and r[2]==1
check('withinbatch reference policy greedily suppresses second valid pair',greedy)
def batch_isolation(t):
 n.update(threshold=1,ctrlref=True,srclst=[['A',''],['B',''],['C','']]);a=m.mp2_SourceControl(pickle.loads(pickle.dumps(([rec('A','B')],[]))));b=m.mp2_SourceControl(pickle.loads(pickle.dumps(([rec('A','C')],[]))));assert len(a[0])==len(b[0])==1
check('independently serialized batches do not share prior reference exclusion',batch_isolation)
def hd_fixture(values,threshold=1,maxdiff=0):
 names=['A.mp4/f1.jpg','A.mp4/f2.jpg','B.mp4/f1.jpg','B.mp4/f2.jpg'];n.update(threshold=threshold,hdmaxdiff=maxdiff,hdcache=sorted(zip(names,values)),resultsetvideo=[[2,['A.mp4','B.mp4'],names,[],[],0]])
 return m.mp5_HD_DistanceControl([0])
def reversed_frames(t):
 a=hd_fixture([0,1,0,1],2);b=hd_fixture([0,1,1,0],2);assert a[0] and b[0] and all(row[0]==0 for row in b[0][1])
check('reversed fingerprint frame order still admitted with two zero-distance matches',reversed_frames)
def average_limit(t):
 n.update(threshold=1,hdmaxdiff=10,hdcache=[['A.mp4/f1.jpg',0],['A.mp4/f2.jpg',(1<<20)-1],['B.mp4/f1.jpg',0]],resultsetvideo=[[2,['A.mp4','B.mp4'],['A.mp4/f1.jpg','A.mp4/f2.jpg','B.mp4/f1.jpg'],[],[],0]])
 r=m.mp5_HD_DistanceControl([0]);assert r[0] and [x[0] for x in r[0][1]]==[0,20]
check('HD policy is average distance10 allowing individualdistance20',average_limit)
def rename_clobber(t):
 a,b=t/'one',t/'two';a.mkdir();b.mkdir();(a/'clip.mp4').write_bytes(b'first');(b/'clip.mp4').write_bytes(b'second');marker=a/'0clip.mp4';marker.write_bytes(b'owned existing marker');p.update(srclst=[['clip.mp4',str(a)+'/', ''],['clip.mp4',str(b)+'/', '']],renamed=False);q.SameSrcNames();assert marker.read_bytes()==b'first' and not(a/'clip.mp4').exists()
check('SameSrcNames rename overwrites preexisting prefixed owned file',rename_clobber)
def empty_sources(t):
 p['srclst']=[]
 try:q.SameSrcNames();assert False
 except IndexError:pass
check('SameSrcNames empty source list raises IndexError',empty_sources)
def single_file_api(t):
 try:q.BoucleFichiers(str(t/'absent-file'),str(t/'images'));assert False
 except TypeError as e:assert 'file' in str(e)
check('single-file BoucleFichiers calls OneFile with missing argument',single_file_api)
def chunkdup(t):
 found=None
 def visit(body):
  global unused
  nonlocal found
  for i,node in enumerate(body):
   if isinstance(node,ast.Assign) and any(isinstance(x,ast.Name) and x.id=='loopcount' for x in node.targets) and isinstance(node.value,ast.BinOp) and isinstance(node.value.right,ast.Constant) and node.value.right.value==5000:
    end=next(j for j in range(i+1,len(body)) if isinstance(body[j],ast.Assign) and any(isinstance(x,ast.Name) and x.id=='mpdatalist' for x in body[j].targets));found=body[i:end];return
   for field in ['body','orelse','finalbody']:
    if isinstance(getattr(node,field,None),list):visit(getattr(node,field))
 visit(tree.body);assert found
 for length in [4999,5000,5001,10000,10001,15000]:
  ns={'mpdatalist':list(range(length))};exec(compile(ast.Module(body=found,type_ignores=[]),'unchanged-original-batch-block','exec'),ns);flat=[x for rows,count in ns['mpdata'] for x in rows];counts=collections.Counter(flat);assert set(flat)==set(range(length));assert len(flat)==length+(5000 if length>=5000 else 0);assert max(counts.values())==(2 if length>=5000 else 1)
check('original batch block repeats5000records at and above5000boundary',chunkdup)
print(json.dumps(dict(passed=len(passed),failed=0,scope='17selected unchanged Pythonfunction ASTs plus actual selected batching statements, real owned file rename and path IO; explicit nonnegative integer xor.bit_count gmpy2 adapter and pickle batch isolation. No imported source/main, FFmpeg/ImageMagick/extraction/Pascalcompile/binary/threadpool/network/corpus/performance qualification.')))
