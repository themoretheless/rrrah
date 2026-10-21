"""Authored synthetic controls loading unchanged pinned original Python functions."""
from pathlib import Path
import sys, contextlib, io
sys.path.insert(0,str(Path('.dedup-work/bookmarks169-20261021').resolve()))
from firefox_bookmarks_deduplicator import deduplicator_utils as u
B='text/x-moz-place';F='text/x-moz-place-container'
def b(url,**kw):return dict(type=B,uri=url,**kw)
def f(name,i,children):return dict(type=F,title=name,id=i,children=children)
with contextlib.redirect_stdout(io.StringIO()):
 x=[b('same'),b('same'),b('distinct')];u.deduplicate(x)
 assert [v['uri'] for v in x]==['same']
 print_result='missing IDs: unrelated distinct bookmark removed'
 y=[f('left',1,[f('shared',2,[b('one',id=10)])]),f('right',3,[f('SHARED',4,[b('two',id=11)])])];u.directory_merge(y)
 assert len(y[0]['children'][0]['children'])==2 and y[1]['children']==[]
 z=[b('same',id=1),b('same',id=2),b('distinct',id=3)];u.deduplicate(z)
 assert [v['id'] for v in z]==[1,3]
print('PASS '+print_result)
print('PASS case-insensitive same-name folders merge across distinct parents')
print('PASS distinct-ID positive dedup retains unrelated bookmark')
