import importlib.util,pathlib,tempfile,json,unittest.mock,logging,io
b=pathlib.Path(__file__).parent;s=b.parent/'evidence/WSH032--image-deduplicate-cluster-webui/remaining-pinned/img_dedup_clust/tools/operate_images.py'
spec=importlib.util.spec_from_file_location('review197_ops_failure',s);m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
results=[]
with tempfile.TemporaryDirectory(prefix='review197-owned-failure-') as root:
 p=pathlib.Path(root);src=p/'a.png';src.write_bytes(b'owned-synthetic-placeholder')
 for op,target in [('copy','copy2'),('move','move')]:
  log=io.StringIO();handler=logging.StreamHandler(log);logging.getLogger().addHandler(handler)
  try:
   with unittest.mock.patch.object(m.shutil,target,side_effect=OSError('injected owned operation failure')) as stub:
    value=m.operate_images_file(str(p),[['a.png']],operation=op)
   assert stub.call_count==1 and value is None and src.read_bytes()==b'owned-synthetic-placeholder'
   assert 'injected owned operation failure' in log.getvalue()
   results.append({'operation':op,'attempts':stub.call_count,'return_value':value,'source_preserved':True,'error_logged':True})
  finally:logging.getLogger().removeHandler(handler)
(b/'failure-control-results.json').write_text(json.dumps({'scope':'Complete unchanged original operations module; owned temporary source; only shutil copy2/move replaced with raising mocks. No real move/remove/rename. Original function creates owned output directories and catches the failure.','cases':results},indent=2)+'\n')
print('2 failure controls passed: error logged, return None, source preserved')
