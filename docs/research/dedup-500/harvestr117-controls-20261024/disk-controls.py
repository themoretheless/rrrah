import importlib.util, pathlib, tempfile, json, sys, hashlib
source=pathlib.Path(__file__).with_name('disk_manager.py')
spec=importlib.util.spec_from_file_location('original_disk',source);m=importlib.util.module_from_spec(spec);sys.modules[spec.name]=m;spec.loader.exec_module(m)
root=pathlib.Path(tempfile.mkdtemp(prefix='disk-owned-',dir=pathlib.Path(__file__).resolve().parent));downloads=root/'downloads';downloads.mkdir();folder=downloads/'sample';folder.mkdir()
inside=folder/'remove.mp4';inside.write_bytes(b'owned');outside=root/'keep.mp4';outside.write_bytes(b'keep')
history=downloads/'history.json';history.write_text(json.dumps({'sample':{'removed':{'output':str(inside)},'retained':{'output':str(outside)}}}))
d=m.DiskManager(downloads);result=d.delete_files([str(inside),str(outside)])
assert result['removed']==1 and result['skipped']==1
assert not inside.exists() and outside.read_bytes()==b'keep'
assert json.loads(history.read_text())=={}
# Positive: only the successfully removed path is requested, other history survives.
inside.write_bytes(b'owned');history.write_text(json.dumps({'sample':{'removed':{'output':str(inside)},'retained':{'output':str(outside)}}}))
r=d.delete_files([str(inside)]);assert r['removed']==1
assert 'retained' in json.loads(history.read_text())['sample']
print(json.dumps({'source_sha256':hashlib.sha256(source.read_bytes()).hexdigest(),'controls_passed':2,'finding':'Mixed allowed/skipped paths: outside file survives, but its history is removed because requested paths rather than successfully removed paths drive sync. Single successful path positive preserves unrelated history.','result':result,'scope':'Original disk manager, real owned temp files only; no external files or production application','root':str(root)},indent=2))
