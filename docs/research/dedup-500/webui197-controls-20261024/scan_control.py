import importlib.util,pathlib,tempfile,json
b=pathlib.Path(__file__).parent;s=b.parent/'evidence/WSH032--image-deduplicate-cluster-webui/remaining-pinned/img_dedup_clust/tools/SearchImagesTags.py';sp=importlib.util.spec_from_file_location('scan197',s);m=importlib.util.module_from_spec(sp);sp.loader.exec_module(m)
with tempfile.TemporaryDirectory(prefix='review197-scan-') as root:
 p=pathlib.Path(root);(p/'a.JPG').write_bytes(b'not decoded');(p/'a.png').write_bytes(b'not decoded');(p/'a.txt').write_text('shared tags');(p/'nested').mkdir();(p/'nested/b.png').write_bytes(b'placeholder')
 obj=m.SearchImagesTags(str(p));names=obj.image_files();assert set(names)=={'a.JPG','a.png'};assert obj.tag_files()==['a.txt','a.txt'];assert obj.tag_content()==['shared tags','shared tags']
 (p/'c.png').write_bytes(b'placeholder');assert len(obj.tag_files())==2;assert len(obj.image_files())==3
 result={'scope':'Complete original SearchImagesTags module, actual owned temporary files; no decoding/model/UI.','cases':3,'findings':['Extension-only selection accepts invalid image bytes and does not recurse into nested directories. This is documented directory-listing scope, not image validation.','Two images sharing stem a map to same a.txt sidecar.','tag_files uses cached image list until image_files explicitly refreshes; tag_content refreshes sidecar mapping but not directory listing.']}
(b/'scan-control-results.json').write_text(json.dumps(result,indent=2)+'\n');print('3 scanner controls passed')
