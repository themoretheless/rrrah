import ast
from pathlib import Path
root=Path('.dedup-work/fourhash424-20261021')
def selected(path):
 t=ast.parse((root/path).read_text());cls=next(x for x in t.body if isinstance(x,ast.ClassDef));ns={};exec(compile(ast.Module(body=[cls],type_ignores=[]),path,'exec'),ns);return ns[cls.name]
H=selected('hamming/hamming.py');h=H()
assert h.distance(h.transform('0'),h.transform('ffffffffffffffff'))==64
print('PASS original packaged Hamming full64bit distance')
assert h.distance('0','00') is False and h.distance('0','00') <=13
print('PASS mismatched lengths return False, numerically accepted by <=13 threshold')
a=h.transform('0');b=h.transform('10000000000000000');assert len(a)==64 and len(b)==65 and h.distance(a,b) is False
print('PASS oversized hex remains65bits; mismatch sentinel can enter match branch')
Broken=selected('hamming.py')
try:Broken('0','1').distance()
except NameError:print('PASS alternate root Hamming class references undefined a/b')
else:raise AssertionError('expected source NameError')
try:ast.parse((root/'similarity.py').read_text())
except IndentationError:print('PASS similarity.py cannot parse because methods have no body')
else:raise AssertionError('expected incomplete source syntax')
print('RESULT 5 controls passed; selected original classes and syntax parse; no database, video decode or script entrypoints')
