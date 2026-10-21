import ast, io, json, types
from pathlib import Path
from lib2to3.refactor import RefactoringTool, get_fixers_from_package
source=Path('.dedup-work/quodlibetor234-20261021/dedupe.py').read_text()
converted=str(RefactoringTool(get_fixers_from_package('lib2to3.fixes')).refactor_string(source,'dedupe.py'))
# Python2 str bytes must become Python3 str at this formatting boundary only.
converted=converted.replace("return self.__unicode__().encode('utf-8')","return self.__unicode__()")
tree=ast.parse(converted)
selected=[n for n in tree.body if (isinstance(n,ast.ClassDef) and n.name in ['Song','NotASong']) or (isinstance(n,ast.FunctionDef) and n.name=='add_song') or (isinstance(n,ast.Assign) and any(isinstance(t,ast.Name) and t.id in ['valid_exts','wtags'] for t in n.targets))]
files={}
path=types.SimpleNamespace(exists=lambda _:True,isfile=lambda _:True,splitext=lambda p:(p.rsplit('.',1)[0],'.'+p.rsplit('.',1)[1]),basename=lambda p:p.rsplit('/',1)[-1])
env={'os':types.SimpleNamespace(path=path,isfile=lambda _:True),'sys':types.SimpleNamespace(stderr=io.StringIO()),'mutagen':types.SimpleNamespace(File=lambda p,easy:files[p])}
exec(compile(ast.Module(body=selected,type_ignores=[]),'original-selected','exec'),env)
def tagged(artist,album,title):return {'artist':artist,'album':[album],'title':[title],'length':1,'bitrate':128}
def scan(items):
 files.clear();files.update(items);songs={};dups={}
 for p,_ in items:env['add_song'](songs,dups,p)
 return songs,dups
assert len(scan([('a.mp3',tagged(['A'],'B','C')),('b.mp3',tagged(['X'],'B','C'))])[1])==0
print('PASS distinct ordinary metadata separated')
assert len(scan([('a.mp3',tagged(['A'],'B','C')),('b.flac',tagged(['A'],'B','C'))])[1])==1
print('PASS identical metadata groups different paths/types without audio content comparison')
assert len(scan([('a.mp3',tagged(['A - B'],'C','D')),('b.mp3',tagged(['A'],'B - C','D'))])[1])==1
print('PASS distinct field tuples collide in formatted artist-album-title key')
assert len(scan([('a.mp3',tagged(['A','X'],'B','C')),('b.mp3',tagged(['A','Y'],'B','C'))])[1])==1
print('PASS secondary artist values ignored')
assert len(scan([('a.mp3',tagged(['A'],'B','C')),('a.mp3',tagged(['A'],'B','C'))])[1])==1
print('PASS repeated same path counted as duplicate song')
missing=tagged(['A'],'B','C');del missing['artist']
assert scan([('a.mp3',missing)])[0]=={}
print('PASS missing artist skips candidate instead of content fallback')
print('TOTAL6passed; original selected class/add_song after2to3 andstr-boundary adaptation; synthetic mutagen/os only; no audio, walk, output writing, editor, deletion or CLI')
