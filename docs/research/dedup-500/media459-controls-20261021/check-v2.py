import pathlib,subprocess,tempfile,ast,types,json,hashlib
root=pathlib.Path('.dedup-work/media459-20261021').resolve();binary=pathlib.Path('.dedup-work/media459-controls-20261021/main_cli').resolve();old=json.load(open('docs/research/dedup-500/media459-controls-20261021/result.json'));assert old['source_sha256']['main_cli.cpp']==hashlib.sha256((root/'main_cli.cpp').read_bytes()).hexdigest()
script=root/'duplicate_gui.py';tree=ast.parse(script.read_bytes());cls=next(n for n in tree.body if isinstance(n,ast.ClassDef) and n.name=='DuplicateFinderGUI');method=next(n for n in cls.body if isinstance(n,ast.FunctionDef) and n.name=='parse_results');ns={};exec(compile(ast.Module(body=[method],type_ignores=[]),str(script),'exec'),ns)
with tempfile.TemporaryDirectory(prefix='rrrah-media459-protocol-') as tmp:
 p=pathlib.Path(tmp);(p/'a|b.txt').write_bytes(b'content');(p/'normal.txt').write_bytes(b'content');r=subprocess.run([str(binary),str(p)],capture_output=True,text=True,check=True);assert 'EXACT|' in r.stdout
 self=types.SimpleNamespace(status_var=types.SimpleNamespace(set=lambda _:None),root=types.SimpleNamespace(update_idletasks=lambda:None),display_results=lambda:None)
 try:ns['parse_results'](self,r.stdout)
 except ValueError as e:error=str(e)
 else:raise AssertionError('Expected delimiter failure')
 missing=subprocess.run([str(binary),str(p/'missing')],capture_output=True,text=True);assert missing.returncode==0 and not missing.stdout and 'Error scanning directory' in missing.stderr
 output={'method':'Original GUI parse_results AST method unchanged, only nonvisual status/root/display transport adapters. Actual previously compiled unchanged CLI on owned temp files. No Tk window, deletion or confirmation functions executed.','gui_source_sha256':hashlib.sha256(script.read_bytes()).hexdigest(),'cli_binary_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),'pipe_filename':{'backend_stdout':r.stdout,'parser_error':error},'missing_directory':{'exit_code':missing.returncode,'stdout':missing.stdout,'stderr':missing.stderr},'limits':'Cannot infer full GUI event loop, thread timing, cancellation, packaged Windows behavior or real deletion from these controls.'}
pathlib.Path('docs/research/dedup-500/media459-controls-20261021/result-v2.json').write_text(json.dumps(output,indent=2)+'\n');print(json.dumps(output,indent=2))
