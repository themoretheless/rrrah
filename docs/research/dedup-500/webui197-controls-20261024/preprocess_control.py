import ast,pathlib,tempfile,json,typing
import numpy as np
import cv2
from PIL import Image,ImageOps
b=pathlib.Path(__file__).parent;s=b.parent/'evidence/WSH032--image-deduplicate-cluster-webui/remaining-pinned/tag_images_by_wd14_tagger.py';t=ast.parse(s.read_text());nodes=[n for n in t.body if isinstance(n,ast.FunctionDef) and n.name in {'load_image','preprocess_image'}];ns={'np':np,'cv2':cv2,'Image':Image,'Path':pathlib.Path,'Union':typing.Union,'IMAGE_SIZE':448,'IMAGE_MODE':'RGB'};exec(compile(ast.Module(body=nodes,type_ignores=[]),str(s),'exec'),ns)
with tempfile.TemporaryDirectory(prefix='review197-preprocess-') as root:
 p=pathlib.Path(root)
 for name,color in [('red',(255,0,0,0)),('blue',(0,0,255,0))]:Image.new('RGBA',(12,12),color).save(p/(name+'.png'))
 a=ns['load_image'](p/'red.png');c=ns['load_image'](p/'blue.png');assert a.shape==(448,448,3) and a.dtype==np.float32 and not np.array_equal(a,c)
 im=Image.new('RGB',(12,20),'black');im.paste((255,0,0),(0,0,6,10));exif=Image.Exif();exif[274]=6;im.save(p/'oriented.jpg',exif=exif)
 with Image.open(p/'oriented.jpg') as decoded:
  raw=ns['preprocess_image'](decoded.convert('RGB'));upright=ns['preprocess_image'](ImageOps.exif_transpose(decoded).convert('RGB'))
 loaded=ns['load_image'](p/'oriented.jpg');assert np.array_equal(loaded,raw) and not np.array_equal(loaded,upright)
 result={'cases':2,'scope':'Unchanged AST-extracted load_image/preprocess_image, actual Pillow/OpenCV/NumPy and owned synthetic images; no model inference.','findings':['Fully transparent PNGs with different hidden RGB yield different prepared tensors: convert RGB discards alpha without canonical composition.','JPEG EXIF orientation6 ignored: loaded tensor equals untransposed decoded pixels and differs from explicit exif_transpose reference.'],'versions':{'opencv':cv2.__version__,'numpy':np.__version__}}
(b/'preprocess-control-results.json').write_text(json.dumps(result,indent=2)+'\n');print('2 real image preprocessing controls reproduced')
