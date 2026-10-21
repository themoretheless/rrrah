"""Original function AST with synthetic paths inside an owned TemporaryDirectory."""
import ast,os,shutil,tempfile
from pathlib import Path
from types import SimpleNamespace as N
R=Path(__file__).resolve().parents[4];C=R/'.dedup-work/duperender348-20261021'
ns={'os':os,'shutil':shutil,'persistent':lambda fn:fn}
for name in ['handler.py','remove_placeholders_operator.py']:
 t=ast.parse((C/name).read_text());t.body=[n for n in t.body if isinstance(n,ast.FunctionDef)]
 exec(compile(t,name,'exec'),ns)
with tempfile.TemporaryDirectory(prefix='duperender-control-') as d:
 path=lambda f:Path(d)/f'{f:04}.png'
 scene=N(frame_start=1,frame_end=2,duperender_properties=N(dupelist='2',frame_start=1,frame_end=2),
 render=N(frame_path=lambda frame:str(path(frame))))
 path(2).touch();ns['replace_placeholders'](scene)
 assert not path(2).exists()
 print('PASS missing original is checked only after empty destination was removed')
 path(1).write_bytes(b'original');path(2).write_bytes(b'existing output')
 ns['replace_placeholders'](scene);assert path(2).read_bytes()==b'existing output'
 print('PASS nonempty existing output is preserved')
 path(2).write_bytes(b'');ns['replace_placeholders'](scene)
 assert path(2).read_bytes()==b'original'
 print('PASS successful replacement copies original bytes')
 scene.duperender_properties.dupelist=''
 try:ns['create_placeholders'](scene)
 except ValueError:print('PASS direct placeholder creation rejects empty list via unchecked int conversion')
 else:raise AssertionError('Expected empty-list conversion failure')
 path(2).write_bytes(b'');assert ns['remove_single_placeholder'](scene,2) is True
 assert not path(2).exists()
 print('PASS zero-byte file accepted as placeholder without ownership metadata')
print('5 controls passed; only owned synthetic temporary files touched; no Blender or actual rendered images')
