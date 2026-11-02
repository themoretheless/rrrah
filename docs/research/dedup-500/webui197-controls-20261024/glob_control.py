import ast,pathlib,tempfile,json,typing,os
from collections import Counter
b=pathlib.Path(__file__).parent;s=b.parent/'evidence/WSH032--image-deduplicate-cluster-webui/remaining-pinned/tag_images_by_wd14_tagger.py';t=ast.parse(s.read_text());nodes=[n for n in t.body if isinstance(n,ast.FunctionDef) and n.name in {'glob_images_pathlib','check_same_name_path'}];ns={'Path':pathlib.Path,'Optional':typing.Optional,'Union':typing.Union,'List':typing.List,'Counter':Counter,'os':os,'IMAGE_EXTENSIONS':['.png','.jpg','.jpeg','.webp','.bmp','.PNG','.JPG','.JPEG','.WEBP','.BMP']};exec(compile(ast.Module(body=nodes,type_ignores=[]),str(s),'exec'),ns)
with tempfile.TemporaryDirectory(prefix='review197-glob-') as root:
 p=pathlib.Path(root);(p/'a.png').write_bytes(b'owned');(p/'a.jpg').write_bytes(b'owned');(p/'dir.png').mkdir();(p/'nested').mkdir();(p/'nested/b.png').write_bytes(b'owned')
 flat=ns['glob_images_pathlib'](p,False);recursive=ns['glob_images_pathlib'](p,True)
 assert p/'dir.png' in flat and p/'nested/b.png' not in flat and p/'nested/b.png' in recursive
 aliases=ns['check_same_name_path'](flat);assert any(set(x)=={str(p/'a.png'),str(p/'a.jpg')} for x in aliases)
 result={'cases':2,'scope':'Unchanged original glob/check_same_name AST functions, real pathlib on owned temporary files/directories; no decoder/model.','findings':['Glob accepts directory named dir.png; no is_file filter. Later image loading may reject it, so no claim directory decoded successfully.','Same-stem image pair detected; load_data source explicitly logs overwrite warning and continues. Recursive flag controls nested discovery.']}
(b/'glob-control-results.json').write_text(json.dumps(result,indent=2)+'\n');print('2 glob controls passed')
