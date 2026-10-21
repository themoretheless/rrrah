"""Original wrapper functions via AST; native extension replaced by explicit adapter.
Only authored files inside TemporaryDirectory are created or removed.
"""
import ast, os, pathlib, shutil, multiprocessing, typing, tempfile, types
source=pathlib.Path('.dedup-work/pydeduplines138-20261021/pydeduplines/__init__.py')
tree=ast.parse(source.read_text());tree.body=[n for n in tree.body if isinstance(n,ast.FunctionDef)]
ns=dict(os=os,pathlib=pathlib,shutil=shutil,multiprocessing=multiprocessing,typing=typing)
exec(compile(tree,str(source),'exec'),ns)
for operation in ['compute_unique_lines','compute_added_lines']:
 for mode in ['validation_error','native_error','success']:
  with tempfile.TemporaryDirectory() as td:
   root=pathlib.Path(td);work=root/'existing';work.mkdir();(work/'sentinel').write_text('authored unrelated fixture')
   inp=root/'input';inp.write_text('a\n');seen=[]
   def native(*args):
    seen.append(args)
    if mode=='native_error':raise RuntimeError('injected native failure')
   ns['pydeduplines']=types.SimpleNamespace(**{operation:native})
   path=str(root/'missing') if mode=='validation_error' else str(inp)
   args=[str(work),[path],str(root/'out'),1,1] if operation=='compute_unique_lines' else [str(work),path,str(inp),str(root/'out'),1,1]
   try:ns[operation](*args)
   except (FileNotFoundError,RuntimeError):assert mode!='success'
   else:assert mode=='success'
   assert not work.exists()
   assert len(seen)==(mode!='validation_error')
   assert inp.read_text()=='a\n'
   print('PASS',operation,mode,'pre-existing workdir removed; external input preserved')
