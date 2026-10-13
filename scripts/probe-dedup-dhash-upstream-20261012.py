#!/usr/bin/env python3
"""Pinned upstream pure/Pillow controls; Wand is not substituted or mocked."""
import ast,doctest,hashlib,importlib.util,json,sys,unittest
from pathlib import Path
from PIL import Image,__version__ as pillow_version
R=Path(__file__).resolve().parents[1];D=R/'docs/research/dedup-500';m=json.loads((D/'dhash-full-source-20261012.json').read_text());s=Path(m['snapshot'])
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
assert all(sha(s/k)==h for k,h in m['files'].items())
sys.path.insert(0,str(s));import dhash
# Retain original class and helper code, removing only the unavailable Wand import.
module=ast.parse((s/'test.py').read_text());module.body=[node for node in module.body if not(isinstance(node,ast.Import)and any(a.name=='wand.image'for a in node.names))];scope={'__file__':str(s/'test.py')};exec(compile(module,str(s/'test.py'),'exec'),scope)
result=unittest.TextTestRunner(verbosity=2).run(unittest.TestSuite([scope['TestDHash']('test_get_grays_pil')]));assert result.wasSuccessful() and result.testsRun==1
checks=doctest.testmod(dhash,verbose=False);assert checks.failed==0
# P-mode transparency is represented by PNG's per-palette alpha table.
a=Image.new('P',(9,9),0);b=Image.new('P',(9,9),0);palette=[]
for i in range(256):palette.extend([i,i,i])
for im in [a,b]:im.putpalette(palette);im.info['transparency']=bytes(256)
b.putdata([x*20 for y in range(9)for x in range(9)])
assert list(a.convert('RGBA').getdata())!=list(b.convert('RGBA').getdata())
assert all(p[3]==0 for p in a.convert('RGBA').getdata()) and all(p[3]==0 for p in b.convert('RGBA').getdata())
ha,hb=dhash.dhash_int(a),dhash.dhash_int(b);assert ha!=hb
white=Image.new('RGB',(9,9),'white');black=Image.new('RGB',(9,9),'black');assert dhash.dhash_int(white)==dhash.dhash_int(black)==0
out=D/'dhash-upstream-controls-20261012.json';assert not out.exists();pins={str(p):sha(p)for p in [Path(__file__),D/'dhash-full-source-20261012.json',s/'test.py',s/'dhash.py',Path(sys.executable),Path(Image.__file__)]};report={'status':'verified_one_original_pillow_test_and_pure_doctests_plus_counterexamples','pins':pins,'python':sys.version,'pillow':pillow_version,'original_unittest_passed':1,'doctest_attempted':checks.attempted,'doctest_failed':checks.failed,'test_adapter':'Original test module AST with only unavailable import wand.image removed; only test_get_grays_pil selected. No mock Wand or source-file changes. Remaining two original tests not executed.','counterexamples':{'fully_invisible_palette_rgb_changes_hash':{'left':ha,'right':hb,'hamming':(ha^hb).bit_count()},'opaque_white_and_black_share_zero_hash':True},'scope':'Pillow and pure-list behavior of pinned source; full upstream suite failed import because Wand/native ImageMagick unavailable. No upstream backend parity/platform or rrrah full-source/runtime qualification.'};out.write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report,indent=2))
