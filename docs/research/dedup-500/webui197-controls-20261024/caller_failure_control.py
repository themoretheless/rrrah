import runpy,pathlib,tempfile,unittest.mock,json,importlib.util,logging
b=pathlib.Path(__file__).parent
ns=runpy.run_path(str(b/'selection_control.py'))['ns']
s=b.parent/'evidence/WSH032--image-deduplicate-cluster-webui/remaining-pinned/img_dedup_clust/tools/operate_images.py';spec=importlib.util.spec_from_file_location('review197_caller_ops',s);m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
with tempfile.TemporaryDirectory(prefix='review197-caller-') as root:
 p=pathlib.Path(root);src=p/'a.png';src.write_bytes(b'owned');ns.update(cluster_list=[['a.png','b.png']],confirmed_images_dir=str(p),operate_images_file=m.operate_images_file)
 with unittest.mock.patch.object(m.shutil,'move',side_effect=OSError('injected move failure')):
  value=ns['confirm_cluster']('{"0":[0]}',1,'','')
 assert src.read_bytes()==b'owned' and ns['cluster_list']==[] and ns['confirmed_images_dir']==''
 assert '成功move' in value[1]
 (b/'caller-failure-results.json').write_text(json.dumps({'scope':'Original undecorated AST confirm_cluster linked to full unchanged operation module; JSON-only serialization and gr.update adapters; shutil.move raises on owned temporary source. No real move/delete.','cases':1,'returned_message':value[1],'source_preserved':True,'cluster_state_cleared':True,'confirmed_directory_cleared':True,'finding':'Caller reports success and clears scan state after all selected file moves fail.'},ensure_ascii=False,indent=2)+'\n')
print('Caller false-success reproduced after injected move failure')
