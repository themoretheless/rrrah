import importlib.util,pathlib,tempfile,json,hashlib
from PIL import Image
b=pathlib.Path(__file__).parent;s=b.parent/'evidence/WSH032--image-deduplicate-cluster-webui/remaining-pinned/img_dedup_clust/tools/operate_images.py'
spec=importlib.util.spec_from_file_location('review197_operations',s);m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
with tempfile.TemporaryDirectory(prefix='review197-owned-') as root:
 p=pathlib.Path(root);src=p/'image.png';cache=p/'cache';Image.new('RGB',(16,16),'red').save(src);assert m.cache_images_file([str(src)],str(cache),8)==False
 old=(cache/'image.png').read_bytes();Image.new('RGB',(16,16),'blue').save(src);assert m.cache_images_file([str(src)],str(cache),8)==False
 assert old==(cache/'image.png').read_bytes();assert Image.open(cache/'image.png').getpixel((0,0))==(255,0,0)
 result={'scope':'Complete original operation module imported unchanged with real Pillow/tqdm; synthetic temporary owned PNGs only; no remove/move/rename invoked.','source_sha256':hashlib.sha256(s.read_bytes()).hexdigest(),'case':'same-name changed-source cache','observed':'Second cache call returns False (no error) and retains old red thumbnail after source becomes blue.','cases':1}
(b/'cache-control-results.json').write_text(json.dumps(result,indent=2)+'\n');print('Stale thumbnail reproduced')
