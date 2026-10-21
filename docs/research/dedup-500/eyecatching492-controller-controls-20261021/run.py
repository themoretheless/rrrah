import sys,ast,tempfile,os,io,time,json,contextlib,hashlib
from pathlib import Path
sys.path[:0]=[str(Path('.dedup-work/eyecatching492-20261021').resolve()),str(Path('.dedup-work/mediahash478-extra-deps-20261021').resolve())]
from PIL import Image
from eyecatchingutil import MetaImage,Coordinates,ImageComparator
source=Path('.dedup-work/eyecatching492-20261021/controller.py').read_text();tree=ast.parse(source);node=next(n for n in tree.body if isinstance(n,ast.ClassDef) and n.name=='Controller');ns=dict(Image=Image,MetaImage=MetaImage,Coordinates=Coordinates,ImageComparator=ImageComparator,time=time);exec(compile(ast.Module(body=[node],type_ignores=[]),'<original-Controller>','exec'),ns);Controller=ns['Controller'];out={};old=os.getcwd()
with tempfile.TemporaryDirectory() as tmp:
 try:
  os.chdir(tmp)
  Image.new('RGB',(8,4),'red').save('a.png');Image.new('RGB',(4,8),'blue').save('b.png');before=hashlib.sha256(Path('a.png').read_bytes()).hexdigest();Controller().normalize_images('a.png','b.png');sizes=[list(Image.open(n).size) for n in ['a.png','b.png']];assert sizes==[[8,8],[4,8]];assert hashlib.sha256(Path('a.png').read_bytes()).hexdigest()!=before
  out['crossed_dimensions']={'sizes_after_normalize':sizes,'a_input_overwritten':True,'equal_sizes':False}
  Image.new('RGB',(3,3),'white').save('c.png');Image.new('RGB',(3,3),'white').save('d.png');c=Controller();c.block_size=4;c.threshold=0;log=io.StringIO()
  with contextlib.redirect_stdout(log):c.linear('c.png','d.png')
  assert 'Dissimilar area: 177.78%' in log.getvalue();out['edge_area_overcount']={'identical_images':True,'block_size':4,'image_size':[3,3],'threshold':0,'reported_dissimilar_area_percent':177.78,'stdout':log.getvalue()}
 finally:os.chdir(old)
out['scope']='Original Controller class compiled unchanged from AST; actual utility/Pillow/ImageHash. Unused cv2/pandas imports bypassed; no browser or video/shift backend invoked. All file writes in temporary fixture directory.'
Path(__file__).with_name('result.json').write_text(json.dumps(out,indent=2)+'\n');print(json.dumps(out,indent=2))
